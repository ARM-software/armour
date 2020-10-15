//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor, ID};
use actix::prelude::*;
use armour_api::host::Status;
use armour_lang::{
    expressions,
    interpret::DPEnv,
    literals,
    meta::IngressEgress,
    policies::{self, FnPolicy, Protocol},
};
use futures::future::{self, TryFutureExt};
use std::boxed::Box;
use std::sync::Arc;

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct PolicyStatus {
    pub timeout: std::time::Duration,
    pub request: FnPolicy,
    pub response: FnPolicy,
    allow_all: bool,
}

impl PolicyStatus {
    fn update_for_policy(&mut self, policy: &policies::Policy) {
        self.request = policy
            .get(policies::ALLOW_REST_REQUEST)
            .cloned()
            .unwrap_or_default();
        self.response = policy
            .get(policies::ALLOW_REST_RESPONSE)
            .cloned()
            .unwrap_or_default();
        self.allow_all = self.request == FnPolicy::Allow && self.response == FnPolicy::Allow
    }
}

impl Default for PolicyStatus {
    fn default() -> Self {
        PolicyStatus {
            timeout: std::time::Duration::from_secs(5),
            allow_all: false,
            request: FnPolicy::default(),
            response: FnPolicy::default(),
        }
    }
}

struct HttpProxy {
    server: actix_web::dev::Server,
    port: u16,
    ingress: Option<std::net::SocketAddrV4>,
}

impl HttpProxy {
    fn new(
        server: actix_web::dev::Server,
        port: u16,
        ingress: Option<std::net::SocketAddrV4>,
    ) -> Self {
        HttpProxy {
            server,
            port,
            ingress,
        }
    }
}

pub struct HttpPolicy {
    policy: Arc<policies::Policy>,
    env: DPEnv,
    proxy: Option<HttpProxy>,
    status: PolicyStatus,
}

impl Policy<(actix_web::dev::Server, Option<std::net::SocketAddrV4>)> for HttpPolicy {
    fn start(
        &mut self,
        server_config: (actix_web::dev::Server, Option<std::net::SocketAddrV4>),
        port: u16,
    ) {
        self.proxy = Some(HttpProxy::new(server_config.0, port, server_config.1))
    }
    #[allow(unused_must_use)]
    fn stop(&mut self) {
        if let Some(proxy) = &self.proxy {
            log::info!("stopping HTTP proxy on port {}", proxy.port);
            proxy.server.stop(true);
        };
        self.proxy = None
    }
    fn set_policy(&mut self, p: policies::Policy) {
        self.status.update_for_policy(&p);
        self.policy = Arc::new(p);
        self.env = DPEnv::new(&self.policy.program)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.port)
    }
    fn policy(&self) -> Arc<policies::Policy> {
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
            ingress: self.proxy.as_ref().map(|p| p.ingress).flatten(),
        })
    }
}

impl Default for HttpPolicy {
    fn default() -> Self {
        let policy = Arc::new(policies::Policy::deny_all(Protocol::HTTP));
        let env = DPEnv::new(&policy.program);
        HttpPolicy {
            policy,
            env,
            proxy: None,
            status: PolicyStatus::default(),
        }
    }
}

impl HttpPolicy {
    fn get(&self) -> PolicyStatus {
        self.status.clone()
    }
    pub fn set_timeout(&mut self, secs: u8) {
        self.status.timeout = std::time::Duration::from_secs(secs.into())
    }
}

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct HttpPolicyResponse {
    pub status: PolicyStatus,
    pub connection: literals::DPConnection,
}

/// Request REST policy information
pub struct GetHttpPolicy(pub (ID, ID));

impl Message for GetHttpPolicy {
    type Result = HttpPolicyResponse;
}

// handle request to get current policy status information
impl Handler<GetHttpPolicy> for PolicyActor {
    type Result = HttpPolicyResponse;

    fn handle(&mut self, msg: GetHttpPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let status = self.http.get();
        if status.allow_all {
            HttpPolicyResponse {
                status,
                connection: literals::Connection::default(),
            }
        } else {
            let from_to = msg.0;
            HttpPolicyResponse {
                status,
                connection: self.connection(from_to.0, from_to.1),
            }
        }
    }
}

/// REST policy functions
pub enum HttpFn {
    Request,
    Response,
}

/// Request evaluation of a (HTTP) policy function
#[derive(Message)]
#[rtype(result = "Result<(bool, Option<String>), expressions::Error>")]
pub struct EvalHttpFn(pub HttpFn, pub Vec<expressions::DPExpr>, pub Option<String>);

// handle requests to evaluate the Armour policy
impl Handler<EvalHttpFn> for PolicyActor {
    type Result = ResponseFuture<Result<(bool, Option<String>), expressions::Error>>;

    fn handle(&mut self, msg: EvalHttpFn, _ctx: &mut Context<Self>) -> Self::Result {
        let function = match msg.0 {
            HttpFn::Request => policies::ALLOW_REST_REQUEST,
            HttpFn::Response => policies::ALLOW_REST_RESPONSE,
        };
        // try to decrypt ingress metadata
        let ingress_meta = msg
            .2
            .map(|xarmour| PolicyActor::decrypt_meta(&self.aead, &xarmour))
            .flatten();
        let meta = IngressEgress::new(ingress_meta, self.label.clone());
        let aead = self.aead.clone();
        Box::pin(
            self.http
                .evaluate(function, msg.1, meta)
                .and_then(move |(b, meta)| {
                    let encrypted = PolicyActor::encrypt_meta(&aead, meta);
                    // if let Some(e) = encrypted.as_ref() {
                    //     log::debug!("meta is: {:?}", PolicyActor::decrypt_meta(&aead, e))
                    // }
                    future::ok((b, encrypted))
                }),
        )
    }
}
