#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;
use armour_api::master::{MasterCodec, PolicyResponse};
use armour_api::proxy::PolicyRequest;
use std::collections::HashMap;
use std::fmt;
use tokio::io::WriteHalf;
use tokio_util::codec::FramedRead;

#[derive(Clone, PartialEq)]
pub enum InstanceSelector {
    All,
    Error,
    ID(usize),
}

pub mod commands;
pub mod rest_policy;

struct Instance {
    pid: Option<u32>,
    addr: Addr<ArmourDataInstance>,
}

impl Instance {
    fn new(addr: Addr<ArmourDataInstance>) -> Self {
        Instance { pid: None, addr }
    }
    fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid)
    }
}

#[derive(Default)]
struct Instances(HashMap<usize, Instance>);

impl fmt::Display for Instances {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = self
            .0
            .iter()
            .map(|(n, i)| {
                if let Some(pid) = i.pid {
                    format!("{} ({})", n, pid)
                } else {
                    n.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(",");
        write!(f, "[{}]", s)
    }
}

/// Actor that handles Unix socket connections
pub struct ArmourDataMaster {
    instances: Instances,
    children: HashMap<u32, std::process::Child>,
    count: usize,
    socket: std::path::PathBuf,
}

impl Actor for ArmourDataMaster {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("removing socket: {}", self.socket.display());
        std::fs::remove_file(self.socket.clone())
            .unwrap_or_else(|e| warn!("failed to remove socket: {}", e))
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

impl ArmourDataMaster {
    pub fn new(socket: std::path::PathBuf) -> Self {
        ArmourDataMaster {
            instances: Instances::default(),
            children: HashMap::new(),
            count: 0,
            socket,
        }
    }
    fn get_instances(&self, instances: InstanceSelector) -> Vec<&Instance> {
        match instances {
            InstanceSelector::Error => {
                warn!("failed to parse instance ID");
                Vec::new()
            }
            InstanceSelector::ID(id) => match self.instances.0.get(&id) {
                None => {
                    info!("instance {} does not exist", id);
                    Vec::new()
                }
                Some(instance) => vec![instance],
            },
            InstanceSelector::All => {
                if self.instances.0.is_empty() {
                    warn!("there are no instances")
                };
                self.instances.0.values().collect()
            }
        }
    }
}

/// Connection notification (from Instance to Master)
pub struct Connect(Addr<ArmourDataInstance>);

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
pub struct Disconnect(usize);

impl Handler<Disconnect> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        info!("removing instance: {}", msg.0);
        if let Some(instance) = self.instances.0.remove(&msg.0) {
            if let Some(pid) = instance.pid {
                if let Some(mut child) = self.children.remove(&pid) {
                    if let Ok(code) = child.wait() {
                        log::info!("{} exited with {}", pid, code)
                    }
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct RegisterPid(usize, u32);

impl Handler<RegisterPid> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: RegisterPid, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            instance.set_pid(msg.1)
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct AddChild(pub u32, pub std::process::Child);

impl Handler<AddChild> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: AddChild, _ctx: &mut Context<Self>) -> Self::Result {
        self.children.insert(msg.0, msg.1);
    }
}

/// Represents commands sent to the data plane master.
///
/// Policy update request are forwarded on to the appropriate instance actor.
#[derive(Message)]
#[rtype("()")]
pub enum MasterCommand {
    ListActive,
    Quit,
    UpdatePolicy(InstanceSelector, Box<PolicyRequest>),
}

impl Handler<MasterCommand> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: MasterCommand, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            MasterCommand::Quit => System::current().stop(), // Not working in actix 0.9!
            MasterCommand::ListActive => {
                if self.instances.0.is_empty() {
                    info!("there are no active instances")
                } else {
                    info!("active instances: {}", self.instances)
                }
            }
            MasterCommand::UpdatePolicy(instances, request) => {
                for instance in self.get_instances(instances) {
                    instance.addr.do_send(*request.clone())
                }
            }
        }
    }
}

/// Actor that handles communication with a data plane instance
///
/// There will be one actor per Unix socket connection
pub struct ArmourDataInstance {
    id: usize,
    master: Addr<ArmourDataMaster>,
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio::net::UnixStream>, MasterCodec>,
}

impl Actor for ArmourDataInstance {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.master
            .send(Connect(ctx.address()))
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    _ => ctx.stop(),
                };
                async {}.into_actor(act)
            })
            .wait(ctx);
    }
}

impl actix::io::WriteHandler<std::io::Error> for ArmourDataInstance {}

impl StreamHandler<Result<PolicyResponse, std::io::Error>> for ArmourDataInstance {
    fn handle(&mut self, msg: Result<PolicyResponse, std::io::Error>, ctx: &mut Self::Context) {
        if let Ok(msg) = msg {
            match msg {
                PolicyResponse::Connect(pid) => {
                    self.master.do_send(RegisterPid(self.id, pid));
                    info!("{}: connect with process {}", self.id, pid)
                }
                PolicyResponse::Started => info!("{}: started a proxy", self.id),
                PolicyResponse::Stopped => info!("{}: stopped a proxy", self.id),
                PolicyResponse::UpdatedPolicy => info!("{}: updated policy", self.id),
                PolicyResponse::RequestFailed => info!("{}: request failed", self.id),
                PolicyResponse::ShuttingDown => {
                    info!("{}: received shutdown", self.id);
                    self.master.do_send(Disconnect(self.id));
                    ctx.stop()
                }
                PolicyResponse::Status { http, tcp } => {
                    info!("{}:\n=== HTTP ===\n{}=== TCP ===\n{}", self.id, http, tcp)
                }
            }
        } else {
            log::warn!("response error: {}", msg.err().unwrap())
        }
    }
    fn finished(&mut self, ctx: &mut Self::Context) {
        log::warn!("{}: connection to instance has closed", self.id);
        self.master.do_send(Disconnect(self.id));
        ctx.stop()
    }
}

impl Handler<PolicyRequest> for ArmourDataInstance {
    type Result = ();
    fn handle(&mut self, msg: PolicyRequest, _ctx: &mut Context<Self>) {
        self.uds_framed.write(msg)
    }
}
