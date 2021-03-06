//! actix-web support for Armour policies
/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use super::policy::{Policy, PolicyActor, ID};
use super::tcp_proxy;
use super::Stop;
use actix::prelude::*;
use armour_api::host::Status;
use armour_lang::{
    expressions::{Error, DPExpr},
    interpret::DPEnv,
    meta::IngressEgress,
    policies::{self, FnPolicy, Protocol},
};
use futures::future::{self, TryFutureExt};
use std::sync::Arc;

pub struct TcpPolicy {
    connect: FnPolicy,
    disconnect: FnPolicy,
    policy: Arc<policies::DPPolicy>,
    env: DPEnv,
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
    fn set_policy(&mut self, p: policies::DPPolicy) {
        self.connect = p
            .get(policies::ALLOW_TCP_CONNECTION)
            .cloned()
            .unwrap_or_default();
        self.disconnect = p
            .get(policies::ON_TCP_DISCONNECT)
            .cloned()
            .unwrap_or_default();
        self.policy = Arc::new(p);
        self.env = DPEnv::new(&self.policy.program)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.1)
    }
    fn policy(&self) -> Arc<policies::DPPolicy> {
        self.policy.clone()
    }
    fn hash(&self) -> String {
        self.policy.blake3()
    }
    fn env(&self) -> &DPEnv {
        &self.env
    }
    fn status(&self) -> Box<Status> {
        Box::new(Status {
            port: self.port(),
            policy: (*self.policy()).clone(),
            ingress: None,
        })
    }
}

impl Default for TcpPolicy {
    fn default() -> Self {
        let policy = Arc::new(policies::DPPolicy::deny_all(Protocol::TCP));
        let env = DPEnv::new(&policy.program);
        TcpPolicy {
            connect: FnPolicy::default(),
            disconnect: FnPolicy::default(),
            policy,
            env,
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
        log::debug!("Handling TCP request at proxy: {}", self.label);
        match self.tcp.connect {
            FnPolicy::Allow => {
                self.connection_number += 1;
                Box::pin(future::ok(TcpPolicyStatus::Allow(Box::new(None))))
            }
            FnPolicy::Deny => {
                log::info!("deny");
                Box::pin(future::ok(TcpPolicyStatus::Block))
            }
            FnPolicy::Args(n) if n == 1 => {
                let connection = self
                    .connection(ID::SocketAddr(msg.0), ID::SocketAddr(msg.1))
                    .into();
                let stats = ConnectionStats::new(&connection);
                Box::pin(
                    self.tcp
                        .evaluate(
                            policies::ALLOW_TCP_CONNECTION,
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
    pub connection: DPExpr,
}

impl ConnectionStats {
    pub fn new(connection: &DPExpr) -> ConnectionStats {
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
        if let FnPolicy::Args(arg_count) = self.tcp.disconnect {
            let args = match arg_count {
                3 => vec![
                    msg.connection,
                    DPExpr::from(msg.sent),
                    DPExpr::from(msg.received),
                ],
                _ => unreachable!(), // policy is checked beforehand
            };
            Box::pin(
                self.tcp
                    .evaluate(policies::ON_TCP_DISCONNECT, args, IngressEgress::default())
                    .and_then(|((), _meta)| future::ok(()))
                    .map_err(|e| log::warn!("error: {}", e)),
            )
        } else {
            Box::pin(future::ok(()))
        }
    }
}
