//! actix-web support for Armour policies
use super::{http_policy::HttpPolicy, http_proxy, tcp_policy::TcpPolicy, tcp_proxy};
use actix::prelude::*;
use actix_web::http::uri;
use armour_api::host::{PolicyResponse, Status};
use armour_api::proxy::{LabelOp, PolicyCodec, PolicyRequest};
use armour_lang::{
    expressions,
    interpret::Env,
    labels, literals,
    meta::{IngressEgress, Meta},
    policies::{self, Protocol},
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
    fn set_policy(&mut self, p: policies::Policy);
    fn port(&self) -> Option<u16>;
    fn policy(&self) -> Arc<policies::Policy>;
    fn hash(&self) -> String;
    fn env(&self) -> &Env;
    fn status(&self) -> Box<Status>;
    fn evaluate<T: std::convert::TryFrom<literals::Literal> + Send + 'static>(
        &self,
        function: &'static str,
        args: Vec<expressions::Expr>,
        meta: IngressEgress,
    ) -> BoxFuture<'static, Result<(T, Option<Meta>), expressions::Error>> {
        log::debug!(r#"evaluting "{}""#, function);
        let now = std::time::Instant::now();
        let mut env = self.env().clone();
        env.set_meta(meta);
        async move {
            let result = expressions::Expr::call(function, args)
                .evaluate(env.clone())
                .await?;
            let meta = env.egress().await;
            log::debug!("result ({:?}): {}", now.elapsed(), result);
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

// type Aead = aes_gcm::Aes256Gcm;
type Aead = chacha20poly1305::ChaChaPoly1305<chacha20::ChaCha20>;

/// Armour policy actor
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
    // connection to host
    uds_framed:
        actix::io::FramedWrite<PolicyResponse, WriteHalf<tokio::net::UnixStream>, PolicyCodec>,
}

// implement Actor trait for PolicyActor
impl Actor for PolicyActor {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        // send a connection message to the data plane host
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
    /// Start a new policy actor that connects to a data plane host on a Unix socket.
    pub fn create_policy(
        stream: tokio::net::UnixStream,
        label: labels::Label,
        timeout: u8,
        key: [u8; 32],
    ) -> Addr<PolicyActor> {
        use aead::{generic_array::GenericArray, NewAead};
        // use aes_gcm::Aes256Gcm;
        let mut http = HttpPolicy::default();
        http.set_timeout(timeout);
        PolicyActor::create(|ctx| {
            let (r, w) = tokio::io::split(stream);
            ctx.add_stream(FramedRead::new(r, PolicyCodec));
            PolicyActor {
                label,
                connection_number: 0,
                http,
                tcp: TcpPolicy::default(),
                // aead: Aes256Gcm::new(&GenericArray::clone_from_slice(&key)),
                aead: chacha20poly1305::ChaChaPoly1305::new(&GenericArray::clone_from_slice(&key)),
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
        use aead::{generic_array::GenericArray, AeadInPlace};
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
        use aead::{generic_array::GenericArray, AeadInPlace};
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
    fn get_port(u: &http::uri::Uri) -> Option<u16> {
        u.port_u16().or_else(|| scheme_port(&u))
    }
    fn id(&mut self, id: ID) -> literals::ID {
        match id {
            ID::Anonymous => literals::ID::default(),
            ID::Uri(u) => {
                if let Some(host) = u.host() {
                    if let Some(id) = self.identity.host_cache.get(host) {
                        id.clone()
                    } else {
                        let mut hosts = BTreeSet::new();
                        let mut ips = BTreeSet::new();
                        let mut labels: BTreeSet<labels::Label> = BTreeSet::new();
                        if let Some(lbls) = self.identity.host_labels.get(host) {
                            labels.extend(lbls.iter().cloned())
                        }
                        hosts.insert(host.to_string());
                        if let Ok(host_ips) = dns_lookup::lookup_host(host) {
                            for ip in host_ips.iter().filter(|ip| ip.is_ipv4()) {
                                if let Some(lbls) = self.identity.ip_labels.get(&ip) {
                                    labels.extend(lbls.iter().cloned())
                                }
                                ips.insert(*ip);
                            }
                        }
                        log::debug!("creating ID for {} with labels {:?}", u, labels);
                        let id = literals::ID::new(hosts, ips, PolicyActor::get_port(&u), labels);
                        self.identity
                            .host_cache
                            .insert(host.to_string(), id.clone());
                        id
                    }
                } else {
                    // failed to get host name, so ID can at best consist of port number
                    literals::ID::new(
                        BTreeSet::new(),
                        BTreeSet::new(),
                        PolicyActor::get_port(&u),
                        BTreeSet::new(),
                    )
                }
            }
            ID::SocketAddr(s) => {
                let ip = s.ip();
                let port = s.port();
                if let Some(id) = self.identity.ip_cache.get(&ip) {
                    id.set_port(port)
                } else {
                    let mut hosts = BTreeSet::new();
                    let mut ips = BTreeSet::new();
                    let mut labels: BTreeSet<labels::Label> = BTreeSet::new();
                    // DNS lookup, with addition of labels
                    if let Ok(host) = dns_lookup::lookup_addr(&ip) {
                        if let Some(lbls) = self.identity.host_labels.get(&host) {
                            labels.extend(lbls.iter().cloned())
                        }
                        hosts.insert(host);
                    }
                    if ip.is_ipv4() {
                        if let Some(lbls) = self.identity.ip_labels.get(&ip) {
                            labels.extend(lbls.iter().cloned())
                        }
                        ips.insert(ip);
                    }
                    log::debug!("creating ID for {} with labels {:?}", s, labels);
                    let id = literals::ID::new(hosts, ips, Some(port), labels);
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
    host_labels: HashMap<String, labels::Labels>,
    ip_labels: HashMap<std::net::IpAddr, labels::Labels>,
    host_cache: HashMap<String, literals::ID>,
    ip_cache: HashMap<std::net::IpAddr, literals::ID>,
}

impl Identity {
    fn clear_caches(&mut self) {
        self.host_cache.clear();
        self.ip_cache.clear()
    }
    fn clear_labels(&mut self) {
        self.host_labels.clear();
        self.ip_labels.clear();
        self.clear_caches()
    }
    fn add_uri(&mut self, uri: &uri::Uri, label: labels::Label) {
        if let Some(host) = uri.host() {
            if let Some(labels) = self.host_labels.get_mut(host) {
                labels.insert(label);
            } else {
                let mut labels = BTreeSet::new();
                labels.insert(label);
                self.host_labels.insert(host.to_string(), labels);
            }
            self.clear_caches()
        }
    }
    fn add_ip(&mut self, ip: std::net::IpAddr, label: labels::Label) {
        if let Some(labels) = self.ip_labels.get_mut(&ip) {
            labels.insert(label);
        } else {
            let mut labels = BTreeSet::new();
            labels.insert(label);
            self.ip_labels.insert(ip, labels);
        }
        self.clear_caches()
    }
    fn remove_uri(&mut self, uri: &uri::Uri, label: Option<labels::Label>) {
        if let Some(host) = uri.host() {
            if let Some(label) = label {
                if let Some(labels) = self.host_labels.get_mut(host) {
                    if labels.remove(&label) && labels.is_empty() {
                        self.host_labels.remove(host);
                    }
                }
            } else {
                self.host_labels.remove(host);
            }
            self.clear_caches()
        }
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
        self.clear_caches()
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
        log::info!("lost connection to host");
        System::current().stop()
    }
}

// handle messages from the data plane host
impl Handler<PolicyRequest> for PolicyActor {
    type Result = ();

    fn handle(&mut self, msg: PolicyRequest, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            PolicyRequest::Label(op) => self.handle_label_op(op),
            PolicyRequest::Timeout(secs) => {
                self.http.set_timeout(secs);
                log::info!("timeout: {:?}", secs)
            }
            PolicyRequest::Status => {
                self.uds_framed.write(PolicyResponse::Status {
                    label: self.label.clone(),
                    labels: self.labels(),
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
            PolicyRequest::StartHttp(config) => {
                let port = config.port();
                if let Some(current_port) = self.http.port() {
                    log::info!("HTTP proxy already started");
                    if port == current_port {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        return;
                    }
                }
                self.http.stop();
                http_proxy::start_proxy(ctx.address(), config.clone())
                    .into_actor(self)
                    .then(move |server, act, _ctx| {
                        if let Ok(server) = server {
                            act.http.start((server, config.ingress()), port);
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
                if let Some(current_port) = self.http.port() {
                    log::info!("TCP proxy already started");
                    if port == current_port {
                        self.uds_framed.write(PolicyResponse::RequestFailed);
                        return;
                    }
                }
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
            PolicyRequest::SetPolicy(policy) => {
                if let Some(tcp_policy) = policy.policy(Protocol::TCP) {
                    self.install_tcp(tcp_policy.clone())
                }
                if let Some(http_policy) = policy.policy(Protocol::HTTP) {
                    self.install_http(http_policy.clone())
                }
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

// label management
impl PolicyActor {
    // convert ID labels into exportable structure
    fn labels(&self) -> BTreeMap<String, labels::Labels> {
        let mut labels = BTreeMap::new();
        for (k, v) in self.identity.host_labels.iter() {
            labels.insert(k.to_string(), v.clone());
        }
        for (k, v) in self.identity.ip_labels.iter() {
            labels.insert(k.to_string(), v.clone());
        }
        labels
    }
    fn handle_label_op(&mut self, op: LabelOp) {
        match op {
            LabelOp::AddUri(uri_labels) => {
                for (s, labels) in uri_labels {
                    match s.parse::<http::uri::Uri>() {
                        Ok(uri) => {
                            for label in labels {
                                log::info!("adding label for: {}", uri);
                                self.identity.add_uri(&uri, label)
                            }
                        }
                        Err(err) => {
                            log::warn!("failed to parse URI: {}", err);
                            self.uds_framed.write(PolicyResponse::RequestFailed)
                        }
                    }
                }
            }
            LabelOp::AddIp(ip_labels) => {
                for (ip, labels) in ip_labels {
                    let ip = ip.into();
                    for label in labels {
                        log::info!("adding label for: {}", ip);
                        self.identity.add_ip(ip, label)
                    }
                }
            }
            LabelOp::RemoveUri(s, label) => match s.parse::<http::uri::Uri>() {
                Ok(uri) => {
                    log::info!("removing label for: {}", uri);
                    self.identity.remove_uri(&uri, label)
                }
                Err(err) => {
                    log::warn!("failed to parse URI: {}", err);
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                }
            },
            LabelOp::RemoveIp(ip, label) => {
                let ip = ip.into();
                log::info!("removing label for: {}", ip);
                self.identity.remove_ip(&ip, label)
            }
            LabelOp::Clear => {
                log::info!("clearing all labels");
                self.identity.clear_labels()
            }
        }
    }
}

// install policies
impl PolicyActor {
    fn install_http(&mut self, policy: policies::Policy) {
        let hash = policy.blake3();
        self.http.set_policy(policy);
        self.uds_framed
            .write(PolicyResponse::UpdatedPolicy(Protocol::HTTP, hash));
        log::info!("installed HTTP policy")
    }
    fn install_tcp(&mut self, policy: policies::Policy) {
        let hash = policy.blake3();
        self.tcp.set_policy(policy);
        self.uds_framed
            .write(PolicyResponse::UpdatedPolicy(Protocol::TCP, hash));
        log::info!("installed TCP policy")
    }
}
