//! actix-web support for Armour policies
use super::{http_policy::RestPolicy, http_proxy, tcp_policy::TcpPolicy, tcp_proxy};
use actix::prelude::*;
use actix_web::http::uri;
use armour_api::master::{PolicyResponse, Status};
use armour_api::proxy::{PolicyCodec, PolicyRequest, Protocol};
use armour_lang::{expressions, interpret::Env, lang, literals};
use futures::future::{BoxFuture, FutureExt};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio_util::codec::FramedRead;

#[derive(Clone, Debug)]
pub enum ID {
    Uri(actix_web::http::uri::Uri),
    SocketAddr(std::net::SocketAddr),
    Anonymous,
}

impl From<actix_web::http::uri::Uri> for ID {
    fn from(uri: actix_web::http::uri::Uri) -> Self {
        ID::Uri(uri)
    }
}

impl From<Option<std::net::SocketAddr>> for ID {
    fn from(sock: Option<std::net::SocketAddr>) -> Self {
        if let Some(s) = sock {
            ID::SocketAddr(s)
        } else {
            ID::Anonymous
        }
    }
}

fn scheme_port(u: &uri::Uri) -> Option<u16> {
    if let Some(scheme) = u.scheme() {
        if *scheme == actix_web::http::uri::Scheme::HTTP {
            Some(80)
        } else if *scheme == actix_web::http::uri::Scheme::HTTPS {
            Some(443)
        } else {
            None
        }
    } else {
        None
    }
}

pub trait Policy<P> {
    fn start(&mut self, proxy: P, port: u16);
    fn stop(&mut self);
    fn set_debug(&mut self, _: bool);
    fn set_policy(&mut self, p: lang::Program);
    fn port(&self) -> Option<u16>;
    fn policy(&self) -> Arc<lang::Program>;
    fn env(&self) -> Arc<Env>;
    fn debug(&self) -> bool;
    fn status(&self) -> Box<Status>;
    fn evaluate<T: std::convert::TryFrom<literals::Literal> + Send + 'static>(
        &self,
        function: &'static str,
        args: Vec<expressions::Expr>,
    ) -> BoxFuture<'static, Result<T, expressions::Error>> {
        let now = if self.debug() {
            info!(r#"evaluting "{}""#, function);
            Some(std::time::Instant::now())
        } else {
            None
        };
        let env = self.env();
        async move {
            let result = expressions::Expr::call(function, args)
                .evaluate(env)
                .await?;
            if let Some(elapsed) = now.map(|t| t.elapsed()) {
                info!("result: {}", result);
                info!("evaluate time: {:?}", elapsed)
            };
            if let expressions::Expr::LitExpr(lit) = result {
                if let Ok(r) = lit.try_into() {
                    Ok(r)
                } else {
                    Err(expressions::Error::new("literal has wrong type"))
                }
            } else {
                Err(expressions::Error::new("did not evaluate to a literal"))
            }
        }
        .boxed()
    }
}

/// Armour policy actor
pub struct PolicyActor {
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio::net::UnixStream>, PolicyCodec>,
    pub connection_number: usize,
    // proxies
    pub http: RestPolicy,
    pub tcp: TcpPolicy,
    id_uri_cache: HashMap<actix_web::http::uri::Uri, literals::ID>,
    id_ip_cache: HashMap<std::net::IpAddr, literals::ID>,
}

// implement Actor trait for PolicyActor
impl Actor for PolicyActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        self.uds_framed
            .write(PolicyResponse::Connect(std::process::id()));
        info!("started Armour policy actor")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("stopped Armour policy")
    }
}

impl PolicyActor {
    /// Start a new policy actor that connects to a data plane master on a Unix socket.
    pub fn create_policy(stream: tokio::net::UnixStream) -> Addr<PolicyActor> {
        PolicyActor::create(|ctx| {
            let (r, w) = tokio::io::split(stream);
            ctx.add_stream(FramedRead::new(r, PolicyCodec));
            PolicyActor {
                connection_number: 0,
                uds_framed: actix::io::FramedWrite::new(w, PolicyCodec, ctx),
                http: RestPolicy::default(),
                tcp: TcpPolicy::default(),
                id_uri_cache: HashMap::new(),
                id_ip_cache: HashMap::new(),
            }
        })
    }
    fn id(&mut self, id: ID) -> literals::ID {
        match id {
            ID::Anonymous => literals::ID::default(),
            ID::Uri(u) => self.id_uri_cache.get(&u).cloned().unwrap_or_else(|| {
                let mut port = u.port_u16();
                if port.is_none() {
                    port = scheme_port(&u)
                }
                let id = literals::ID::from((u.host(), port));
                self.id_uri_cache.insert(u, id.clone());
                id
            }),
            ID::SocketAddr(s) => {
                let ip = s.ip();
                self.id_ip_cache
                    .get(&ip)
                    .map(|id| id.set_port(s.port()))
                    .unwrap_or_else(|| {
                        let id = literals::ID::from(s);
                        self.id_ip_cache.insert(ip, id.clone());
                        id
                    })
            }
        }
    }
    // performance critical (computes IDs, which could involve DNS lookup)
    pub fn connection(&mut self, from: ID, to: ID) -> literals::Connection {
        let number = self.connection_number;
        self.connection_number += 1;
        // let now = std::time::Instant::now();
        // info!("now: {:?}", now.elapsed());
        literals::Connection::from((&self.id(from), &self.id(to), number))
    }
}

