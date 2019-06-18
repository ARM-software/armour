use actix_web::Error;
use arm_policy::{lang, literals};
use futures::{future, Future};
use std::sync::Arc;

pub trait HttpAccept {
    fn accept(&self, req: &actix_web::HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>>
    where
        Self: Sized;
}

#[derive(Clone)]
pub struct ArmourState {
    program: Arc<lang::Program>,
}

impl ArmourState {
    pub fn new() -> ArmourState {
        ArmourState {
            program: Arc::new(lang::Program::new()),
        }
    }
    // Load policy from a file
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
    // Convert Actix-Web HttpRequest into an equivalent Armour expression (HttpRequest literal)
    fn http_request(req: &actix_web::HttpRequest) -> lang::Expr {
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

pub enum Policy {
    Accept,
}

// Implement the "accept" method for Armour policies. Evaluates a "require function"
impl HttpAccept for ArmourState {
    fn accept(&self, req: &actix_web::HttpRequest) -> Box<dyn Future<Item = bool, Error = Error>> {
        let prog = self.program.clone();
        if prog.has_function("require") {
            Box::new(
                lang::Expr::call1("require", ArmourState::http_request(&req))
                    .evaluate(prog)
                    .and_then(|res| match res {
                        lang::Expr::LitExpr(literals::Literal::PolicyLiteral(result)) => {
                            info!("successfully evaluated policy");
                            future::ok(result == literals::Policy::Accept)
                        }
                        _ => unreachable!(),
                    })
                    .map_err(|e| {
                        warn!("got an error when evaluating Armour policy");
                        actix_web::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    }),
            )
        } else {
            // block if there is no "require" function
            Box::new(future::ok(false))
        }
    }
}
