//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor};
use actix::prelude::*;
use armour_data_interface::{codec, policy};
use armour_policy::{expressions, lang};
use futures::Future;
use std::sync::Arc;

/// Information about REST policies
#[derive(Clone, MessageResponse)]
pub struct RestPolicyStatus {
    pub debug: bool,
    pub timeout: std::time::Duration,
    pub request: lang::Policy,
    pub client_payload: lang::Policy,
    pub server_payload: lang::Policy,
    pub response: lang::Policy,
    pub connection_number: usize,
}

impl RestPolicyStatus {
    fn update_for_policy(&mut self, prog: &lang::Program) {
        self.request = prog.policy(policy::ALLOW_REST_REQUEST);
        self.client_payload = prog.policy(policy::ALLOW_CLIENT_PAYLOAD);
        self.server_payload = prog.policy(policy::ALLOW_SERVER_PAYLOAD);
        self.response = prog.policy(policy::ALLOW_REST_RESPONSE)
    }
    pub fn has_ids(&self) -> bool {
        match self {
            // the policy inpterpreter will be needing the endpoint IDs
            RestPolicyStatus {
                request: lang::Policy::Args(n),
                ..
            }
            | RestPolicyStatus {
                client_payload: lang::Policy::Args(n),
                ..
            }
            | RestPolicyStatus {
                server_payload: lang::Policy::Args(n),
                ..
            } => 3 <= *n,
            _ => false,
        }
    }
}

impl Default for RestPolicyStatus {
    fn default() -> Self {
        RestPolicyStatus {
            debug: false,
            timeout: std::time::Duration::from_secs(5),
            request: lang::Policy::default(),
            client_payload: lang::Policy::default(),
            server_payload: lang::Policy::default(),
            response: lang::Policy::default(),
            connection_number: 0,
        }
    }
}

pub struct RestPolicy {
    program: Arc<lang::Program>,
    proxy: Option<(u16, actix_web::dev::Server)>,
    status: RestPolicyStatus,
}

impl Policy<actix_web::dev::Server> for RestPolicy {
    fn start(&mut self, port: u16, proxy: actix_web::dev::Server) {
        self.stop();
        self.proxy = Some((port, proxy))
    }
    fn stop(&mut self) -> bool {
        if let Some((_, ref server)) = self.proxy {
            actix::spawn(server.stop(true)); // graceful stop
            self.proxy = None;
            true
        } else {
            false
        }
    }
    fn set_debug(&mut self, b: bool) {
        self.status.debug = b
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.status.update_for_policy(&p);
        self.program = Arc::new(p)
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|(p, _)| *p)
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
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
            status: RestPolicyStatus::default(),
        }
    }
}

impl RestPolicy {
    fn get(&self) -> RestPolicyStatus {
        self.status.clone()
    }
    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.status.timeout = timeout
    }
}

/// Request REST policy information
pub struct GetRestPolicy;

impl Message for GetRestPolicy {
    type Result = RestPolicyStatus;
}

// handle request to get current policy status information
impl Handler<GetRestPolicy> for PolicyActor {
    type Result = RestPolicyStatus;

    fn handle(&mut self, _msg: GetRestPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let mut status = self.http.get();
        status.connection_number = self.connection_number;
        self.connection_number += 1;
        status
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
