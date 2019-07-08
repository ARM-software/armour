//! actix-web support for Armour policies
use actix::prelude::*;
use actix_web::web;
use arm_policy::{lang, literals};
use armour_data_interface::{PolicyCodec, PolicyRequest, PolicyResponse};
use futures::{future, Future};
use literals::ToLiteral;
use std::str::FromStr;
use std::sync::Arc;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

/// Armour policy actor
///
/// Currently, a "policy" is just an Armour program with "require", "client_payload" and "server_payload" functions.
pub struct DataPolicy {
    /// policy program
    program: Arc<lang::Program>,
    // connection to master
    uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, PolicyCodec>,
}

impl DataPolicy {
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
                        program: Arc::new(lang::Program::new()),
                        uds_framed: actix::io::FramedWrite::new(w, PolicyCodec, ctx),
                    }
                });
                future::ok(addr)
            })
            .wait()
    }
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
                        lang::Expr::LitExpr(literals::Literal::Policy(policy)) => {
                            info!("result is: {:?}", policy);
                            future::ok(Some(policy == literals::Policy::Accept))
                        }
                        lang::Expr::LitExpr(literals::Literal::Bool(accept)) => {
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

/// Internal proxy message for requesting function evaluation over the policy
pub enum Evaluate {
    Require(lang::Expr, lang::Expr),
    ClientPayload(lang::Expr),
    ServerPayload(lang::Expr),
}

impl Message for Evaluate {
    type Result = Result<Option<bool>, lang::Error>;
}

impl Handler<Evaluate> for DataPolicy {
    type Result = Box<dyn Future<Item = Option<bool>, Error = lang::Error>>;

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

    fn handle(&mut self, msg: PolicyRequest, _ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            // Attempt to load a new policy from a file
            PolicyRequest::UpdateFromFile(p) => match lang::Program::check_from_file(
                p.as_path(),
                &*armour_data_interface::POLICY_SIG,
            ) {
                Ok(prog) => {
                    self.program = Arc::new(prog);
                    info!(
                        "installed policy: \"{}\"",
                        p.to_str().unwrap_or("<unknown>")
                    );
                    self.uds_framed.write(PolicyResponse::UpdatedPolicy)
                }
                Err(e) => {
                    warn!(r#"{:?}: {}"#, p, e);
                    self.uds_framed.write(PolicyResponse::RequestFailed)
                }
            },
            PolicyRequest::UpdateFromData(prog) => {
                self.program = Arc::new(prog);
                info!("installed policy from data");
                self.uds_framed.write(PolicyResponse::UpdatedPolicy)
            }
            PolicyRequest::AllowAll => {
                self.program = Arc::new(ALLOW_ALL.clone());
                info!("switched to allow all policy");
                self.uds_framed.write(PolicyResponse::UpdatedPolicy)
            }
            PolicyRequest::DenyAll => {
                self.program = Arc::new(lang::Program::new());
                info!("switched to deny all policy");
                self.uds_framed.write(PolicyResponse::UpdatedPolicy)
            }
            PolicyRequest::Shutdown => System::current().stop(),
        }
    }
}

lazy_static! {
    /// Static "allow all" policy
    pub static ref ALLOW_ALL: lang::Program =
        lang::Program::from_str("fn require(r:HttpRequest) -> bool {true}").unwrap();
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
            Some(std::net::SocketAddr::V4(addr)) => {
                lang::Expr::some(literals::Literal::Tuple(vec![
                    addr.ip().to_literal(),
                    literals::Literal::Int(addr.port() as i64),
                ]))
            }
            _ => lang::Expr::none(),
        }
    }
}
