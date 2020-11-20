use super::instance::{ArmourDataInstance, Instance, InstanceSelector, Instances, Meta};
use actix::prelude::*;
use armour_api::{
    control::{OnboardServiceRequest, OnboardServiceResponse, PolicyQueryRequest, PolicyQueryResponse},
    host::{self, HostCodec},
    proxy::{LabelOp, PolicyRequest},
};
use armour_lang::{
    labels::{Label, Labels},
    literals::DPID
};
use log::*;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use tokio_util::codec::FramedRead;
use std::str::FromStr;

/// Actor that handles Unix socket connections
pub struct ArmourDataHost {
    client: actix_web::client::Client,
    url: url::Url,                               // control plane URL
    label: Label,         // host label (for communication with control plane)
    onboarded: bool,      // did we succesfully on-board with control plane?
    instances: Instances, // instance actor addresses and info
    children: HashMap<u32, std::process::Child>, // maps PID to child process
    count: usize,         // enumerates instances
    socket: std::path::PathBuf, // path to host's UDS socket
    key: [u8; 32],        // host key (for metadata encryption)
}

impl Actor for ArmourDataHost {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("removing socket: {}", self.socket.display());
        std::fs::remove_file(self.socket.clone())
            .unwrap_or_else(|e| warn!("failed to remove socket: {}", e))
    }
}

impl ArmourDataHost {
    pub fn new(
        client: actix_web::client::Client,
        url: &url::Url,
        label: &Label,
        onboarded: bool,
        socket: std::path::PathBuf,
        key: [u8; 32],
    ) -> Self {
        ArmourDataHost {
            client,
            url: url.to_owned(),
            label: label.clone(),
            onboarded,
            instances: Instances::default(),
            children: HashMap::new(),
            count: 0,
            socket,
            key,
        }
    }
    fn update_instances(&mut self, instances:InstanceSelector, tmp_dpid: DPID) {
        let instances = self.get_instances(&instances);
        for instance in instances {
            if let Some( ref mut meta ) = &mut instance.meta {
                meta.tmp_dpid = Some(tmp_dpid.clone());
            }
        }
    }

    fn get_instances(&mut self, instances: &InstanceSelector) -> Vec<&mut Instance> {
        match instances {
            InstanceSelector::Label(instance_label) => {
                let v: Vec<&mut Instance> = self
                    .instances
                    .0
                    .iter_mut()
                    .filter_map(|i| match &i.1.meta {
                        Some(Meta { label, .. }) if instance_label.matches_with(label) => Some(i.1),
                        _ => None,
                    })
                    .collect();
                v
            }
            InstanceSelector::ID(id) => match self.instances.0.get_mut(&id) {
                None => Vec::new(),
                Some(instance) => vec![instance],
            },
            InstanceSelector::All => self.instances.0.values_mut().collect(),
        }
    }
}

/// Notification of new Unix socket connection
#[derive(Message)]
#[rtype("()")]
pub struct UdsConnect(pub tokio::net::UnixStream);

impl Handler<UdsConnect> for ArmourDataHost {
    type Result = ();

    fn handle(&mut self, msg: UdsConnect, ctx: &mut Context<Self>) {
        // For each incoming connection we create `ArmourDataInstance` actor
        let host = ctx.address();
        ArmourDataInstance::create(move |ctx| {
            let (r, w) = tokio::io::split(msg.0);
            ctx.add_stream(FramedRead::new(r, HostCodec));
            ArmourDataInstance {
                id: 0,
                host,
                uds_framed: actix::io::FramedWrite::new(w, HostCodec, ctx),
            }
        });
    }
}

/// Connection notification (from Instance to Host)
pub struct Connect(pub Addr<ArmourDataInstance>);

impl Message for Connect {
    type Result = usize;
}

impl Handler<Connect> for ArmourDataHost {
    type Result = usize;
    fn handle(&mut self, msg: Connect, _ctx: &mut Context<Self>) -> Self::Result {
        let count = self.count;
        info!("adding instance: {}", count);
        self.instances.0.insert(count, Instance::new(msg.0));
        self.count += 1;
        count
    }
}

/// Disconnect notification (from Instance to Host)
#[derive(Message)]
#[rtype("()")]
pub struct Disconnect(pub usize);