impl actix::io::WriteHandler<std::io::Error> for PolicyActor {}

impl StreamHandler<Result<PolicyRequest, std::io::Error>> for PolicyActor {
    fn handle(&mut self, msg: Result<PolicyRequest, std::io::Error>, ctx: &mut Context<Self>) {
        // pass on message to regular handler
        if let Ok(request) = msg {
            ctx.notify(request)
        }
    }
    fn finished(&mut self, _ctx: &mut Context<Self>) {
        info!("lost connection to master");
        System::current().stop()
    }
}

// handle messages from the data plane master
impl Handler<PolicyRequest> for PolicyActor {
    type Result = ();

    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            PolicyRequest::Timeout(secs) => {
                self.http
                    .set_timeout(std::time::Duration::from_secs(secs.into()));
                info!("timeout: {:?}", secs)
            }
            PolicyRequest::Debug(Protocol::REST, debug) => {
                self.http.set_debug(debug);
                info!("REST debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::TCP, debug) => {
                self.tcp.set_debug(debug);
                info!("TCP debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::All, debug) => {
                ctx.notify(PolicyRequest::Debug(Protocol::REST, debug));
                ctx.notify(PolicyRequest::Debug(Protocol::TCP, debug))
            }
            PolicyRequest::Status => {
                self.uds_framed.write(PolicyResponse::Status {
                    http: self.http.status(),
                    tcp: self.tcp.status(),
                });
            }
            PolicyRequest::Stop(Protocol::REST) => {
                if self.http.port().is_none() {
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                } else {
                    self.http.stop();
                    self.uds_framed.write(PolicyResponse::Stopped)
                }
            }
            PolicyRequest::Stop(Protocol::TCP) => {
                if self.tcp.port().is_none() {
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                } else {
                    self.tcp.stop();
                    self.uds_framed.write(PolicyResponse::Stopped)
                }
            }
            PolicyRequest::Stop(Protocol::All) => {
                ctx.notify(PolicyRequest::Stop(Protocol::REST));
                ctx.notify(PolicyRequest::Stop(Protocol::TCP))
            }
            PolicyRequest::StartHttp(port) => {
                self.http.stop();
                http_proxy::start_proxy(ctx.address(), port)
                    .into_actor(self)
                    .then(move |server, act, _ctx| {
                        if let Ok(server) = server {
                            act.http.start(server, port);
                            act.uds_framed.write(PolicyResponse::Started)
                        } else {
                            // TODO: show error and port
                            warn!("failed to start HTTP proxy")
                        };
                        async {}.into_actor(act)
                    })
                    .wait(ctx)
            }
            PolicyRequest::StartTcp(port) => {
                self.tcp.stop();
                tcp_proxy::start_proxy(port, ctx.address())
                    .into_actor(self)
                    .then(move |server, act, _ctx| {
                        if let Ok(server) = server {
                            act.tcp.start(server, port);
                            act.uds_framed.write(PolicyResponse::Started)
                        } else {
                            // TODO: show error and port
                            warn!("failed to start TCP proxy")
                        };
                        async {}.into_actor(act)
                    })
                    .wait(ctx)
            }
            PolicyRequest::SetPolicy(Protocol::REST, prog) => {
                self.http.set_policy(prog);
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed HTTP policy")
            }
            PolicyRequest::SetPolicy(Protocol::TCP, prog) => {
                self.tcp.set_policy(prog);
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed TCP policy")
            }
            PolicyRequest::SetPolicy(Protocol::All, prog) => {
                ctx.notify(PolicyRequest::SetPolicy(Protocol::REST, prog.clone()));
                ctx.notify(PolicyRequest::SetPolicy(Protocol::TCP, prog))
            }
            PolicyRequest::Shutdown => {
                log::info!("shutting down");
                self.uds_framed.write(PolicyResponse::ShuttingDown);
                self.uds_framed.close();
                System::current().stop()
            }
        }
    }
}
