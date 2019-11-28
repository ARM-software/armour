//! actix-web support for Armour policies
use super::{http_policy::RestPolicy, http_proxy, tcp_policy::TcpPolicy, tcp_proxy};
use actix::prelude::*;
use actix_web::http::uri;
use armour_data_interface::codec::{PolicyCodec, PolicyRequest, PolicyResponse, Protocol, Status};
use armour_policy::{expressions, lang, literals};
use futures::{future, Future};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

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
    if let Some(scheme) = u.scheme_part() {
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
    fn start(&mut self, port: u16, proxy: P);
    fn stop(&mut self) -> bool;
    fn set_debug(&mut self, _: bool);
    fn set_policy(&mut self, p: lang::Program);
    fn port(&self) -> Option<u16>;
    fn policy(&self) -> Arc<lang::Program>;
    fn debug(&self) -> bool;
    fn status(&self) -> Box<Status>;
    fn evaluate<T: std::convert::TryFrom<literals::Literal> + 'static>(
        &self,
        function: &str,
        args: Vec<expressions::Expr>,
    ) -> Box<dyn Future<Item = T, Error = expressions::Error>> {
        let now = if self.debug() {
            info!(r#"evaluting "{}""#, function);
            Some(std::time::Instant::now())
        } else {
            None
        };
        Box::new(
            expressions::Expr::call(function, args)
                .evaluate(self.policy())
                .and_then(move |result| {
                    if let Some(elapsed) = now.map(|t| t.elapsed()) {
                        info!("result: {}", result);
                        info!("evaluate time: {:?}", elapsed)
                    };
                    match result {
                        expressions::Expr::LitExpr(lit) => {
                            if let Ok(r) = lit.try_into() {
                                future::ok(r)
                            } else {
                                future::err(expressions::Error::new("literal has wrong type"))
                            }
                        }
                        _ => future::err(expressions::Error::new("did not evaluate to a literal")),
                    }
                }),
        )
    }
}

/// Armour policy actor
///
/// Currently, a "policy" is an Armour program with a set of standard functions:
/// - `allow_rest_request`
/// - `allow_client_payload`
/// - `allow_server_payload`
/// - `allow_tcp_connection`
/// - `on_tcp_disconnect`
pub struct PolicyActor {
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, PolicyCodec>,
    pub connection_number: usize,
    // proxies
    pub http: RestPolicy,
    pub tcp: TcpPolicy,
    ids_uri: HashMap<actix_web::http::uri::Uri, literals::ID>,
    ids_ip: HashMap<std::net::IpAddr, literals::ID>,
}

// implement Actor trait for PolicyActor
impl Actor for PolicyActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("started Armour policy")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.uds_framed.write(PolicyResponse::ShuttingDown);
        info!("stopped Armour policy")
    }
}

impl PolicyActor {
    /// Start a new policy actor that connects to a data plane master on a Unix socket.
    pub fn create_policy<P: AsRef<std::path::Path>>(
        master_socket: P,
    ) -> std::io::Result<Addr<PolicyActor>> {
        tokio_uds::UnixStream::connect(master_socket)
            .and_then(|stream| {
                let addr = PolicyActor::create(|ctx| {
                    let (r, w) = stream.split();
                    ctx.add_stream(FramedRead::new(r, PolicyCodec));
                    PolicyActor {
                        connection_number: 0,
                        uds_framed: actix::io::FramedWrite::new(w, PolicyCodec, ctx),
                        http: RestPolicy::default(),
                        tcp: TcpPolicy::default(),
                        ids_uri: HashMap::new(),
                        ids_ip: HashMap::new(),
                    }
                });
                future::ok(addr)
            })
            .wait()
    }
    fn id(&mut self, id: ID) -> literals::ID {
        match id {
            ID::Anonymous => literals::ID::default(),
            ID::Uri(u) => self.ids_uri.get(&u).cloned().unwrap_or_else(|| {
                let mut port = u.port_u16();
                if port.is_none() {
                    port = scheme_port(&u)
                }
                let id = literals::ID::from((u.host(), port));
                self.ids_uri.insert(u, id.clone());
                id
            }),
            ID::SocketAddr(s) => {
                let ip = s.ip();
                self.ids_ip
                    .get(&ip)
                    .map(|id| id.set_port(s.port()))
                    .unwrap_or_else(|| {
                        let id = literals::ID::from(s);
                        self.ids_ip.insert(ip, id.clone());
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

impl StreamHandler<PolicyRequest, std::io::Error> for PolicyActor {
    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) {
        // pass on message to regular handler
        ctx.notify(msg)
    }
    fn finished(&mut self, _ctx: &mut Context<Self>) {
        info!("lost connection to master");
        System::current().stop();
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
            PolicyRequest::Debug(Protocol::Rest, debug) => {
                self.http.set_debug(debug);
                info!("REST debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::TCP, debug) => {
                self.tcp.set_debug(debug);
                info!("TCP debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::All, debug) => {
                ctx.notify(PolicyRequest::Debug(Protocol::Rest, debug));
                ctx.notify(PolicyRequest::Debug(Protocol::TCP, debug))
            }
            PolicyRequest::Status => self.uds_framed.write(PolicyResponse::Status {
                http: self.http.status(),
                tcp: self.tcp.status(),
            }),
            PolicyRequest::Stop(Protocol::Rest) => {
                if self.http.stop() {
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped REST proxy")
                } else {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!("there is no REST proxy to stop")
                }
            }
            PolicyRequest::Stop(Protocol::TCP) => {
                if self.tcp.stop() {
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped TCP proxy on port")
                } else {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!("there is no TCP proxy to stop")
                }
            }
            PolicyRequest::Stop(Protocol::All) => {
                ctx.notify(PolicyRequest::Stop(Protocol::Rest));
                ctx.notify(PolicyRequest::Stop(Protocol::TCP))
            }
            PolicyRequest::StartHttp(config) => {
                let port = config.port;
                match http_proxy::start_proxy(ctx.address(), config) {
                    Ok(server) => {
                        self.http.start(port, server);
                        self.uds_framed.write(PolicyResponse::Started)
                    }
                    Err(err) => {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        warn!("failed to start REST proxy on port {}: {}", port, err)
                    }
                }
            }
            PolicyRequest::StartTcp(port) => match tcp_proxy::start_proxy(port, ctx.address()) {
                Ok(server) => {
                    self.tcp.start(port, server);
                    self.uds_framed.write(PolicyResponse::Started)
                }
                Err(err) => {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!("failed to start TCP proxy on port {}: {}", port, err)
                }
            },
            PolicyRequest::SetPolicy(Protocol::Rest, prog) => {
                self.http.set_policy(prog);
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed REST policy")
            }
            PolicyRequest::SetPolicy(Protocol::TCP, prog) => {
                self.tcp.set_policy(prog);
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed TCP policy")
            }
            PolicyRequest::SetPolicy(Protocol::All, prog) => {
                ctx.notify(PolicyRequest::SetPolicy(Protocol::Rest, prog.clone()));
                ctx.notify(PolicyRequest::SetPolicy(Protocol::TCP, prog))
            }
            PolicyRequest::Shutdown => System::current().stop(),
        }
    }
}
