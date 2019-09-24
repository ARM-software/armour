//! actix-web support for Armour policies
use super::{http_policy::RestPolicy, http_proxy, tcp_policy::TcpPolicy, tcp_proxy};
use actix::prelude::*;
use armour_data_interface::{PolicyCodec, PolicyRequest, PolicyResponse, Protocol, Status};
use armour_policy::{lang, literals};
use futures::{future, Future};
use std::convert::TryInto;
use std::sync::Arc;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

pub trait Policy<P> {
    fn start(&mut self, port: u16, proxy: P);
    fn stop(&mut self) -> bool;
    fn port(&self) -> Option<u16>;
    fn proxy(&self) -> Option<P>;
    fn set_policy(&mut self, p: lang::Program);
    fn policy(&self) -> Arc<lang::Program>;
    fn set_debug(&mut self, _: bool);
    fn debug(&self) -> bool;
    fn deny_all(&mut self);
    fn allow_all(&mut self);
    fn is_allow_all(&self) -> bool;
    fn is_deny_all(&self) -> bool;
    fn status(&self) -> Box<Status>;
    fn evaluate<T: std::convert::TryFrom<literals::Literal> + 'static>(
        &self,
        function: &str,
        args: Vec<lang::Expr>,
    ) -> Box<dyn Future<Item = T, Error = lang::Error>> {
        let now = if self.debug() {
            debug!(r#"evaluting "{}""#, function);
            Some(std::time::Instant::now())
        } else {
            None
        };
        Box::new(
            lang::Expr::call(function, args)
                .evaluate(self.policy())
                .and_then(move |result| {
                    if let Some(elapsed) = now.map(|t| t.elapsed()) {
                        debug!("result: {:?}", result);
                        debug!("evaluate time: {:?}", elapsed)
                    };
                    match result {
                        lang::Expr::LitExpr(lit) => {
                            if let Ok(r) = lit.try_into() {
                                future::ok(r)
                            } else {
                                future::err(lang::Error::new("literal has wrong type"))
                            }
                        }
                        _ => future::err(lang::Error::new("did not evaluate to a literal")),
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
                    }
                });
                future::ok(addr)
            })
            .wait()
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
            PolicyRequest::SetPolicy(Protocol::Rest, policy) => {
                use armour_data_interface::Policy;
                match policy {
                    Policy::AllowAll => self.http.allow_all(),
                    Policy::DenyAll => self.http.deny_all(),
                    Policy::Program(prog) => self.http.set_policy(prog),
                }
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed REST policy")
            }
            PolicyRequest::SetPolicy(Protocol::TCP, policy) => {
                use armour_data_interface::Policy;
                match policy {
                    Policy::AllowAll => self.tcp.allow_all(),
                    Policy::DenyAll => self.tcp.deny_all(),
                    Policy::Program(prog) => self.tcp.set_policy(prog),
                }
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed TCP policy")
            }
            PolicyRequest::SetPolicy(Protocol::All, policy) => {
                ctx.notify(PolicyRequest::SetPolicy(Protocol::Rest, policy.clone()));
                ctx.notify(PolicyRequest::SetPolicy(Protocol::TCP, policy))
            }
            PolicyRequest::Shutdown => System::current().stop(),
        }
    }
}
