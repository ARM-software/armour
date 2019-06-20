//! actix-web support for Armour policies
use actix_web::{web, Error};
use arm_policy::{lang, literals};
use futures::{future, Future};
use std::sync::Arc;

/// Armour policies, currently just Armour programs
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
    pub fn from_file<P: AsRef<std::path::Path> + std::fmt::Display>(
        &mut self,
        p: P,
    ) -> std::io::Result<()> {
        match lang::Program::from_file(p.as_ref()) {
            Ok(prog) => {
                info!("installed policy: \"{}\"", p);
                self.program = Arc::new(prog);
                Ok(())
            }
            Err(e) => {
                warn!(r#""{}": {}"#, p, e);
                Err(std::io::Error::from(std::io::ErrorKind::Other))
            }
        }
    }
}

/// Trait for evaluating Armour policies
pub trait EvaluatePolicy {
    fn evaluate_policy<A>(
        &self,
        function: &str,
        args: Vec<&A>,
    ) -> Box<dyn Future<Item = Option<bool>, Error = Error>>
    where
        A: ToArmourExpression;
}

/// Implement EvaluatePolicy trait using Armour policy
impl EvaluatePolicy for web::Data<ArmourPolicy> {
    fn evaluate_policy<A>(
        &self,
        function: &str,
        args: Vec<&A>,
    ) -> Box<dyn Future<Item = Option<bool>, Error = Error>>
    where
        A: ToArmourExpression,
    {
        if self.program.has_function(function) {
            Box::new(
                lang::Expr::call(
                    function,
                    args.into_iter().map(|a| a.to_armour_expression()).collect(),
                )
                .evaluate(self.program.clone())
                .and_then(|result| match result {
                    lang::Expr::LitExpr(literals::Literal::PolicyLiteral(policy)) => {
                        future::ok(Some(policy == literals::Policy::Accept))
                    }
                    lang::Expr::LitExpr(literals::Literal::BoolLiteral(accept)) => {
                        future::ok(Some(accept))
                    }
                    _ => future::err(arm_policy::lang::Error::new(
                        "did not evaluate to a bool or policy literal",
                    )),
                })
                .map_err(|e| {
                    warn!("{}", e);
                    e.to_actix()
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

/// Trait for converting errors into actix-web errors
pub trait ToActixError {
    fn to_actix(self) -> Error
    where
        Self: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        std::io::Error::new(std::io::ErrorKind::Other, self).into()
    }
}

impl ToActixError for arm_policy::lang::Error {}
impl ToActixError for url::ParseError {}
impl ToActixError for http::header::ToStrError {}
impl ToActixError for &str {}
