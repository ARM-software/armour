//! actix-web support for Armour policies
use super::policy::{Policy, PolicyActor};
use actix::prelude::*;
use armour_data_interface as interface;
use armour_policy::lang;
use futures::Future;
use std::sync::Arc;

pub struct RestPolicy {
    program: Arc<lang::Program>,
    allow_all: bool,
    debug: bool,
    timeout: std::time::Duration,
    proxy: Option<(u16, actix_web::dev::Server)>,
}

impl Policy<actix_web::dev::Server> for RestPolicy {
    fn start(&mut self, port: u16, proxy: actix_web::dev::Server) {
        self.stop();
        self.proxy = Some((port, proxy))
    }
    fn port(&self) -> Option<u16> {
        self.proxy.as_ref().map(|(p, _)| *p)
    }
    fn stop(&mut self) -> bool {
        if let Some((_, ref server)) = self.proxy {
            server.stop(true); // graceful stop
            self.proxy = None;
            true
        } else {
            false
        }
    }
    fn set_policy(&mut self, p: lang::Program) {
        self.program = Arc::new(p);
        self.allow_all = false
    }
    fn policy(&self) -> Arc<lang::Program> {
        self.program.clone()
    }
    fn set_debug(&mut self, b: bool) {
        self.debug = b
    }
    fn debug(&self) -> bool {
        self.debug
    }
    fn deny_all(&mut self) {
        self.set_policy(lang::Program::default());
        self.allow_all = false
    }
    fn allow_all(&mut self) {
        self.set_policy(lang::Program::default());
        self.allow_all = true;
    }
    fn is_allow_all(&self) -> bool {
        self.allow_all
    }
    fn is_deny_all(&self) -> bool {
        !(self.allow_all
            || self.program.has_function(interface::ALLOW_REST_REQUEST)
            || self.program.has_function(interface::ALLOW_CLIENT_PAYLOAD)
            || self.program.has_function(interface::ALLOW_SERVER_PAYLOAD)
            || self.program.has_function(interface::ALLOW_REST_RESPONSE))
    }
    fn status(&self) -> Box<interface::Status> {
        let policy = if self.is_allow_all() {
            interface::Policy::AllowAll
        } else if self.is_deny_all() {
            interface::Policy::DenyAll
        } else {
            interface::Policy::Program((*self.policy()).clone())
        };
        Box::new(interface::Status {
            port: self.port(),
            debug: self.debug(),
            policy,
        })
    }
}

impl Default for RestPolicy {
    fn default() -> Self {
        RestPolicy {
            program: Arc::new(lang::Program::default()),
            allow_all: false,
            debug: false,
            timeout: std::time::Duration::from_secs(5),
            proxy: None,
        }
    }
}

impl RestPolicy {
    fn get(&self, connection_number: usize) -> RestPolicyStatus {
        if self.allow_all {
            RestPolicyStatus {
                allow_all: true,
                debug: self.debug,
                timeout: self.timeout,
                connection_number,
                request: None,
                client_payload: None,
                server_payload: None,
                response: None,
            }
        } else {
            let program = &self.program;
            RestPolicyStatus {
                allow_all: false,
                debug: self.debug,
                timeout: self.timeout,
                connection_number,
                request: program.arg_count(interface::ALLOW_REST_REQUEST),
                client_payload: program.arg_count(interface::ALLOW_CLIENT_PAYLOAD),
                server_payload: program.arg_count(interface::ALLOW_SERVER_PAYLOAD),
                response: program.arg_count(interface::ALLOW_REST_REQUEST),
            }
        }
    }
    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.timeout = timeout
    }
}

/// Request REST policy information
pub struct GetRestPolicy;

impl Message for GetRestPolicy {
    type Result = RestPolicyStatus;
}

/// Information about REST policies
#[derive(MessageResponse)]
pub struct RestPolicyStatus {
    pub allow_all: bool,
    pub debug: bool,
    pub timeout: std::time::Duration,
    pub connection_number: usize,
    pub request: Option<u8>,
    pub client_payload: Option<u8>,
    pub server_payload: Option<u8>,
    pub response: Option<u8>,
}

// handle request to get current policy status information
impl Handler<GetRestPolicy> for PolicyActor {
    type Result = RestPolicyStatus;

    fn handle(&mut self, _msg: GetRestPolicy, _ctx: &mut Context<Self>) -> Self::Result {
        let policy = self.http.get(self.connection_number);
        self.connection_number += 1;
        policy
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
pub struct EvalRestFn(pub RestFn, pub Vec<lang::Expr>);

impl Message for EvalRestFn {
    type Result = Result<bool, lang::Error>;
}

// handle requests to evaluate the Armour policy
impl Handler<EvalRestFn> for PolicyActor {
    type Result = Box<dyn Future<Item = bool, Error = lang::Error>>;

    fn handle(&mut self, msg: EvalRestFn, _ctx: &mut Context<Self>) -> Self::Result {
        let function = match msg.0 {
            RestFn::Request => interface::ALLOW_REST_REQUEST,
            RestFn::ClientPayload => interface::ALLOW_CLIENT_PAYLOAD,
            RestFn::ServerPayload => interface::ALLOW_SERVER_PAYLOAD,
            RestFn::Response => interface::ALLOW_REST_RESPONSE,
        };
        self.http.evaluate(function, msg.1)
    }
}
