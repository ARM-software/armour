use super::master::{
    ArmourDataMaster, Connect, Disconnect, RegisterHttpHash, RegisterProxy, RegisterTcpHash,
};
use actix::prelude::*;
use armour_api::master::{self, MasterCodec, PolicyResponse};
use armour_api::proxy::{PolicyRequest, Protocol};
use log::*;
use std::collections::HashMap;
use std::fmt;
use tokio::io::WriteHalf;

#[derive(Clone, PartialEq)]
pub enum InstanceSelector {
    All,
    Name(String),
    ID(usize),
}

#[derive(Clone)]
pub struct Meta {
    pub pid: u32,
    pub name: String,
    pub http: String, // hash of HTTP policy
    pub tcp: String,  // hash of TCP policy
}

impl From<&Meta> for master::Proxy {
    fn from(m: &Meta) -> Self {
        master::Proxy {
            name: m.name.to_string(),
            http: m.http.to_string(),
            tcp: m.tcp.to_string(),
        }
    }
}

impl Meta {
    fn new(pid: u32, name: String, http: String, tcp: String) -> Self {
        Meta {
            pid,
            name,
            http,
            tcp,
        }
    }
}

impl fmt::Display for Meta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#""{}"; pid: {}; http: {}; tcp: {}"#,
            self.name, self.pid, self.http, self.tcp
        )
    }
}

pub struct Instance {
    pub meta: Option<Meta>,
    pub addr: Addr<ArmourDataInstance>,
}

impl Instance {
    pub fn new(addr: Addr<ArmourDataInstance>) -> Self {
        Instance { meta: None, addr }
    }
    pub fn set_meta(&mut self, meta: Meta) {
        self.meta = Some(meta)
    }
    pub fn set_http_hash(&mut self, s: &str) {
        if let Some(mut meta) = self.meta.as_mut() {
            meta.http = s.to_string();
        }
    }
    pub fn set_tcp_hash(&mut self, s: &str) {
        if let Some(mut meta) = self.meta.as_mut() {
            meta.tcp = s.to_string();
        }
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
                if let Some(ref meta) = i.meta {
                    format!(r#"{} ({})"#, n, meta)
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
                PolicyResponse::Connect(pid, name, http, tcp) => {
                    info!(r#"{}: connect with process "{}" {}"#, self.id, name, pid);
                    self.master
                        .do_send(RegisterProxy(self.id, Meta::new(pid, name, http, tcp)))
                }
                PolicyResponse::Started => info!("{}: started a proxy", self.id),
                PolicyResponse::Stopped => info!("{}: stopped a proxy", self.id),
                PolicyResponse::UpdatedPolicy(protocol, hash) => {
                    info!("{}: updated policy", self.id);
                    match protocol {
                        Protocol::HTTP => self.master.do_send(RegisterHttpHash(self.id, hash)),
                        Protocol::TCP => self.master.do_send(RegisterTcpHash(self.id, hash)),
                        Protocol::All => {
                            self.master.do_send(RegisterHttpHash(self.id, hash.clone()));
                            self.master.do_send(RegisterTcpHash(self.id, hash))
                        }
                    }
                }
                PolicyResponse::RequestFailed => info!("{}: request failed", self.id),
                PolicyResponse::ShuttingDown => {
                    info!("{}: received shutdown", self.id);
                    self.master.do_send(Disconnect(self.id));
                    ctx.stop()
                }
                PolicyResponse::Status { http, tcp } => {
                    info!("{}:\n=== HTTP ===\n{}=== TCP ===\n{}", self.id, http, tcp);
                    self.master
                        .do_send(RegisterHttpHash(self.id, http.policy.blake3_string()));
                    self.master
                        .do_send(RegisterTcpHash(self.id, tcp.policy.blake3_string()))
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
