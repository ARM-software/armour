//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor, ID};
use actix::prelude::*;
use armour_data_interface::{codec, policy};
use armour_policy::{expressions, externals::Disconnector, interpret::Env, lang, literals};
use futures::Future;
use std::sync::Arc;

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct PolicyStatus {
    pub debug: bool,
    pub timeout: std::time::Duration,
    pub request: lang::Policy,
    pub client_payload: lang::Policy,
    pub server_payload: lang::Policy,
    pub response: lang::Policy,
}

impl PolicyStatus {
    fn update_for_policy(&mut self, prog: &lang::Program) {
        self.request = prog.policy(policy::ALLOW_REST_REQUEST);
        self.client_payload = prog.policy(policy::ALLOW_CLIENT_PAYLOAD);
        self.server_payload = prog.policy(policy::ALLOW_SERVER_PAYLOAD);
        self.response = prog.policy(policy::ALLOW_REST_RESPONSE)
    }
}

impl Default for PolicyStatus {
    fn default() -> Self {
        PolicyStatus {
            debug: false,
            timeout: std::time::Duration::from_secs(5),
            request: lang::Policy::default(),
            client_payload: lang::Policy::default(),
            server_payload: lang::Policy::default(),
            response: lang::Policy::default(),
        }
    }
}

pub struct RestPolicy {
    program: Arc<lang::Program>,
    proxy: Option<(Arc<Env>, u16)>,
    status: PolicyStatus,
}

impl Policy<actix_web::dev::Server> for RestPolicy {
    fn start(&mut self, env: Env, port: u16, addr: Addr<PolicyActor>) -> Option<Disconnector> {
        match super::http_proxy::start_proxy(addr, port) {
            Ok(server) => {
                self.proxy = Some((Arc::new(env), port));
                Some(Box::new(futures::lazy(move || {
                    info!("stopping HTTP server");
                    server.stop(true)
                })))
            }
            Err(err) => {
                warn!("failed to start REST proxy on port {}: {}", port, err);
                None
            }
        }
    }
    fn stop(&mut self) {
        self.proxy = None
    }
    fn set_debug(&mut self, b: bool) {
        self.status.debug = b
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.status.update_for_policy(&p);
        self.program = Arc::new(p);
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|p| p.1)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn env(&self) -> Arc<Env> {
        self.proxy.as_ref().map(|p| p.0.clone()).unwrap_or_default()
    }
    fn debug(&self) -> bool {
        self.status.debug
    }
    fn status(&self) -> Box<codec::Status> {
        Box::new(codec::Status {
            port: self.port(),
            debug: self.debug(),
            policy: (*self.policy()).clone(),
        })
    }
}

impl Default for RestPolicy {
    fn default() -> Self {
        RestPolicy {
            program: Arc::new(lang::Program::default()),
            proxy: None,
            status: PolicyStatus::default(),
        }
    }
}

impl RestPolicy {
    fn get(&self) -> PolicyStatus {
        self.status.clone()
    }
    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.status.timeout = timeout
    }
}

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct RestPolicyResponse {
    pub status: PolicyStatus,
    pub connection: literals::Connection,
}

/// Request REST policy information
pub struct GetRestPolicy(pub (ID, ID));

impl Message for GetRestPolicy {
    type Result = RestPolicyResponse;
}

// handle request to get current policy status information
impl Handler<GetRestPolicy> for PolicyActor {
    type Result = RestPolicyResponse;

    fn handle(&mut self, msg: GetRestPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let from_to = msg.0;
        RestPolicyResponse {
            status: self.http.get(),
            connection: self.connection(from_to.0, from_to.1),
        }
    }
}

/// REST policy functions
pub enum RestFn {
    Request,
    ClientPayload,
    ServerPayload,
    Response,
}

/// Request evaluation of a (REST) policy function
pub struct EvalRestFn(pub RestFn, pub Vec<expressions::Expr>);

impl Message for EvalRestFn {
    type Result = Result<bool, expressions::Error>;
}

// handle requests to evaluate the Armour policy
impl Handler<EvalRestFn> for PolicyActor {
    type Result = Box<dyn Future<Item = bool, Error = expressions::Error>>;

    fn handle(&mut self, msg: EvalRestFn, _ctx: &mut Context<Self>) -> Self::Result {
        let function = match msg.0 {
            RestFn::Request => policy::ALLOW_REST_REQUEST,
            RestFn::ClientPayload => policy::ALLOW_CLIENT_PAYLOAD,
            RestFn::ServerPayload => policy::ALLOW_SERVER_PAYLOAD,
            RestFn::Response => policy::ALLOW_REST_RESPONSE,
        };
        self.http.evaluate(function, msg.1)
    }
}
