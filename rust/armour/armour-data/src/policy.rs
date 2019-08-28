//! actix-web support for Armour policies
use super::{http_proxy, tcp_proxy, Stop};
use actix::prelude::*;
use actix_web::web;
use armour_data_interface::{PolicyCodec, PolicyRequest, PolicyResponse};
use armour_policy::{lang, literals};
use futures::{future, Future};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

/// Armour policy actor
///
/// Currently, a "policy" is just an Armour program with "require", "client_payload" and "server_payload" functions.
pub struct DataPolicy {
    /// policy program
    program: Arc<lang::Program>,
    allow_all: bool,
    debug: bool,
    timeout: std::time::Duration,
    connection_number: usize,
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, PolicyCodec>,
    // proxies
    http_proxies: HashMap<u16, Box<actix_web::dev::Server>>,
    tcp_proxies: HashMap<u16, Addr<tcp_proxy::TcpDataServer>>,
}

impl DataPolicy {
    fn default_policy() -> Arc<lang::Program> {
        Arc::new(lang::Program::default())
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
                        debug: false,
                        timeout: std::time::Duration::from_secs(5),
                        connection_number: 0,
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
        self.program = Arc::new(p);
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
        let now = if self.debug {
            debug!(r#"evaluting "{}"""#, function);
            Some(std::time::Instant::now())
        } else {
            None
        };
        Box::new(
            lang::Expr::call(function, args)
                .evaluate(self.program.clone())
                .and_then(move |result| match result {
                    // lang::Expr::LitExpr(literals::Literal::Policy(policy)) => {
                    //     if let Some(elapsed) = now.map(|t| t.elapsed()) {
                    //         debug!("result is: {:?} ({:?})", policy, elapsed)
                    //     };
                    //     future::ok(policy == literals::Policy::Accept)
                    // }
                    lang::Expr::LitExpr(literals::Literal::Bool(accept)) => {
                        if let Some(elapsed) = now.map(|t| t.elapsed()) {
                            debug!("result is: {} ({:?})", accept, elapsed)
                        };
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

/// Internal proxy message for getting policy information
pub struct GetPolicy;

impl Message for GetPolicy {
    type Result = Policy;
}

#[derive(MessageResponse)]
pub struct Policy {
    pub allow_all: bool,
    pub debug: bool,
    pub timeout: std::time::Duration,
    pub connection_number: usize,
    pub require: Option<u8>,
    pub client_payload: Option<u8>,
    pub server_payload: Option<u8>,
}

// handle request to get current policy status information
impl Handler<GetPolicy> for DataPolicy {
    type Result = Policy;

    fn handle(&mut self, _msg: GetPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let connection_number = self.connection_number;
        self.connection_number += 1;
        if self.allow_all {
            Policy {
                allow_all: true,
                debug: self.debug,
                timeout: self.timeout,
                connection_number,
                require: None,
                client_payload: None,
                server_payload: None,
            }
        } else {
            let program = &self.program;
            Policy {
                allow_all: false,
                debug: self.debug,
                timeout: self.timeout,
                connection_number,
                require: program.arg_count("require"),
                client_payload: program.arg_count("client_payload"),
                server_payload: program.arg_count("server_payload"),
            }
        }
    }
}

type VExpr = Vec<lang::Expr>;

/// Internal proxy message for requesting function evaluation over the policy
pub enum Evaluate {
    Require(VExpr),
    ClientPayload(VExpr),
    ServerPayload(VExpr),
}

impl Message for Evaluate {
    type Result = Result<bool, lang::Error>;
}

// handle requests to evaluate the Armour policy
impl Handler<Evaluate> for DataPolicy {
    type Result = Box<dyn Future<Item = bool, Error = lang::Error>>;

    fn handle(&mut self, msg: Evaluate, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            Evaluate::Require(args) => self.evaluate_policy("require", args),
            Evaluate::ClientPayload(args) => self.evaluate_policy("client_payload", args),
            Evaluate::ServerPayload(args) => self.evaluate_policy("server_payload", args),
        }
    }
}

// TCP connection policies
pub struct ConnectPolicy(pub std::net::SocketAddr, pub std::net::SocketAddr);

pub enum ConnectionPolicy {
    Allow(Box<ConnectionStats>),
    Block,
}

impl Message for ConnectPolicy {
    type Result = Result<ConnectionPolicy, lang::Error>;
}

impl Handler<ConnectPolicy> for DataPolicy {
    type Result = Box<dyn Future<Item = ConnectionPolicy, Error = lang::Error>>;

    fn handle(&mut self, msg: ConnectPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            ConnectPolicy(from, to) => {
                if self.allow_all {
                    self.connection_number += 1;
                    Box::new(future::ok(ConnectionPolicy::Block))
                } else {
                    match self.program.arg_count("allow_connection") {
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
                            Box::new(self.evaluate_policy("allow_connection", args).and_then(
                                move |res| {
                                    future::ok(if res {
                                        ConnectionPolicy::Allow(Box::new(connection))
                                    } else {
                                        ConnectionPolicy::Block
                                    })
                                },
                            ))
                        }
                        _ => Box::new(future::ok(ConnectionPolicy::Block)),
                    }
                }
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
impl Handler<ConnectionStats> for DataPolicy {
    type Result = Box<dyn Future<Item = (), Error = ()>>;

    fn handle(&mut self, msg: ConnectionStats, _ctx: &mut Context<Self>) -> Self::Result {
        let arg_count = if self.allow_all {
            0
        } else {
            self.program.arg_count("after_connection").unwrap_or(0)
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
                self.evaluate_policy("after_connection", args)
                    .map_err(|e| log::warn!("error: {}", e))
                    .map(|_| ()),
            )
        } else {
            Box::new(future::ok(()))
        }
    }
}

// handle messages from the data plane master
impl Handler<PolicyRequest> for DataPolicy {
    type Result = ();

    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            PolicyRequest::Timeout(secs) => {
                self.timeout = std::time::Duration::from_secs(secs.into());
                info!("timout: {:?}", self.timeout)
            }
            PolicyRequest::Debug(debug) => {
                self.debug = debug;
                info!("debug: {}", debug)
            }
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
                        server.do_send(Stop);
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
                    server.do_send(Stop);
                    self.uds_framed.write(PolicyResponse::Stopped);
                    info!("stopped TCP proxy on port {}", port)
                }
            }
            PolicyRequest::Start(config) => {
                let port = config.port;
                match http_proxy::start_proxy(ctx.address(), config) {
                    Ok(server) => {
                        self.http_proxies.insert(port, Box::new(server));
                        self.uds_framed.write(PolicyResponse::Started)
                    }
                    Err(err) => {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        warn!("failed to start proxy on port {}: {}", port, err)
                    }
                }
            }
            PolicyRequest::StartTcp(port) => match tcp_proxy::start_proxy(port, ctx.address()) {
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

// implement Actor trait for DataPolicy
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

// convert payloads into Armour-language expressions
impl ToArmourExpression for usize {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::i64(*self as i64)
    }
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

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::Bytes {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::BytesMut {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

// convert socket addresses into Armour-language expressions (of type ID)
impl ToArmourExpression for std::net::SocketAddr {
    fn to_expression(&self) -> lang::Expr {
        let mut id = literals::ID::default();
        let ip = self.ip();
        if ip.is_ipv4() {
            id = id.add_ip(ip)
        };
        id = id.set_port(self.port());
        if let Ok(host) = dns_lookup::lookup_addr(&ip) {
            id = id.add_host(&host)
        }
        lang::Expr::id(id)
    }
}

impl ToArmourExpression for Option<std::net::SocketAddr> {
    fn to_expression(&self) -> lang::Expr {
        if let Some(addr) = self {
            addr.to_expression()
        } else {
            lang::Expr::id(literals::ID::default())
        }
    }
}

// convert URLs into Armour-language expressions (of type ID)
impl ToArmourExpression for url::Url {
    fn to_expression(&self) -> lang::Expr {
        let mut id = literals::ID::default();
        if let Some(host) = self.host_str() {
            id = id.add_host(host);
            if let Ok(ips) = dns_lookup::lookup_host(host) {
                for ip in ips.iter().filter(|ip| ip.is_ipv4()) {
                    id = id.add_ip(*ip)
                }
            }
        }
        if let Some(port) = self.port() {
            id = id.set_port(port)
        } else {
            match self.scheme() {
                "https" => id = id.set_port(443),
                "http" => id = id.set_port(80),
                s => log::debug!("scheme is: {}", s),
            }
        }
        lang::Expr::id(id)
    }
}
