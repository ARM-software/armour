//! actix-web support for Armour policies
use super::{http_policy::HttpPolicy, http_proxy, tcp_policy::TcpPolicy, tcp_proxy};
use actix::prelude::*;
use actix_web::http::uri;
use armour_api::master::{PolicyResponse, Status};
use armour_api::proxy::{self, LabelOp, PolicyCodec, PolicyRequest, Protocol};
use armour_lang::{
    expressions,
    interpret::Env,
    labels, lang, literals,
    meta::{IngressEgress, Meta},
};
use futures::future::{BoxFuture, FutureExt};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryInto;
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio_util::codec::FramedRead;

// Trait for managing proxies and their associated policies (implemented for HTTP and TCP protocols)
pub trait Policy<P> {
    fn start(&mut self, proxy: P, port: u16);
    fn stop(&mut self);
    fn set_debug(&mut self, _: bool);
    fn set_policy(&mut self, p: lang::Program);
    fn port(&self) -> Option<u16>;
    fn policy(&self) -> Arc<lang::Program>;
    fn hash(&self) -> String;
    fn env(&self) -> &Env;
    fn debug(&self) -> bool;
    fn status(&self) -> Box<Status>;
    fn evaluate<T: std::convert::TryFrom<literals::Literal> + Send + 'static>(
        &self,
        function: &'static str,
        args: Vec<expressions::Expr>,
        meta: IngressEgress,
    ) -> BoxFuture<'static, Result<(T, Option<Meta>), expressions::Error>> {
        let now = if self.debug() {
            log::info!(r#"evaluting "{}""#, function);
            // log::info!(r#"args "{:?}""#, args);
            Some(std::time::Instant::now())
        } else {
            None
        };
        let mut env = self.env().clone();
        env.set_meta(meta);
        async move {
            let result = expressions::Expr::call(function, args)
                .evaluate(env.clone())
                .await?;
            let meta = env.egress().await;
            if let Some(elapsed) = now.map(|t| t.elapsed()) {
                log::info!("result: {}", result);
                log::info!("evaluate time: {:?}", elapsed)
            };
            if let expressions::Expr::LitExpr(lit) = result {
                if let Ok(r) = lit.try_into() {
                    // log::info!("meta is: {:?}", meta);
                    Ok((r, meta))
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

type Aead = aes_gcm::Aes256Gcm;

/// Armour policy actor
#[allow(dead_code)] // TODO
pub struct PolicyActor {
    pub label: labels::Label,
    pub connection_number: usize,
    // proxies
    pub http: HttpPolicy,
    pub tcp: TcpPolicy,
    // authenticated encryption with associated data (for metadata)
    pub aead: Aead,
    // ID information
    identity: Identity,
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio::net::UnixStream>, PolicyCodec>,
}

// implement Actor trait for PolicyActor
impl Actor for PolicyActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        // send a connection message to the data plane master
        self.uds_framed.write(PolicyResponse::Connect(
            std::process::id(),
            self.label.clone(),
            self.http.hash(),
            self.tcp.hash(),
        ));
        log::info!("started Armour policy actor")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("stopped Armour policy")
    }
}

impl PolicyActor {
    /// Start a new policy actor that connects to a data plane master on a Unix socket.
    pub fn create_policy(
        stream: tokio::net::UnixStream,
        label: labels::Label,
        key: [u8; 32],
    ) -> Addr<PolicyActor> {
        use aead::{generic_array::GenericArray, NewAead};
        use aes_gcm::Aes256Gcm;
        PolicyActor::create(|ctx| {
            let (r, w) = tokio::io::split(stream);
            ctx.add_stream(FramedRead::new(r, PolicyCodec));
            PolicyActor {
                label,
                connection_number: 0,
                http: HttpPolicy::default(),
                tcp: TcpPolicy::default(),
                aead: Aes256Gcm::new(GenericArray::clone_from_slice(&key)),
                identity: Identity::default(),
                uds_framed: actix::io::FramedWrite::new(w, PolicyCodec, ctx),
            }
        })
    }
    fn nonce() -> Option<Vec<u8>> {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|t| [&b"armo"[..4], &t.as_nanos().to_be_bytes()[8..]].concat())
            .ok()
    }
    fn encrypt(aead: &Aead, message: &str) -> Option<String> {
        use aead::{generic_array::GenericArray, Aead};
        let nonce = PolicyActor::nonce()?;
        let mut block = message.to_string().into_bytes();
        if aead
            .encrypt_in_place(GenericArray::from_slice(&nonce), &[], &mut block)
            .is_ok()
        {
            Some(format!(
                "{};{}",
                base64::encode(&block),
                base64::encode(&nonce)
            ))
        } else {
            None
        }
    }
    fn decrypt(aead: &Aead, message: &str) -> Option<Vec<u8>> {
        use aead::{generic_array::GenericArray, Aead};
        let res: Result<Vec<Vec<u8>>, base64::DecodeError> =
            message.split(';').map(base64::decode).collect();
        match res.ok()?.as_slice() {
            [block, nonce] => {
                let mut block: Vec<u8> = block.to_vec();
                if aead
                    .decrypt_in_place(GenericArray::from_slice(&nonce), &[], &mut block)
                    .is_ok()
                {
                    Some(block)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    pub fn encrypt_meta(aead: &Aead, meta: Option<Meta>) -> Option<String> {
        meta.as_ref()
            .map(|m| {
                serde_json::to_string(m)
                    .map(|s| PolicyActor::encrypt(aead, &s))
                    .ok()
                    .flatten()
            })
            .flatten()
    }
    pub fn decrypt_meta(aead: &Aead, message: &str) -> Option<Meta> {
        let bytes = PolicyActor::decrypt(aead, message)?;
        serde_json::from_slice(&bytes).ok()
    }
}
// identity/connection management
impl PolicyActor {
    fn id(&mut self, id: ID) -> literals::ID {
        match id {
            ID::Anonymous => literals::ID::default(),
            ID::Uri(u) => {
                let labels = self.identity.uri_labels.get(&u);
                // log::debug!("creating ID for {} with labels {:?}", u, labels);
                if let Some(id) = self.identity.uri_cache.get(&u) {
                    id.clone()
                } else {
                    let mut port = u.port_u16();
                    if port.is_none() {
                        port = scheme_port(&u)
                    }
                    let id = literals::ID::from((u.host(), port, labels));
                    self.identity.uri_cache.insert(u, id.clone());
                    id
                }
            }
            ID::SocketAddr(s) => {
                let ip = s.ip();
                let labels = self.identity.ip_labels.get(&ip);
                if let Some(id) = self.identity.ip_cache.get(&ip) {
                    id.set_port(s.port())
                } else {
                    let id = literals::ID::from((s, labels));
                    self.identity.ip_cache.insert(ip, id.clone());
                    id
                }
            }
        }
    }
    // performance critical (computes IDs, which could involve DNS lookup)
    pub fn connection(&mut self, from: ID, to: ID) -> literals::Connection {
        let number = self.connection_number;
        self.connection_number += 1;
        // let now = std::time::Instant::now();
        // log::info!("now: {:?}", now.elapsed());
        literals::Connection::from((&self.id(from), &self.id(to), number))
    }
}

#[derive(Default)]
struct Identity {
    /// map from IDs to Armour label set expressions
    uri_labels: HashMap<uri::Uri, labels::Labels>,
    ip_labels: HashMap<std::net::IpAddr, labels::Labels>,
    uri_cache: HashMap<uri::Uri, literals::ID>,
    ip_cache: HashMap<std::net::IpAddr, literals::ID>,
}

impl Identity {
    fn clear_labels(&mut self) {
        self.uri_labels.clear();
        self.ip_labels.clear();
        self.uri_cache.clear();
        self.ip_cache.clear()
    }
    fn add_uri(&mut self, uri: uri::Uri, label: labels::Label) {
        if let Some(labels) = self.uri_labels.get_mut(&uri) {
            labels.insert(label);
        } else {
            let mut labels = BTreeSet::new();
            labels.insert(label);
            self.uri_labels.insert(uri, labels);
        }
        self.uri_cache.clear()
    }
    fn add_ip(&mut self, ip: std::net::IpAddr, label: labels::Label) {
        if let Some(labels) = self.ip_labels.get_mut(&ip) {
            labels.insert(label);
        } else {
            let mut labels = BTreeSet::new();
            labels.insert(label);
            self.ip_labels.insert(ip, labels);
        }
        self.ip_cache.clear()
    }
    fn remove_uri(&mut self, uri: &uri::Uri, label: Option<labels::Label>) {
        if let Some(label) = label {
            if let Some(labels) = self.uri_labels.get_mut(uri) {
                if labels.remove(&label) && labels.is_empty() {
                    self.uri_labels.remove(uri);
                }
            }
        } else {
            self.uri_labels.remove(uri);
        }
        self.uri_cache.clear()
    }
    fn remove_ip(&mut self, ip: &std::net::IpAddr, label: Option<labels::Label>) {
        if let Some(label) = label {
            if let Some(labels) = self.ip_labels.get_mut(ip) {
                if labels.remove(&label) && labels.is_empty() {
                    self.ip_labels.remove(ip);
                }
            }
        } else {
            self.ip_labels.remove(ip);
        }
        self.ip_cache.clear()
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ID {
    Uri(actix_web::http::uri::Uri),
    SocketAddr(std::net::SocketAddr),
    Anonymous,
}

impl ID {
    fn authority(uri: uri::Uri) -> uri::Uri {
        if let Some(auth) = uri.authority() {
            if let Ok(uri) = uri::Builder::new().authority(auth.clone()).build() {
                uri
            } else {
                uri
            }
        } else {
            uri
        }
    }
}

impl From<uri::Uri> for ID {
    fn from(uri: uri::Uri) -> Self {
        ID::Uri(ID::authority(uri))
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

impl actix::io::WriteHandler<std::io::Error> for PolicyActor {}

impl StreamHandler<Result<PolicyRequest, std::io::Error>> for PolicyActor {
    fn handle(&mut self, msg: Result<PolicyRequest, std::io::Error>, ctx: &mut Context<Self>) {
        // pass on message to regular handler
        if let Ok(request) = msg {
            ctx.notify(request)
        }
    }
    fn finished(&mut self, _ctx: &mut Context<Self>) {
        log::info!("lost connection to master");
        System::current().stop()
    }
}

// handle messages from the data plane master
impl Handler<PolicyRequest> for PolicyActor {
    type Result = ();

    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            PolicyRequest::Label(op) => self.handle_label_op(op),
            PolicyRequest::Timeout(secs) => {
                self.http
                    .set_timeout(std::time::Duration::from_secs(secs.into()));
                log::info!("timeout: {:?}", secs)
            }
            PolicyRequest::Debug(Protocol::HTTP, debug) => {
                self.http.set_debug(debug);
                log::info!("HTTP debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::TCP, debug) => {
                self.tcp.set_debug(debug);
                log::info!("TCP debug: {}", debug)
            }
            PolicyRequest::Debug(Protocol::All, debug) => {
                ctx.notify(PolicyRequest::Debug(Protocol::HTTP, debug));
                ctx.notify(PolicyRequest::Debug(Protocol::TCP, debug))
            }
            PolicyRequest::Status => {
                self.uds_framed.write(PolicyResponse::Status {
                    label: self.label.clone(),
                    http: self.http.status(),
                    tcp: self.tcp.status(),
                });
            }
            PolicyRequest::Stop(Protocol::HTTP) => {
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
                ctx.notify(PolicyRequest::Stop(Protocol::HTTP));
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
                            log::warn!("failed to start HTTP proxy")
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
                            log::warn!("failed to start TCP proxy")
                        };
                        async {}.into_actor(act)
                    })
                    .wait(ctx)
            }
            PolicyRequest::SetPolicy(policy) => match policy {
                proxy::Policy::AllowAll(Protocol::All) => {
                    self.install_http_allow_all();
                    self.install_tcp_allow_all()
                }
                proxy::Policy::AllowAll(Protocol::HTTP) => self.install_http_allow_all(),
                proxy::Policy::AllowAll(Protocol::TCP) => self.install_tcp_allow_all(),
                proxy::Policy::DenyAll(Protocol::All) => {
                    self.install_http_deny_all();
                    self.install_tcp_deny_all()
                }
                proxy::Policy::DenyAll(Protocol::HTTP) => self.install_http_deny_all(),
                proxy::Policy::DenyAll(Protocol::TCP) => self.install_tcp_deny_all(),
                proxy::Policy::Program(prog) => match prog.protocol().as_str() {
                    "http" => self.install_http(prog),
                    "tcp" => self.install_tcp(prog),
                    s => {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        log::info!("failed to install policy, unrecognized protocol: {}", s)
                    }
                },
            },
            PolicyRequest::Shutdown => {
                log::info!("shutting down");
                self.uds_framed.write(PolicyResponse::ShuttingDown);
                self.uds_framed.close();
                System::current().stop()
            }
        }
    }
}

// label management
impl PolicyActor {
    // convert ID labels into exportable structure
    fn labels(&self) -> BTreeMap<String, labels::Labels> {
        let mut labels = BTreeMap::new();
        for (k, v) in self.identity.uri_labels.iter() {
            labels.insert(k.to_string(), v.clone());
        }
        for (k, v) in self.identity.ip_labels.iter() {
            labels.insert(k.to_string(), v.clone());
        }
        labels
    }
    // convert url::Url into uri::Uri by taking just the domain and port (when available)
    fn url_to_uri(url: url::Url) -> Option<uri::Uri> {
        if let Some(domain) = url.domain() {
            let s = if let Some(port) = url.port_or_known_default() {
                format!("{}:{}", domain, port)
            } else {
                domain.to_string()
            };
            s.parse::<http::uri::Uri>().ok()
        } else {
            None
        }
    }
    fn handle_label_op(&mut self, op: LabelOp) {
        match op {
            LabelOp::AddUrl(url, label) => {
                if let Some(uri) = PolicyActor::url_to_uri(url) {
                    log::info!("adding label for: {}", uri);
                    self.identity.add_uri(uri, label)
                } else {
                    log::warn!("failed to convert URL to Uri");
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                }
            }
            LabelOp::AddIp(ip, label) => {
                let ip = ip.into();
                log::info!("adding label for: {}", ip);
                self.identity.add_ip(ip, label)
            }
            LabelOp::RemoveUrl(url, label) => {
                if let Some(uri) = PolicyActor::url_to_uri(url) {
                    log::info!("removing label for: {}", uri);
                    self.identity.remove_uri(&uri, label)
                } else {
                    log::warn!("failed to convert URL to Uri");
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                }
            }
            LabelOp::RemoveIp(ip, label) => {
                let ip = ip.into();
                log::info!("removing label for: {}", ip);
                self.identity.remove_ip(&ip, label)
            }
            LabelOp::Clear => {
                log::info!("clearing all labels");
                self.identity.clear_labels()
            }
            LabelOp::List => self.uds_framed.write(PolicyResponse::Labels(self.labels())),
        }
    }
}

// install policies
impl PolicyActor {
    fn install_http(&mut self, prog: lang::Program) {
        let hash = prog.blake3_string();
        self.http.set_policy(prog);
        self.uds_framed.write(PolicyResponse::UpdatedPolicy(
            armour_api::proxy::Protocol::HTTP,
            hash,
        ));
        log::info!("installed HTTP policy")
    }
    fn install_tcp(&mut self, prog: lang::Program) {
        let hash = prog.blake3_string();
        self.tcp.set_policy(prog);
        self.uds_framed.write(PolicyResponse::UpdatedPolicy(
            armour_api::proxy::Protocol::TCP,
            hash,
        ));
        log::info!("installed TCP policy")
    }
    fn install_http_allow_all(&mut self) {
        if let Ok(prog) = lang::Program::allow_all(&lang::HTTP_POLICY) {
            self.install_http(prog)
        } else {
            self.uds_framed.write(PolicyResponse::RequestFailed);
            log::info!("failed to install HTTP allow all policy")
        }
    }
    fn install_tcp_allow_all(&mut self) {
        if let Ok(prog) = lang::Program::allow_all(&lang::TCP_POLICY) {
            self.install_tcp(prog)
        } else {
            self.uds_framed.write(PolicyResponse::RequestFailed);
            log::info!("failed to install TCP allow all policy")
        }
    }
    fn install_http_deny_all(&mut self) {
        if let Ok(prog) = lang::Program::deny_all(&lang::HTTP_POLICY) {
            self.install_http(prog)
        } else {
            self.uds_framed.write(PolicyResponse::RequestFailed);
            log::info!("failed to install HTTP deny all policy")
        }
    }
    fn install_tcp_deny_all(&mut self) {
        if let Ok(prog) = lang::Program::deny_all(&lang::TCP_POLICY) {
            self.install_tcp(prog)
        } else {
            self.uds_framed.write(PolicyResponse::RequestFailed);
            log::info!("failed to install TCP deny all policy")
        }
    }
}
