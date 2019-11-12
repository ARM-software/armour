#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;
use armour_data_interface::codec::{MasterCodec, PolicyRequest, PolicyResponse};
use std::collections::HashMap;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

#[derive(PartialEq)]
pub enum Instances {
    All,
    Error,
    SoleInstance,
    ID(usize),
}

pub mod commands;

/// Actor that handles Unix socket connections
pub struct ArmourDataMaster {
    instances: HashMap<usize, Addr<ArmourDataInstance>>,
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
pub struct UdsConnect(pub tokio_uds::UnixStream);

impl Handler<UdsConnect> for ArmourDataMaster {
    type Result = ();

    fn handle(&mut self, msg: UdsConnect, ctx: &mut Context<Self>) {
        // For each incoming connection we create `ArmourDataInstance` actor
        let master = ctx.address();
        ArmourDataInstance::create(move |ctx| {
            let (r, w) = msg.0.split();
            ArmourDataInstance::add_stream(FramedRead::new(r, MasterCodec), ctx);
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
            instances: HashMap::new(),
            count: 0,
            socket,
        }
    }
    fn get_instances(&self, instances: Instances) -> Vec<&Addr<ArmourDataInstance>> {
        match instances {
            Instances::Error => {
                warn!("failed to parse instance ID");
                Vec::new()
            }
            Instances::ID(id) => match self.instances.get(&id) {
                None => {
                    info!("instance {} does not exist", id);
                    Vec::new()
                }
                Some(instance) => vec![instance],
            },
            Instances::All => {
                if self.instances.is_empty() {
                    warn!("there are no instances")
                };
                self.instances.values().collect()
            }
            Instances::SoleInstance => match self.instances.len() {
                0 => {
                    warn!("there are no instances");
                    Vec::new()
                }
                1 => vec![self.instances.values().next().unwrap()],
                _ => {
                    warn!("there is more than one instance");
                    Vec::new()
                }
            },
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
        self.instances.insert(count, msg.0);
        self.count += 1;
        count
    }
}

/// Disconnect notification (from Instance to Master)
#[derive(Message)]
pub struct Disconnect(usize);

impl Handler<Disconnect> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        info!("removing instance: {}", msg.0);
        self.instances.remove(&msg.0);
    }
}

/// Represents commands sent to the data plane master.
///
/// Policy update request are forwarded on to the appropriate instance actor.
#[derive(Message)]
pub enum MasterCommand {
    ListActive,
    Quit,
    UpdatePolicy(Instances, Box<PolicyRequest>),
}

impl Handler<MasterCommand> for ArmourDataMaster {
    type Result = ();
    fn handle(&mut self, msg: MasterCommand, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            MasterCommand::Quit => System::current().stop(),
            MasterCommand::ListActive => {
                if self.instances.is_empty() {
                    info!("there are no active instances")
                } else {
                    info!(
                        "active instances: {:?}",
                        self.instances.keys().collect::<Vec<&usize>>()
                    )
                }
            }
            MasterCommand::UpdatePolicy(instances, request) => {
                for instance in self.get_instances(instances) {
                    instance.do_send(*request.clone())
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
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, MasterCodec>,
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
                actix::fut::ok(())
            })
            .wait(ctx);
    }
}

impl actix::io::WriteHandler<std::io::Error> for ArmourDataInstance {}

impl StreamHandler<PolicyResponse, std::io::Error> for ArmourDataInstance {
    fn handle(&mut self, msg: PolicyResponse, ctx: &mut Self::Context) {
        match msg {
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
    }
}

impl Handler<PolicyRequest> for ArmourDataInstance {
    type Result = ();
    fn handle(&mut self, msg: PolicyRequest, _ctx: &mut Context<Self>) {
        self.uds_framed.write(msg)
    }
}