impl Handler<Disconnect> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, ctx: &mut Context<Self>) -> Self::Result {
        info!("removing instance: {}", msg.0);
        if let Some(instance) = self.instances.0.remove(&msg.0) {
            if let Some(meta) = instance.meta {
                if let Some(mut child) = self.children.remove(&meta.pid) {
                    if let Ok(code) = child.wait() {
                        log::info!("{} exited with {}", meta, code);
                        if self.onboarded {
                            let onboard = OnboardServiceRequest {
                                service: {
                                    if let Some(dpid) = meta.tmp_dpid.clone() {
                                        match dpid.find_label(&Label::from_str("ServiceID::**").unwrap()) {
                                            Some(l) => l.clone(),
                                            _ =>  meta.label
                                        }
                                    } else {
                                        meta.label 
                                    }
                                },
                                host: self.label.clone(),
                                tmp_dpid: meta.tmp_dpid
                            };
                            let url = self.url.clone();
                            let client = self.client.clone();
                            async move {
                                crate::control_plane(
                                    client,
                                    &url,
                                    http::Method::DELETE,
                                    "service/drop",
                                    &onboard,
                                )
                                .await
                            }
                            .into_actor(self)
                            .then(|res, act, _ctx| {
                                match res {
                                    Ok(message) => {
                                        log::info!("control plane dropped proxy: {}", message)
                                    }
                                    Err(err) => log::warn!("error dropping proxy: {}", err),
                                };
                                async {}.into_actor(act)
                            })
                            .wait(ctx)
                        }
                    }
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct RegisterProxy(pub usize, pub Meta);

impl Handler<RegisterProxy> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: RegisterProxy, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            instance.set_meta(msg.1);
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct CPOnboardProxy(pub usize, pub HashMap<std::net::IpAddr, Labels>);

impl Handler<CPOnboardProxy> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: CPOnboardProxy, ctx: &mut Context<Self>) -> Self::Result {
        if !self.onboarded {
            return;
        }
        // if host on-boarded then notify control plane
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            match &mut instance.meta {
                None => log::warn!("RegisterProxy should be processed before CPOnboardingProxy"),
                Some(meta) => {
                    let label = meta.label.clone();

                    if msg.1.len() > 1 {
                        log::warn!("Information propagation from proxy to CP assumes there is only one µservice per proxy");
                    }
                    
                    let mut tmp_dpid = meta.tmp_dpid.clone().unwrap_or(DPID::default());
                    for (ip, labels) in msg.1 {
                        let mut ips = BTreeSet::new();
                        ips.insert(ip);
                        tmp_dpid.ips = ips;
                        tmp_dpid.labels = labels;
                        break;//FIXME assume only one µservice per proxy
                    }

                    let instance = InstanceSelector::Label(label.clone());
                    let onboard = OnboardServiceRequest {
                        service: label.clone(),
                        host: self.label.clone(),
                        tmp_dpid: Some(tmp_dpid.clone())
                    };
                    let url = self.url.clone();
                    let url_clone = self.url.clone();
                    let client = self.client.clone();
                    
                    // on-board
                    async move {
                        (tmp_dpid, crate::control_plane_deserialize::<_, OnboardServiceResponse>(
                            client,
                            &url,
                            http::Method::POST,
                            "service/on-board",
                            &onboard,
                        )
                        .await)
                    }
                    .into_actor(self)
                    .then(|(mut tmp_dpid, on_board_res), act, _ctx| {
                        let client = act.client.clone();
                        async move {
                            let service_id = match on_board_res {
                                Ok(req) => {
                                    log::info!("registered service with control plane: {}", req.service_id);
                                    req.service_id
                                }
                                Err(ref err) => {
                                    log::warn!("failed to register service with control plane: {}", err);
                                    label.clone()
                                }
                            };

                            let query = PolicyQueryRequest {
                                label: service_id.clone(),
                            };
                            
                            tmp_dpid.labels.insert(service_id);

                            // query policy
                            (tmp_dpid, crate::control_plane_deserialize::<_, PolicyQueryResponse>(
                                client,
                                &url_clone,
                                http::Method::GET,
                                "policy/query",
                                &query,
                            )
                            .await)

                        }
                        .into_actor(act)
                        .then(|(tmp_dpid, policy_res), act, ctx| {
                            // log::debug!("got labels: {:?}", policy_res.labels);
                            match policy_res {
                                Ok(policy_response) => {
                                    ctx.notify(PolicyCommand::new(
                                        instance.clone(),
                                        PolicyRequest::Label(LabelOp::AddUri(
                                            policy_response.labels.into_iter().collect(),
                                        )),
                                    ));
                                    ctx.notify(PolicyCommand::new(
                                        instance.clone(),
                                        PolicyRequest::SetPolicy(policy_response.policy),
                                    ));
                                    ctx.notify(ServiceGlobalID::new(tmp_dpid, instance))
                                }
                                Err(err) => log::warn!("failed to obtain policy: {}", err),
                            };
                            async {}.into_actor(act)
                        })
                    })
                    .wait(ctx)
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct RegisterHttpHash(pub usize, pub String);

impl Handler<RegisterHttpHash> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: RegisterHttpHash, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            instance.set_http_hash(&msg.1)
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct RegisterTcpHash(pub usize, pub String);

impl Handler<RegisterTcpHash> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: RegisterTcpHash, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            instance.set_tcp_hash(&msg.1)
        }
    }
}

// launch a new proxy
#[derive(Message)]
#[rtype("()")]
pub struct Launch {
    force: bool,
    label: Label,
    log: log::Level,
    timeout: Option<u8>,
}

impl Launch {
    pub fn new(label: Label, force: bool, log: log::Level, timeout: Option<u8>) -> Self {
        Launch {
            force,
            label,
            log,
            timeout,
        }
    }
}

impl Handler<Launch> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: Launch, _ctx: &mut Context<Self>) -> Self::Result {
        let instance = InstanceSelector::Label(msg.label.clone());
        if msg.force || self.get_instances(&instance).is_empty() {
            let armour_proxy = armour_proxy();
            let mut command = std::process::Command::new(&armour_proxy);
            command
                .env("ARMOUR_PASS", base64::encode(&self.key))
                .arg("-l")
                .arg(msg.log.to_string().to_lowercase())
                .arg("--label")
                .arg(&msg.label.to_string());
            if let Some(secs) = msg.timeout {
                command.arg("--timeout").arg(secs.to_string());
            }
            match command.arg(&self.socket).spawn() {
                Ok(child) => {
                    let pid = child.id();
                    log::info!("launched proxy processs: {} {}", msg.label, pid);
                    self.children.insert(pid, child);
                }
                Err(err) => log::warn!("failed to launch: {}\n{}", armour_proxy.display(), err),
            }
        } else if !msg.force {
            log::warn!(r#"proxy "{}" already exists"#, msg.label)
        }
    }
}

fn armour_proxy() -> std::path::PathBuf {
    if let Ok(Some(path)) =
        std::env::current_exe().map(|path| path.parent().map(|dir| dir.join("armour-proxy")))
    {
        path
    } else {
        std::path::PathBuf::from("./armour-proxy")
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct Quit;

impl Handler<Quit> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, _msg: Quit, _ctx: &mut Context<Self>) -> Self::Result {
        info!("removing socket: {}", self.socket.display());
        std::fs::remove_file(self.socket.clone())
            .unwrap_or_else(|e| warn!("failed to remove socket: {}", e));
        System::current().stop()
    }
}

#[derive(Message)]
#[rtype("Arc<Vec<String>>")]
pub struct List;

impl Handler<List> for ArmourDataHost {
    type Result = Arc<Vec<String>>;
    fn handle(&mut self, _msg: List, _ctx: &mut Context<Self>) -> Self::Result {
        if self.instances.0.is_empty() {
            info!("there are no active instances")
        } else {
            info!("active instances: {}", self.instances)
        }
        let list: BTreeSet<String> = self
            .instances
            .0
            .values()
            .filter_map(|i| i.meta.as_ref().map(|m| m.label.to_string()))
            .collect();
        Arc::new(list.into_iter().collect())
    }
}

#[derive(Message)]
#[rtype("Arc<Vec<host::PolicyStatus>>")]
pub struct MetaData(pub InstanceSelector);

impl Handler<MetaData> for ArmourDataHost {
    type Result = Arc<Vec<host::PolicyStatus>>;
    fn handle(&mut self, msg: MetaData, _ctx: &mut Context<Self>) -> Self::Result {
        Arc::new(
            self.get_instances(&msg.0)
                .iter()
                .filter_map(|i| i.meta.as_ref().map(host::PolicyStatus::from))
                .collect(),
        )
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct ServiceGlobalID(pub DPID, pub InstanceSelector);
impl ServiceGlobalID {
    pub fn new(global_id:DPID, instances: InstanceSelector) -> Self {
        ServiceGlobalID(global_id, instances)
    }
}
impl Handler<ServiceGlobalID> for ArmourDataHost {
    type Result = ();
    fn handle(&mut self, msg: ServiceGlobalID, _ctx: &mut Context<Self>) -> Self::Result {
        let ServiceGlobalID(global_id, instances) = msg;
        log::info!("global id is: {:#?} for {:?}", global_id.clone(), instances.clone());
        self.update_instances(instances, global_id);
    }
}

#[derive(Message)]
#[rtype("Option<&'static str>")]
pub struct PolicyCommand(pub bool, pub InstanceSelector, pub PolicyRequest);

impl PolicyCommand {
    pub fn new(instance: InstanceSelector, req: PolicyRequest) -> Self {
        PolicyCommand(false, instance, req)
    }
    pub fn new_with_retry(instance: InstanceSelector, req: PolicyRequest) -> Self {
        PolicyCommand(true, instance, req)
    }
    fn second_attempt(self) -> Self {
        PolicyCommand(false, self.1, self.2)
    }
}

impl Handler<PolicyCommand> for ArmourDataHost {
    type Result = Option<&'static str>;
    fn handle(&mut self, msg: PolicyCommand, ctx: &mut Context<Self>) -> Self::Result {
        let PolicyCommand(retry, instance, request) = &msg;
        let selected = self.get_instances(instance);
        if selected.is_empty() {
            if *retry {
                ctx.notify_later(msg.second_attempt(), std::time::Duration::from_secs(1));
                static MSG: &str = "failed to select a proxy: will try once more...";
                log::warn!("{}", MSG);
                Some(MSG)
            } else {
                static MSG: &str = "failed to select a proxy";
                log::warn!("{}", MSG);
                Some(MSG)
            }
        } else {
            for instance in selected {
                instance.addr.do_send(request.clone())
            }
            None
        }
    }
}

