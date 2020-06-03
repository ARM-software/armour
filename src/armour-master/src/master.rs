use super::instance::{ArmourDataInstance, Instance, InstanceSelector, Instances, Meta};
use actix::prelude::*;
use armour_api::{
    control::{OnboardServiceRequest, PolicyQueryRequest, PolicyQueryResponse},
    master::{self, MasterCodec},
    proxy::{LabelOp, PolicyRequest},
};
use armour_lang::labels::Label;
use log::*;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use tokio_util::codec::FramedRead;

/// Actor that handles Unix socket connections
pub struct ArmourDataMaster {
    url: String,                                 // control plane URL
    label: Label,         // master label (for communication with control plane)
    onboarded: bool,      // did we succesfully on-board with control plane?
    instances: Instances, // instance actor addresses and info
    children: HashMap<u32, std::process::Child>, // maps PID to child process
    count: usize,         // enumerates instances
    socket: std::path::PathBuf, // path to master's UDS socket
    key: [u8; 32],        // master key (for metadata encryption)
}

impl Actor for ArmourDataMaster {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("removing socket: {}", self.socket.display());
        std::fs::remove_file(self.socket.clone())
            .unwrap_or_else(|e| warn!("failed to remove socket: {}", e))
    }
}

impl ArmourDataMaster {
    pub fn new(
        url: &str,
        label: &Label,
        onboarded: bool,
        socket: std::path::PathBuf,
        key: [u8; 32],
    ) -> Self {
        ArmourDataMaster {
            url: url.to_string(),
            label: label.clone(),
            onboarded,
            instances: Instances::default(),
            children: HashMap::new(),
            count: 0,
            socket,
            key,
        }
    }
    fn get_instances(&self, instances: &InstanceSelector) -> Vec<&Instance> {
        match instances {
            InstanceSelector::Label(instance_label) => {
                let v: Vec<&Instance> = self
                    .instances
                    .0
                    .iter()
                    .filter_map(|i| match &i.1.meta {
                        Some(Meta { label, .. }) if instance_label.matches_with(label) => Some(i.1),
                        _ => None,
                    })
                    .collect();
                v
            }
            InstanceSelector::ID(id) => match self.instances.0.get(&id) {
                None => Vec::new(),
                Some(instance) => vec![instance],
            },
            InstanceSelector::All => self.instances.0.values().collect(),
        }
    }
}

/// Notification of new Unix socket connection
#[derive(Message)]
#[rtype("()")]
pub struct UdsConnect(pub tokio::net::UnixStream);

impl Handler<UdsConnect> for ArmourDataMaster {
    type Result = ();

    fn handle(&mut self, msg: UdsConnect, ctx: &mut Context<Self>) {
        // For each incoming connection we create `ArmourDataInstance` actor
        let master = ctx.address();
        ArmourDataInstance::create(move |ctx| {
            let (r, w) = tokio::io::split(msg.0);
            ctx.add_stream(FramedRead::new(r, MasterCodec));
            ArmourDataInstance {
                id: 0,
                master,
                uds_framed: actix::io::FramedWrite::new(w, MasterCodec, ctx),
            }
        });
    }
}

/// Connection notification (from Instance to Master)
pub struct Connect(pub Addr<ArmourDataInstance>);

impl Message for Connect {
    type Result = usize;
}

impl Handler<Connect> for ArmourDataMaster {
    type Result = usize;
    fn handle(&mut self, msg: Connect, _ctx: &mut Context<Self>) -> Self::Result {
        let count = self.count;
        info!("adding instance: {}", count);
        self.instances.0.insert(count, Instance::new(msg.0));
        self.count += 1;
        count
    }
}

/// Disconnect notification (from Instance to Master)
#[derive(Message)]
#[rtype("()")]
pub struct Disconnect(pub usize);

impl Handler<Disconnect> for ArmourDataMaster {
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
                                service: meta.label,
                                master: self.label.clone(),
                            };
                            let url = self.url.clone();
                            async move {
                                crate::control_plane(
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

impl Handler<RegisterProxy> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: RegisterProxy, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            let label = msg.1.label.clone();
            instance.set_meta(msg.1);
            // if master on-boarded then notify control plane
            if self.onboarded {
                let instance = InstanceSelector::Label(label.clone());
                let query = PolicyQueryRequest {
                    label: label.clone(),
                };
                let onboard = OnboardServiceRequest {
                    service: label,
                    master: self.label.clone(),
                };
                let url = self.url.clone();
                let url_clone = self.url.clone();
                // on-board
                async move {
                    crate::control_plane(&url, http::Method::POST, "service/on-board", &onboard)
                        .await
                }
                .into_actor(self)
                .then(|on_board_res, act, _ctx| {
                    async move {
                        match on_board_res {
                            Ok(message) => log::info!("on-boarded with control plane: {}", message),
                            Err(err) => log::warn!("on-boarding failed for service: {}", err),
                        };
                        // query policy
                        crate::control_plane_deserialize::<_, PolicyQueryResponse>(
                            &url_clone,
                            http::Method::GET,
                            "policy/query",
                            &query,
                        )
                        .await
                    }
                    .into_actor(act)
                    .then(|policy_res, act, ctx| {
                        match policy_res {
                            // log::debug!("got labels: {:?}", policy_response.labels);
                            Ok(policy_response) => {
                                ctx.notify(PolicyCommand::new(
                                    instance.clone(),
                                    PolicyRequest::Label(LabelOp::AddUri(
                                        policy_response.labels.into_iter().collect(),
                                    )),
                                ));
                                ctx.notify(PolicyCommand::new(
                                    instance,
                                    PolicyRequest::SetPolicy(policy_response.policy),
                                ))
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

#[derive(Message)]
#[rtype("()")]
pub struct RegisterHttpHash(pub usize, pub String);

impl Handler<RegisterHttpHash> for ArmourDataMaster {
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

impl Handler<RegisterTcpHash> for ArmourDataMaster {
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

impl Handler<Launch> for ArmourDataMaster {
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

impl Handler<Quit> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, _msg: Quit, _ctx: &mut Context<Self>) -> Self::Result {
        System::current().stop()
    }
}

#[derive(Message)]
#[rtype("Arc<Vec<String>>")]
pub struct List;

impl Handler<List> for ArmourDataMaster {
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
#[rtype("Arc<Vec<master::PolicyStatus>>")]
pub struct MetaData(pub InstanceSelector);

impl Handler<MetaData> for ArmourDataMaster {
    type Result = Arc<Vec<master::PolicyStatus>>;
    fn handle(&mut self, msg: MetaData, _ctx: &mut Context<Self>) -> Self::Result {
        Arc::new(
            self.get_instances(&msg.0)
                .iter()
                .filter_map(|i| i.meta.as_ref().map(master::PolicyStatus::from))
                .collect(),
        )
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

impl Handler<PolicyCommand> for ArmourDataMaster {
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
