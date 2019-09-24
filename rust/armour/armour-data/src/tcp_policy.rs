//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor};
use super::tcp_proxy;
use super::{Stop, ToArmourExpression};
use actix::prelude::*;
use armour_data_interface as interface;
use armour_policy::lang;
use futures::{future, Future};
use std::sync::Arc;

pub struct TcpPolicy {
    program: Arc<lang::Program>,
    allow_all: bool,
    debug: bool,
    proxy: Option<(u16, Addr<tcp_proxy::TcpDataServer>)>,
}

impl Policy<Addr<tcp_proxy::TcpDataServer>> for TcpPolicy {
    fn start(&mut self, port: u16, proxy: Addr<tcp_proxy::TcpDataServer>) {
        self.stop();
        self.proxy = Some((port, proxy))
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|(p, _)| *p)
    }
    fn proxy(&self) -> Option<Addr<tcp_proxy::TcpDataServer>> {
        self.proxy.as_ref().map(|(_, s)| s.clone())
    }
    fn stop(&mut self) -> bool {
        if let Some(server) = self.proxy() {
            server.do_send(Stop);
            self.proxy = None;
            true
        } else {
            false
        }
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.program = Arc::new(p);
        self.allow_all = false
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn set_debug(&mut self, b: bool) {
        self.debug = b
    }
    fn debug(&self) -> bool {
        self.debug
    }
    fn deny_all(&mut self) {
        self.set_policy(lang::Program::default());
        self.allow_all = false
    }
    fn allow_all(&mut self) {
        self.set_policy(lang::Program::default());
        self.allow_all = true;
    }
    fn is_allow_all(&self) -> bool {
        self.allow_all
    }
    fn is_deny_all(&self) -> bool {
        !(self.allow_all || self.program.has_function(interface::ALLOW_TCP_CONNECTION))
    }
    fn status(&self) -> Box<interface::Status> {
        let policy = if self.is_allow_all() {
            interface::Policy::AllowAll
        } else if self.is_deny_all() {
            interface::Policy::DenyAll
        } else {
            interface::Policy::Program((*self.policy()).clone())
        };
        Box::new(interface::Status {
            port: self.port(),
            debug: self.debug(),
            policy,
        })
    }
}

impl Default for TcpPolicy {
    fn default() -> Self {
        TcpPolicy {
            program: Arc::new(lang::Program::default()),
            allow_all: false,
            debug: false,
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
    type Result = Result<TcpPolicyStatus, lang::Error>;
}

impl Handler<GetTcpPolicy> for PolicyActor {
    type Result = Box<dyn Future<Item = TcpPolicyStatus, Error = lang::Error>>;

    fn handle(&mut self, msg: GetTcpPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let GetTcpPolicy(from, to) = msg;
        if self.tcp.is_allow_all() {
            self.connection_number += 1;
            Box::new(future::ok(TcpPolicyStatus::Allow(Box::new(None))))
        } else {
            match self.tcp.policy().arg_count(interface::ALLOW_TCP_CONNECTION) {
                Some(n) if n == 2 || n == 3 => {
                    let connection_number = self.connection_number;
                    self.connection_number += 1;
                    let from = from.to_expression();
                    let to = to.to_expression();
                    let number = connection_number.to_expression();
                    if let (Some(from), Some(to)) = (from.host(), to.host()) {
                        info!(r#"checking connection from "{}" to "{}""#, from, to)
                    }
                    let connection = ConnectionStats::new(&from, &to, &number);
                    let args = match n {
                        2 => vec![from, to],
                        _ => vec![from, to, number],
                    };
                    Box::new(
                        self.tcp
                            .evaluate(interface::ALLOW_TCP_CONNECTION, args)
                            .and_then(move |res| {
                                future::ok(if res {
                                    TcpPolicyStatus::Allow(Box::new(Some(connection)))
                                } else {
                                    TcpPolicyStatus::Block
                                })
                            }),
                    )
                }
                _ => Box::new(future::ok(TcpPolicyStatus::Block)),
            }
        }
    }
}

#[derive(Clone)]
pub struct ConnectionStats {
    pub sent: usize,
    pub received: usize,
    pub from: lang::Expr,
    pub to: lang::Expr,
    pub number: lang::Expr,
}

impl ConnectionStats {
    pub fn new(from: &lang::Expr, to: &lang::Expr, number: &lang::Expr) -> ConnectionStats {
        ConnectionStats {
            sent: 0,
            received: 0,
            from: from.clone(),
            to: to.clone(),
            number: number.clone(),
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
        let arg_count = if self.tcp.is_allow_all() {
            0
        } else {
            self.tcp
                .policy()
                .arg_count(interface::ON_TCP_DISCONNECT)
                .unwrap_or(0)
        };
        if 2 <= arg_count && arg_count <= 5 {
            let args = match arg_count {
                2 => vec![msg.from, msg.to],
                3 => vec![msg.from, msg.to, msg.number],
                5 => vec![
                    msg.from,
                    msg.to,
                    msg.number,
                    msg.sent.to_expression(),
                    msg.received.to_expression(),
                ],
                _ => unreachable!(),
            };
            Box::new(
                self.tcp
                    .evaluate(interface::ON_TCP_DISCONNECT, args)
                    .map_err(|e| log::warn!("error: {}", e)),
            )
        } else {
            Box::new(future::ok(()))
        }
    }
}
