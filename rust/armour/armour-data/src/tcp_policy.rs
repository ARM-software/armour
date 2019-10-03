//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor};
use super::tcp_proxy;
use super::{Stop, ToArmourExpression};
use actix::prelude::*;
use armour_data_interface::{codec, policy};
use armour_policy::{expressions, lang};
use futures::{future, Future};
use std::sync::Arc;

pub struct TcpPolicy {
    connect: lang::Policy,
    disconnect: lang::Policy,
    debug: bool,
    program: Arc<lang::Program>,
    proxy: Option<(u16, Addr<tcp_proxy::TcpDataServer>)>,
}

impl Policy<Addr<tcp_proxy::TcpDataServer>> for TcpPolicy {
    fn start(&mut self, port: u16, proxy: Addr<tcp_proxy::TcpDataServer>) {
        self.stop();
        self.proxy = Some((port, proxy))
    }
    fn stop(&mut self) -> bool {
        if let Some((_, ref server)) = self.proxy {
            server.do_send(Stop);
            self.proxy = None;
            true
        } else {
            false
        }
    }
    fn set_debug(&mut self, b: bool) {
        self.debug = b
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.connect = p.policy(policy::ALLOW_TCP_CONNECTION);
        self.disconnect = p.policy(policy::ON_TCP_DISCONNECT);
        self.program = Arc::new(p)
    }
    fn deny_all(&mut self) {
        self.set_policy(lang::Program::deny_all(&policy::TCP_POLICY).unwrap())
    }
    fn allow_all(&mut self) {
        self.set_policy(lang::Program::allow_all(&policy::TCP_POLICY).unwrap())
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|(p, _)| *p)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn debug(&self) -> bool {
        self.debug
    }
    fn status(&self) -> Box<codec::Status> {
        let policy = if self.program.is_allow_all() {
            codec::Policy::AllowAll
        } else if self.program.is_deny_all() {
            codec::Policy::DenyAll
        } else {
            codec::Policy::Program((*self.policy()).clone())
        };
        Box::new(codec::Status {
            port: self.port(),
            debug: self.debug(),
            policy,
        })
    }
}

impl Default for TcpPolicy {
    fn default() -> Self {
        TcpPolicy {
            connect: lang::Policy::default(),
            disconnect: lang::Policy::default(),
            debug: false,
            program: Arc::new(lang::Program::deny_all(&policy::TCP_POLICY).unwrap()),
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
        let GetTcpPolicy(from, to) = msg;
        match self.tcp.connect {
            lang::Policy::Allow => {
                self.connection_number += 1;
                Box::new(future::ok(TcpPolicyStatus::Allow(Box::new(None))))
            }
            lang::Policy::Deny => {
                info!("deny");
                Box::new(future::ok(TcpPolicyStatus::Block))
            }
            lang::Policy::Args(n) if n == 2 || n == 3 => {
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
                        .evaluate(policy::ALLOW_TCP_CONNECTION, args)
                        .and_then(move |res| {
                            future::ok(if res {
                                TcpPolicyStatus::Allow(Box::new(Some(connection)))
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
    pub from: expressions::Expr,
    pub to: expressions::Expr,
    pub number: expressions::Expr,
}

impl ConnectionStats {
    pub fn new(
        from: &expressions::Expr,
        to: &expressions::Expr,
        number: &expressions::Expr,
    ) -> ConnectionStats {
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
        if let lang::Policy::Args(arg_count) = self.tcp.disconnect {
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
