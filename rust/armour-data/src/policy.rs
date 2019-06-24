//! actix-web support for Armour policies
use actix::prelude::*;
use actix_web::web;
use arm_policy::{lang, literals};
use armour_data_interface::ArmourPolicyRequest;
use futures::{future, Future};
use std::sync::Arc;
// use tokio_codec::FramedRead;
// use tokio_io::{io::WriteHalf, AsyncRead};

/// Armour policies, currently just Armour programs
#[allow(dead_code)]
pub struct ArmourPolicy {
    program: Arc<lang::Program>,
    // uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, MasterArmourDataCodec>,
}

impl ArmourPolicy {
    pub fn create_policy<P: AsRef<std::path::Path>>(_p: P) -> Addr<ArmourPolicy> {
        (ArmourPolicy {
            program: Arc::new(lang::Program::new()),
        })
        .start()
    }
}

impl actix::io::WriteHandler<std::io::Error> for ArmourPolicy {}

impl StreamHandler<ArmourPolicyRequest, std::io::Error> for ArmourPolicy {
    fn handle(&mut self, msg: ArmourPolicyRequest, ctx: &mut Context<Self>) {
        // need to report back using uds_framed
        ctx.address()
            .send(msg)
            .then(|_| future::ok::<(), ()>(()))
            .wait()
            .unwrap_or(())
    }
}

pub enum ArmourEvaluateMessage {
    Require(lang::Expr),
    ClientPayload(lang::Expr),
    ServerPayload(lang::Expr),
}

impl Message for ArmourEvaluateMessage {
    type Result = Result<Option<bool>, lang::Error>;
}

impl Handler<ArmourEvaluateMessage> for ArmourPolicy {
    type Result = Box<dyn Future<Item = Option<bool>, Error = lang::Error>>;

    fn handle(&mut self, msg: ArmourEvaluateMessage, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            ArmourEvaluateMessage::Require(arg) => self.evaluate_policy("require", vec![arg]),
            ArmourEvaluateMessage::ClientPayload(arg) => {
                self.evaluate_policy("client_payload", vec![arg])
            }
            ArmourEvaluateMessage::ServerPayload(arg) => {
                self.evaluate_policy("server_payload", vec![arg])
            }
        }
    }
}

impl Handler<ArmourPolicyRequest> for ArmourPolicy {
    type Result = std::io::Result<()>;

    fn handle(&mut self, msg: ArmourPolicyRequest, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            // Attempt to load a new policy from a file
            ArmourPolicyRequest::UpdateFromFile(p) => match lang::Program::from_file(p.as_path()) {
                Ok(prog) => {
                    info!(
                        "installed policy: \"{}\"",
                        p.to_str().unwrap_or("<unknown>")
                    );
                    self.program = Arc::new(prog);
                    Ok(())
                }
                Err(e) => {
                    warn!(r#"{:?}: {}"#, p, e);
                    Err(std::io::Error::from(std::io::ErrorKind::Other))
                }
            },
        }
    }
}

impl Actor for ArmourPolicy {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("started Armour Policy")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("stopped Armour Policy")
    }
}

/// Trait for evaluating policies
pub trait EvaluatePolicy {
    fn evaluate_policy(
        &self,
        function: &str,
        args: Vec<lang::Expr>,
    ) -> Box<dyn Future<Item = Option<bool>, Error = lang::Error>>;
}

/// Implement EvaluatePolicy trait using Armour policy
impl EvaluatePolicy for ArmourPolicy {
    fn evaluate_policy(
        &self,
        function: &str,
        args: Vec<lang::Expr>,
    ) -> Box<dyn Future<Item = Option<bool>, Error = lang::Error>> {
        if self.program.has_function(function) {
            info!(r#"evaluting "{}"""#, function);
            Box::new(
                lang::Expr::call(function, args)
                    .evaluate(self.program.clone())
                    .and_then(|result| match result {
                        lang::Expr::LitExpr(literals::Literal::PolicyLiteral(policy)) => {
                            info!("result is: {:?}", policy);
                            future::ok(Some(policy == literals::Policy::Accept))
                        }
                        lang::Expr::LitExpr(literals::Literal::BoolLiteral(accept)) => {
                            info!("result is: {}", accept);
                            future::ok(Some(accept))
                        }
                        _ => future::err(lang::Error::new(
                            "did not evaluate to a bool or policy literal",
                        )),
                    }),
            )
        } else {
            // there is no "function"
            Box::new(future::ok(None))
        }
    }
}

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_armour_expression(&self) -> lang::Expr;
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for web::HttpRequest {
    fn to_armour_expression(&self) -> lang::Expr {
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
    fn to_armour_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

impl ToArmourExpression for web::BytesMut {
    fn to_armour_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}
