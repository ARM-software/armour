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
    fn accept(&self, req: &HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        Self: Sized;
}

// Implement the "accept" method for Armour policies. Evaluates a "require function"
impl AcceptRequest for web::Data<ArmourPolicy> {
    fn accept(&self, req: &HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>> {
        if self.program.has_function("require") {
            Box::new(
                lang::Expr::call1("require", ArmourPolicy::http_request(&req))
                    .evaluate(self.program.clone())
                    .and_then(|res| match res {
                        lang::Expr::LitExpr(literals::Literal::PolicyLiteral(result)) => {
                            info!("successfully evaluated policy");
                            future::ok(result == literals::Policy::Accept)
                        }
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
pub trait AcceptResponse {
    fn accept(&self, req: &web::Bytes) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        Self: Sized;
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
