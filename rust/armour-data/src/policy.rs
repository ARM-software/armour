//! actix-web support for Armour policies
use actix_web::{web, Error, HttpRequest};
use arm_policy::{lang, literals};
use futures::{future, Future};
use std::sync::Arc;

/// Armour policies, based on evaluating Armour programs
#[derive(Clone)]
pub struct ArmourPolicy {
    program: Arc<lang::Program>,
}

impl ArmourPolicy {
    pub fn new() -> ArmourPolicy {
        ArmourPolicy {
            program: Arc::new(lang::Program::new()),
        }
    }
    /// Attempt to load a new policy from a file
    pub fn from_file<P: AsRef<std::path::Path> + std::fmt::Display>(&mut self, p: P) -> bool {
        match lang::Program::from_file(p.as_ref()) {
            Ok(prog) => {
                info!("installed policy: \"{}\"", p);
                self.program = Arc::new(prog);
                true
            }
            Err(e) => {
                warn!("path \"{}\": {}", p, e);
                false
            }
        }
    }
    /// Convert an actix-web HttpRequest into an equivalent Armour language literal
    fn http_request(req: &HttpRequest) -> lang::Expr {
        let headers: Vec<(&str, &[u8])> = req
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_bytes()))
            .collect();
        lang::Expr::http_request(literals::HttpRequest::from((
            req.method().as_str(),
            format!("{:?}", req.version()).as_str(),
            req.path(),
            req.query_string(),
            headers,
        )))
    }
}

/// Trait for accepting, or rejecting, HTTP requests
pub trait AcceptRequest {
    fn accept_request(&self, req: &HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        Self: Sized;
}

// Implement the "accept" method for Armour policies. Evaluates a "require function"
impl AcceptRequest for web::Data<ArmourPolicy> {
    fn accept_request(&self, req: &HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>> {
        const REQUIRE: &str = "require";
        if self.program.has_function(REQUIRE) {
            Box::new(
                lang::Expr::call1(REQUIRE, ArmourPolicy::http_request(&req))
                    .evaluate(self.program.clone())
                    .and_then(|result| match result {
                        lang::Expr::LitExpr(literals::Literal::PolicyLiteral(policy)) => {
                            info!("successfully evaluated policy");
                            future::ok(policy == literals::Policy::Accept)
                        }
                        // TODO: handle dynamic type errors
                        _ => unreachable!(),
                    })
                    .map_err(|e| {
                        warn!("got an error when evaluating Armour policy");
                        e.to_actix()
                    }),
            )
        } else {
            // block if there is no "require" function
            Box::new(future::ok(false))
        }
    }
}

/// Trait for accepting, or rejecting, HTTP responses
pub trait AcceptPayload {
    fn accept_payload<B>(
        &self,
        checker: &str,
        payload: &B,
    ) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        Self: Sized,
        B: AsRef<[u8]>;
}

impl AcceptPayload for web::Data<ArmourPolicy> {
    fn accept_payload<B>(
        &self,
        checker: &str,
        payload: &B,
    ) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        B: AsRef<[u8]>,
    {
        if self.program.has_function(checker) {
            Box::new(
                lang::Expr::call1(checker, lang::Expr::data(payload.as_ref()))
                    .evaluate(self.program.clone())
                    .and_then(|result| match result {
                        lang::Expr::LitExpr(literals::Literal::PolicyLiteral(policy)) => {
                            future::ok(policy == literals::Policy::Accept)
                        }
                        // TODO: handle dynamic type errors
                        _ => unreachable!(),
                    })
                    .map_err(|e| {
                        warn!("got an error when evaluating Armour policy");
                        e.to_actix()
                    }),
            )
        } else {
            // block if there is no "ensure" function
            Box::new(future::ok(true))
        }
    }
}

/// Trait for lifting errors into actix-web errors
pub trait ToActixError {
    fn to_actix(self) -> Error
    where
        Self: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Error::from(std::io::Error::new(std::io::ErrorKind::Other, self))
    }
}

impl ToActixError for arm_policy::lang::Error {}
impl ToActixError for url::ParseError {}
impl ToActixError for http::header::ToStrError {}
impl ToActixError for &str {}
