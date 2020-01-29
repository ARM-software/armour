use super::master::{ArmourDataMaster, Connect, Disconnect, RegisterPid};
use actix::prelude::*;
use armour_api::master::{MasterCodec, PolicyResponse};
use armour_api::proxy::PolicyRequest;
use log::*;
use std::collections::HashMap;
use std::fmt;
use tokio::io::WriteHalf;

#[derive(Clone, PartialEq)]
pub enum InstanceSelector {
    All,
    Error,
    ID(usize),
}

pub struct Instance {
    pub pid: Option<u32>,
    pub addr: Addr<ArmourDataInstance>,
}

impl Instance {
    pub fn new(addr: Addr<ArmourDataInstance>) -> Self {
        Instance { pid: None, addr }
    }
    pub fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid)
    }
}

#[derive(Default)]
pub struct Instances(pub HashMap<usize, Instance>);

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

/// Actor that handles communication with a data plane instance
///
/// There will be one actor per Unix socket connection
pub struct ArmourDataInstance {
    pub id: usize,
    pub master: Addr<ArmourDataMaster>,
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio::net::UnixStream>, MasterCodec>,
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
