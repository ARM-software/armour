//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor, ID};
use actix::prelude::*;
use armour_api::master::Status;
use armour_lang::{
    expressions,
    interpret::Env,
    lang, literals,
    meta::{IngressEgress, Meta},
};
use futures::future::{self, TryFutureExt};
use std::boxed::Box;
use std::sync::Arc;

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct PolicyStatus {
    pub timeout: std::time::Duration,
    pub request: lang::Policy,
    pub response: lang::Policy,
    allow_all: bool,
}

impl PolicyStatus {
    fn update_for_policy(&mut self, prog: &lang::Program) {
        self.request = prog.policy(lang::ALLOW_REST_REQUEST);
        self.response = prog.policy(lang::ALLOW_REST_RESPONSE);
        self.allow_all = self.request == lang::Policy::Allow && self.response == lang::Policy::Allow
    }
}

impl Default for PolicyStatus {
    fn default() -> Self {
        PolicyStatus {
            timeout: std::time::Duration::from_secs(5),
            allow_all: false,
            request: lang::Policy::default(),
            response: lang::Policy::default(),
        }
    }
}

pub struct HttpPolicy {
    program: Arc<lang::Program>,
    env: Env,
    proxy: Option<(actix_web::dev::Server, u16)>,
    status: PolicyStatus,
}

impl Policy<actix_web::dev::Server> for HttpPolicy {
    fn start(&mut self, server: actix_web::dev::Server, port: u16) {
        self.proxy = Some((server, port))
    }
    #[allow(unused_must_use)]
    fn stop(&mut self) {
        if let Some((server, port)) = &self.proxy {
            log::info!("stopping HTTP proxy on port {}", port);
            server.stop(true);
        };
        self.proxy = None
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.status.update_for_policy(&p);
        self.program = Arc::new(p);
        self.env = Env::new(&self.program)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.1)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn hash(&self) -> String {
        self.program.blake3_string()
    }
    fn env(&self) -> &Env {
        &self.env
    }
    fn status(&self) -> Box<Status> {
        Box::new(Status {
            port: self.port(),
            policy: (*self.policy()).clone(),
        })
    }
}

impl Default for HttpPolicy {
    fn default() -> Self {
        let program = Arc::new(lang::Program::deny_all(&lang::HTTP_POLICY).unwrap_or_default());
        HttpPolicy {
            program: program.clone(),
            env: Env::new(&program),
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
    pub connection: literals::Connection,
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
pub struct EvalHttpFn(pub HttpFn, pub Vec<expressions::Expr>, pub Option<String>);

// handle requests to evaluate the Armour policy
impl Handler<EvalHttpFn> for PolicyActor {
    type Result = ResponseFuture<Result<(bool, Option<String>), expressions::Error>>;

    fn handle(&mut self, msg: EvalHttpFn, _ctx: &mut Context<Self>) -> Self::Result {
        let function = match msg.0 {
            HttpFn::Request => lang::ALLOW_REST_REQUEST,
            HttpFn::Response => lang::ALLOW_REST_RESPONSE,
        };
        // try to decrypt ingress metadata
        let ingress_meta = msg
            .2
            .map(|xarmour| PolicyActor::decrypt_meta(&self.aead, &xarmour))
            .flatten();
        let meta = IngressEgress::new(ingress_meta, Some(Meta::new(self.label.clone())));
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
