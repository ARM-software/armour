//! actix-web support for Armour policies
use super::{http_proxy, tcp_proxy};
use actix::prelude::*;
use actix_web::web;
use arm_policy::{lang, literals};
use armour_data_interface::{PolicyCodec, PolicyRequest, PolicyResponse};
use futures::{future, Future};
use literals::ToLiteral;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

/// Armour policy actor
///
/// Currently, a "policy" is just an Armour program with "require", "client_payload" and "server_payload" functions.
pub struct DataPolicy {
    /// policy program
    program: Arc<Mutex<lang::Program>>,
    allow_all: bool,
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, PolicyCodec>,
    // proxies
    http_proxies: HashMap<u16, Box<actix_web::dev::Server>>,
    tcp_proxies: HashMap<u16, Addr<tcp_proxy::TcpDataServer>>,
}

impl DataPolicy {
    fn default_policy() -> Arc<Mutex<lang::Program>> {
        Arc::new(Mutex::new(lang::Program::default()))
    }
    /// Start a new policy actor that connects to a data plane master on a Unix socket.
    pub fn create_policy<P: AsRef<std::path::Path>>(
        master_socket: P,
    ) -> std::io::Result<Addr<DataPolicy>> {
        tokio_uds::UnixStream::connect(master_socket)
            .and_then(|stream| {
                let addr = DataPolicy::create(|ctx| {
                    let (r, w) = stream.split();
                    ctx.add_stream(FramedRead::new(r, PolicyCodec));
                    DataPolicy {
                        program: DataPolicy::default_policy(),
                        allow_all: false,
                        uds_framed: actix::io::FramedWrite::new(w, PolicyCodec, ctx),
                        http_proxies: HashMap::new(),
                        tcp_proxies: HashMap::new(),
                    }
                });
                future::ok(addr)
            })
            .wait()
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.program = Arc::new(Mutex::new(p));
        self.allow_all = false
    }
    fn deny_all_policy(&mut self) {
        self.program = DataPolicy::default_policy();
        self.allow_all = false
    }
    fn allow_all_policy(&mut self) {
        self.program = DataPolicy::default_policy();
        self.allow_all = true
    }
    fn evaluate_policy(
        &self,
        function: &str,
        args: Vec<lang::Expr>,
    ) -> Box<dyn Future<Item = bool, Error = lang::Error>> {
        let now = std::time::Instant::now();
        info!(r#"evaluting "{}"""#, function);
        Box::new(
            lang::Expr::call(function, args)
                .evaluate(Arc::new(self.program.lock().unwrap().clone()))
                .and_then(move |result| match result {
                    lang::Expr::LitExpr(literals::Literal::Policy(policy)) => {
                        info!("result is: {:?} ({:?})", policy, now.elapsed());
                        future::ok(policy == literals::Policy::Accept)
                    }
                    lang::Expr::LitExpr(literals::Literal::Bool(accept)) => {
                        info!("result is: {} ({:?})", accept, now.elapsed());
                        future::ok(accept)
                    }
                    _ => future::err(lang::Error::new(
                        "did not evaluate to a bool or policy literal",
                    )),
                }),
        )
    }
}

impl actix::io::WriteHandler<std::io::Error> for DataPolicy {}

impl StreamHandler<PolicyRequest, std::io::Error> for DataPolicy {
    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) {
        // pass on message to regular handler
        ctx.notify(msg)
    }
    fn finished(&mut self, _ctx: &mut Context<Self>) {
        info!("lost connection to master");
        System::current().stop();
    }
}

/// Internal proxy message for checking if a policy function exits
pub struct Check;

impl Message for Check {
    type Result = Policy;
}

#[derive(MessageResponse)]
pub enum Policy {
    AllowAll,
    Policy {
        require: bool,
        client_payload: bool,
        server_payload: bool,
    },
}

impl Handler<Check> for DataPolicy {
    type Result = Policy;

    fn handle(&mut self, _msg: Check, _ctx: &mut Context<Self>) -> Self::Result {
        if self.allow_all {
            Policy::AllowAll
        } else {
            let p = self.program.lock().unwrap();
            match (
                p.has_function("require"),
                p.has_function("client_payload"),
                p.has_function("server_payload"),
            ) {
                (require, client_payload, server_payload) => Policy::Policy {
                    require,
                    client_payload,
                    server_payload,
                },
            }
        }
    }
}

/// Internal proxy message for requesting function evaluation over the policy
pub enum Evaluate {
    Require(lang::Expr, lang::Expr),
    ClientPayload(lang::Expr),
    ServerPayload(lang::Expr),
}

impl Message for Evaluate {
    type Result = Result<bool, lang::Error>;
}

