use super::instance::{ArmourDataInstance, Instance, InstanceSelector, Instances, Meta};
use actix::prelude::*;
use armour_api::master::MasterCodec;
use armour_api::proxy::PolicyRequest;
use log::*;
use std::collections::HashMap;
use tokio_util::codec::FramedRead;

/// Actor that handles Unix socket connections
pub struct ArmourDataMaster {
    instances: Instances,                        // instance actor addresses and info
    children: HashMap<u32, std::process::Child>, // maps PID to child process
    count: usize,                                // enumerates instances
    socket: std::path::PathBuf,                  // path to master's UDS socket
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
            InstanceSelector::Name(instance_name) => {
                let v: Vec<&Instance> = self
                    .instances
                    .0
                    .iter()
                    .filter_map(|i| match &i.1.meta {
                        Some(Meta { name, .. }) if *name == instance_name => Some(i.1),
                        _ => None,
                    })
                    .collect();
                if v.is_empty() {
                    warn!("there are no instances with name: {}", instance_name)
                };
                v
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
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        info!("removing instance: {}", msg.0);
        if let Some(instance) = self.instances.0.remove(&msg.0) {
            if let Some(meta) = instance.meta {
                if let Some(mut child) = self.children.remove(&meta.pid) {
                    if let Ok(code) = child.wait() {
                        log::info!("{} exited with {}", meta, code)
                    }
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype("()")]
pub struct RegisterProxy(pub usize, pub u32, pub String);

impl Handler<RegisterProxy> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: RegisterProxy, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(instance) = self.instances.0.get_mut(&msg.0) {
            instance.set_meta(msg.1, &msg.2)
        }
    }
}

// message sent when new proxy is "launched"
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
}

impl Handler<MasterCommand> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: MasterCommand, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            MasterCommand::Quit => System::current().stop(),
            MasterCommand::ListActive => {
                if self.instances.0.is_empty() {
                    info!("there are no active instances")
                } else {
                    info!("active instances: {}", self.instances)
                }
            }
        }
    }
}

#[derive(Message)]
#[rtype("Option<&'static str>")]
pub struct PolicyCommand(pub InstanceSelector, pub PolicyRequest);

impl Handler<PolicyCommand> for ArmourDataMaster {
    type Result = Option<&'static str>;
    fn handle(&mut self, msg: PolicyCommand, _ctx: &mut Context<Self>) -> Self::Result {
        let selected = self.get_instances(msg.0);
        if selected.is_empty() {
            Some("failed to select a proxy")
        } else if msg.1.valid() {
            for instance in selected {
                instance.addr.do_send(msg.1.clone())
            }
            None
        } else {
            Some("policy is empty")
        }
    }
}
