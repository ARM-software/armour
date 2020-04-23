//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor, ID};
use super::tcp_proxy;
use super::Stop;
use actix::prelude::*;
use armour_api::master::Status;
use armour_lang::{
    expressions::{Error, Expr},
    interpret::Env,
    lang,
    meta::IngressEgress,
};
use futures::future::{self, TryFutureExt};
use std::sync::Arc;

pub struct TcpPolicy {
    connect: lang::Policy,
    disconnect: lang::Policy,
    program: Arc<lang::Program>,
    env: Env,
    proxy: Option<(Addr<tcp_proxy::TcpDataServer>, u16)>,
}

impl Policy<Addr<tcp_proxy::TcpDataServer>> for TcpPolicy {
    fn start(&mut self, server: Addr<tcp_proxy::TcpDataServer>, port: u16) {
        self.proxy = Some((server, port))
    }
    fn stop(&mut self) {
        if let Some((server, port)) = &self.proxy {
            log::info!("stopping TCP proxy on port {}", port);
            server.do_send(Stop);
        }
        self.proxy = None
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.connect = p.policy(lang::ALLOW_TCP_CONNECTION);
        self.disconnect = p.policy(lang::ON_TCP_DISCONNECT);
        self.program = Arc::new(p);
        self.env = Env::new(&self.program)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.1)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn hash(&self) -> String {
        self.program.blake3_string()
    }
    fn env(&self) -> &Env {
        &self.env
    }
    fn status(&self) -> Box<Status> {
        Box::new(Status {
            port: self.port(),
            policy: (*self.policy()).clone(),
        })
    }
}

impl Default for TcpPolicy {
    fn default() -> Self {
        let program = Arc::new(lang::Program::deny_all(&lang::TCP_POLICY).unwrap_or_default());
        TcpPolicy {
            connect: lang::Policy::default(),
            disconnect: lang::Policy::default(),
            program: program.clone(),
            env: Env::new(&program),
            proxy: None,
        }
    }
}

// TCP connection policies
#[derive(Message)]
#[rtype("Result<TcpPolicyStatus, Error>")]
pub struct GetTcpPolicy(pub std::net::SocketAddr, pub std::net::SocketAddr);

pub enum TcpPolicyStatus {
    Allow(Box<Option<ConnectionStats>>),
    Block,
}

impl Handler<GetTcpPolicy> for PolicyActor {
    type Result = ResponseFuture<Result<TcpPolicyStatus, Error>>;

    fn handle(&mut self, msg: GetTcpPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        match self.tcp.connect {
            lang::Policy::Allow => {
                self.connection_number += 1;
                Box::pin(future::ok(TcpPolicyStatus::Allow(Box::new(None))))
            }
            lang::Policy::Deny => {
                log::info!("deny");
                Box::pin(future::ok(TcpPolicyStatus::Block))
            }
            lang::Policy::Args(n) if n == 1 => {
                let connection = self
                    .connection(ID::SocketAddr(msg.0), ID::SocketAddr(msg.1))
                    .into();
                let stats = ConnectionStats::new(&connection);
                Box::pin(
                    self.tcp
                        .evaluate(
                            lang::ALLOW_TCP_CONNECTION,
                            vec![connection],
                            IngressEgress::default(), // TODO
                        )
                        .and_then(move |(res, _meta)| {
                            future::ok(if res {
                                TcpPolicyStatus::Allow(Box::new(Some(stats)))
                            } else {
                                TcpPolicyStatus::Block
                            })
                        }),
                )
            }
            _ => unreachable!(), // policy is checked beforehand
        }
    }
}

#[derive(Message)]
#[rtype("Result<(),()>")]
#[derive(Clone)]
pub struct ConnectionStats {
    pub sent: usize,
    pub received: usize,
    pub connection: Expr,
}

impl ConnectionStats {
    pub fn new(connection: &Expr) -> ConnectionStats {
        ConnectionStats {
            sent: 0,
            received: 0,
            connection: connection.clone(),
        }
    }
}

// sent by the TCP proxy when the TCP connection finishes
impl Handler<ConnectionStats> for PolicyActor {
    type Result = ResponseFuture<Result<(), ()>>;

    fn handle(&mut self, msg: ConnectionStats, _ctx: &mut Context<Self>) -> Self::Result {
        if let lang::Policy::Args(arg_count) = self.tcp.disconnect {
            let args = match arg_count {
                3 => vec![
                    msg.connection,
                    Expr::from(msg.sent),
                    Expr::from(msg.received),
                ],
                _ => unreachable!(), // policy is checked beforehand
            };
            Box::pin(
                self.tcp
                    .evaluate(lang::ON_TCP_DISCONNECT, args, IngressEgress::default())
                    .and_then(|((), _meta)| future::ok(()))
                    .map_err(|e| log::warn!("error: {}", e)),
            )
        } else {
            Box::pin(future::ok(()))
        }
    }
}