impl Handler<Evaluate> for DataPolicy {
    type Result = Box<dyn Future<Item = bool, Error = lang::Error>>;

    fn handle(&mut self, msg: Evaluate, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            Evaluate::Require(arg1, arg2) => self.evaluate_policy("require", vec![arg1, arg2]),
            Evaluate::ClientPayload(arg) => self.evaluate_policy("client_payload", vec![arg]),
            Evaluate::ServerPayload(arg) => self.evaluate_policy("server_payload", vec![arg]),
        }
    }
}

impl Handler<PolicyRequest> for DataPolicy {
    type Result = ();

    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            PolicyRequest::QueryActivePorts => {
                let http: HashSet<u16> = self
                    .http_proxies
                    .keys()
                    .map(|port| port.to_owned())
                    .collect();
                let tcp: HashSet<u16> = self
                    .tcp_proxies
                    .keys()
                    .map(|port| port.to_owned())
                    .collect();
                self.uds_framed
                    .write(PolicyResponse::ActivePorts { http, tcp })
            }
            PolicyRequest::Stop(port) => match self.http_proxies.get(&port) {
                Some(server) => {
                    server.stop(true); // graceful stop
                    self.http_proxies.remove(&port);
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped proxy on port {}", port)
                }
                None => match self.tcp_proxies.get(&port) {
                    Some(server) => {
                        server.do_send(tcp_proxy::Stop);
                        self.tcp_proxies.remove(&port);
                        self.uds_framed.write(PolicyResponse::Stopped);
                        info!("stopped TCP proxy on port {}", port)
                    }
                    None => {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        warn!("there is no proxy to stop on port {}", port)
                    }
                },
            },
            PolicyRequest::StopAll => {
                for (port, server) in self.http_proxies.drain() {
                    server.stop(true); // graceful stop
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped proxy on port {}", port)
                }
                for (port, server) in self.tcp_proxies.drain() {
                    server.do_send(tcp_proxy::Stop);
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped TCP proxy on port {}", port)
                }
            }
            PolicyRequest::Start(port) => match http_proxy::start_proxy(ctx.address(), port) {
                Ok(server) => {
                    self.http_proxies.insert(port, Box::new(server));
                    self.uds_framed.write(PolicyResponse::Started)
                }
                Err(err) => {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!("failed to start proxy on port {}: {}", port, err)
                }
            },
            PolicyRequest::StartTcp(port, socket) => match tcp_proxy::start_proxy(port, socket) {
                Ok(server) => {
                    self.tcp_proxies.insert(port, server);
                    self.uds_framed.write(PolicyResponse::Started)
                }
                Err(err) => {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!("failed to start TCP proxy on port {}: {}", port, err)
                }
            },
            // Attempt to load a new policy from a file
            PolicyRequest::UpdateFromFile(p) => match lang::Program::check_from_file(
                p.as_path(),
                &*armour_data_interface::POLICY_SIG,
            ) {
                Ok(prog) => {
                    self.set_policy(prog);
                    self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                    info!(
                        "installed policy: \"{}\"",
                        p.to_str().unwrap_or("<unknown>")
                    )
                }
                Err(e) => {
                    self.uds_framed.write(PolicyResponse::RequestFailed);
                    warn!(r#"{:?}: {}"#, p, e)
                }
            },
            PolicyRequest::UpdateFromData(prog) => {
                self.set_policy(prog);
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("installed policy from data")
            }
            PolicyRequest::AllowAll => {
                self.allow_all_policy();
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("switched to allow all policy")
            }
            PolicyRequest::DenyAll => {
                self.deny_all_policy();
                self.uds_framed.write(PolicyResponse::UpdatedPolicy);
                info!("switched to deny all policy")
            }
            PolicyRequest::Shutdown => System::current().stop(),
        }
    }
}

impl Actor for DataPolicy {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("started Armour policy")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.uds_framed.write(PolicyResponse::ShuttingDown);
        info!("stopped Armour policy")
    }
}

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_expression(&self) -> lang::Expr;
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for web::HttpRequest {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::http_request(literals::HttpRequest::from((
            self.method().as_str(),
            format!("{:?}", self.version()).as_str(),
            self.path(),
            self.query_string(),
            self.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
        )))
    }
}

impl ToArmourExpression for web::Bytes {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

impl ToArmourExpression for web::BytesMut {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

impl ToArmourExpression for Option<std::net::SocketAddr> {
    fn to_expression(&self) -> lang::Expr {
        match self {
            Some(std::net::SocketAddr::V4(addr)) => lang::Expr::some(addr.ip().to_literal()),
            _ => lang::Expr::none(),
        }
    }
}
