use super::host::{
    ArmourDataHost, Connect, Disconnect, RegisterHttpHash, RegisterProxy, RegisterTcpHash,
};
use actix::prelude::*;
use armour_api::host::{self, HostCodec, PolicyResponse};
use armour_api::proxy::PolicyRequest;
use armour_lang::{labels::Label, literals::DPID, policies::Protocol};
use log::*;
use std::collections::HashMap;
use std::fmt;
use tokio::io::WriteHalf;

#[derive(Clone, PartialEq, Debug)]
pub enum InstanceSelector {
    All,
    Label(Label),
    ID(usize),
}

#[derive(Clone)]
pub struct Meta {
    pub pid: u32,
    pub tmp_dpid: Option<DPID>,
    pub label: Label,
    pub http: String, // hash of HTTP policy
    pub tcp: String,  // hash of TCP policy
}

impl From<&Meta> for host::PolicyStatus {
    fn from(m: &Meta) -> Self {
        host::PolicyStatus {
            label: m.label.to_owned(),
            http: m.http.to_string(),
            tcp: m.tcp.to_string(),
        }
    }
}

impl Meta {
    fn new(pid: u32, tmp_dpid: Option<DPID>, label: Label, http: String, tcp: String) -> Self {
        Meta {
            pid,
            tmp_dpid,
            label,
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
            self.label, self.pid, self.http, self.tcp
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
    /// unique ID of instance
    pub id: usize,
    /// address of data plane host actor
    pub host: Addr<ArmourDataHost>,
    pub uds_framed:
        actix::io::FramedWrite<PolicyRequest, WriteHalf<tokio::net::UnixStream>, HostCodec>,
}

impl Actor for ArmourDataInstance {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.host
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
                PolicyResponse::Connect(pid, tmp_dpid, label, http, tcp) => {
                    info!(r#"{}: connect with process "{}" {} {:?}"#, self.id, label, pid, tmp_dpid);
                    self.host
                        .do_send(RegisterProxy(self.id, Meta::new(pid, tmp_dpid, label, http, tcp)))
                }
                PolicyResponse::Started => info!("{}: started a proxy", self.id),
                PolicyResponse::Stopped => info!("{}: stopped a proxy", self.id),
                PolicyResponse::UpdatedPolicy(protocol, hash) => {
                    info!("{}: updated policy", self.id);
                    match protocol {
                        Protocol::HTTP => self.host.do_send(RegisterHttpHash(self.id, hash)),
                        Protocol::TCP => self.host.do_send(RegisterTcpHash(self.id, hash)),
                        Protocol::Phantom(_) => unimplemented!()
                    }
                }
                PolicyResponse::RequestFailed => info!("{}: request failed", self.id),
                PolicyResponse::ShuttingDown => {
                    info!("{}: received shutdown", self.id);
                    self.host.do_send(Disconnect(self.id));
                    ctx.stop()
                }
                PolicyResponse::Status {
                    label,
                    labels,
                    http,
                    tcp,
                } => {
                    info!(
                        "{} {}:\n=== HTTP ===\n{}\n=== TCP ===\n{}\n=== Labels ===\n{:?}",
                        self.id, label, http, tcp, labels
                    );
                    self.host
                        .do_send(RegisterHttpHash(self.id, http.policy.blake3()));
                    self.host
                        .do_send(RegisterTcpHash(self.id, tcp.policy.blake3()))
                }
            }
        } else {
            log::warn!("response error: {}", msg.err().unwrap())
        }
    }
    fn finished(&mut self, ctx: &mut Self::Context) {
        log::warn!("{}: connection to instance has closed", self.id);
        self.host.do_send(Disconnect(self.id));
        ctx.stop()
    }
}

impl Handler<PolicyRequest> for ArmourDataInstance {
    type Result = ();
    fn handle(&mut self, msg: PolicyRequest, _ctx: &mut Context<Self>) {
        self.uds_framed.write(msg)
    }
}
