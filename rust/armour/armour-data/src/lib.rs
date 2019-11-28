#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;
use actix_web::web;
use armour_policy::{expressions, literals};

#[derive(Message)]
pub struct Stop;

pub mod http_policy;
pub mod http_proxy;
pub mod policy;
pub mod tcp_codec;
pub mod tcp_policy;
pub mod tcp_proxy;

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_expression(&self) -> expressions::Expr;
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpRequest, &literals::Connection) {
    fn to_expression(&self) -> expressions::Expr {
        let (req, connection) = *self;
        expressions::Expr::from(literals::HttpRequest::from((
            req.method().as_str(),
            format!("{:?}", req.version()).as_str(),
            req.path(),
            req.query_string(),
            req.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            connection.clone(),
        )))
    }
}

/// Convert an actix-web HttpResponse into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpResponse, &literals::Connection) {
    fn to_expression(&self) -> expressions::Expr {
        let (req, connection) = *self;
        let head = req.head();
        expressions::Expr::from(literals::HttpResponse::from((
            format!("{:?}", head.version).as_str(),
            req.status().as_u16(),
            head.reason,
            req.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            connection.clone(),
        )))
    }
}
