//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor, ID};
use super::{tcp_proxy, Stop};
use actix::prelude::*;
use armour_data_interface::{codec, policy};
use armour_policy::{expressions, externals::Disconnector, interpret::Env, lang};
use expressions::Expr;
use futures::{future, Future};
use std::sync::Arc;

struct Proxy {
    env: Arc<Env>,
    port: u16,
}

pub struct TcpPolicy {
    connect: lang::Policy,
    disconnect: lang::Policy,
    debug: bool,
    program: Arc<lang::Program>,
    proxy: Option<Proxy>,
}

impl Policy<Addr<tcp_proxy::TcpDataServer>> for TcpPolicy {
    fn start(&mut self, env: Env, port: u16, addr: Addr<PolicyActor>) -> Option<Disconnector> {
        match tcp_proxy::start_proxy(port, addr) {
            Ok(server) => {
                let server_clone = server.clone();
                let fut = futures::lazy(move || server_clone.send(Stop).then(|_| future::ok(())));
                self.proxy = Some(Proxy {
                    env: Arc::new(env),
                    port,
                });
                Some(Box::new(fut))
            }
            Err(err) => {
                warn!("failed to start TCP proxy on port {}: {}", port, err);
                None
            }
        }
    }
    fn stop(&mut self) {
        self.proxy = None
    }
    fn set_debug(&mut self, b: bool) {
        self.debug = b
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.connect = p.policy(policy::ALLOW_TCP_CONNECTION);
        self.disconnect = p.policy(policy::ON_TCP_DISCONNECT);
        self.program = Arc::new(p)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.port)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn env(&self) -> Arc<Env> {
        self.proxy
            .as_ref()
            .map(|p| p.env.clone())
            .unwrap_or_default()
    }
    fn debug(&self) -> bool {
        self.debug
    }
    fn status(&self) -> Box<codec::Status> {
        Box::new(codec::Status {
            port: self.port(),
            debug: self.debug(),
            policy: (*self.policy()).clone(),
        })
    }
}

impl Default for TcpPolicy {
    fn default() -> Self {
        TcpPolicy {
            connect: lang::Policy::default(),
            disconnect: lang::Policy::default(),
            debug: false,
            program: Arc::new(lang::Program::default()),
            proxy: None,
        }
    }
}

// TCP connection policies
pub struct GetTcpPolicy(pub std::net::SocketAddr, pub std::net::SocketAddr);

pub enum TcpPolicyStatus {
    Allow(Box<Option<ConnectionStats>>),
    Block,
}

impl Message for GetTcpPolicy {
    type Result = Result<TcpPolicyStatus, expressions::Error>;
}

impl Handler<GetTcpPolicy> for PolicyActor {
    type Result = Box<dyn Future<Item = TcpPolicyStatus, Error = expressions::Error>>;

    fn handle(&mut self, msg: GetTcpPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        match self.tcp.connect {
            lang::Policy::Allow => {
                self.connection_number += 1;
                Box::new(future::ok(TcpPolicyStatus::Allow(Box::new(None))))
            }
            lang::Policy::Deny => {
                info!("deny");
                Box::new(future::ok(TcpPolicyStatus::Block))
            }
            lang::Policy::Args(n) if n == 1 => {
                let connection = self
                    .connection(ID::SocketAddr(msg.0), ID::SocketAddr(msg.1))
                    .into();
                let stats = ConnectionStats::new(&connection);
                Box::new(
                    self.tcp
                        .evaluate(policy::ALLOW_TCP_CONNECTION, vec![connection])
                        .and_then(move |res| {
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

#[derive(Clone)]
pub struct ConnectionStats {
    pub sent: usize,
    pub received: usize,
    pub connection: expressions::Expr,
}

impl ConnectionStats {
    pub fn new(connection: &expressions::Expr) -> ConnectionStats {
        ConnectionStats {
            sent: 0,
            received: 0,
            connection: connection.clone(),
        }
    }
}

impl Message for ConnectionStats {
    type Result = Result<(), ()>;
}

// sent by the TCP proxy when the TCP connection finishes
impl Handler<ConnectionStats> for PolicyActor {
    type Result = Box<dyn Future<Item = (), Error = ()>>;

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
            Box::new(
                self.tcp
                    .evaluate(policy::ON_TCP_DISCONNECT, args)
                    .map_err(|e| log::warn!("error: {}", e)),
            )
        } else {
            Box::new(future::ok(()))
        }
    }
}
