use actix::prelude::*;
use actix_web::web;
use armour_lang::{expressions, literals};

#[derive(Message)]
#[rtype("()")]
pub struct Stop;

pub mod http_policy;
pub mod http_proxy;
pub mod policy;
pub mod tcp_codec;
pub mod tcp_policy;
pub mod tcp_proxy;

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_expression(&self) -> expressions::DPExpr;
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpRequest, &literals::DPConnection) {
    fn to_expression(&self) -> expressions::DPExpr {
        let (req, connection) = *self;
        literals::HttpRequest::new(
            req.method().as_str(),
            format!("{:?}", req.version()).as_str(),
            req.path(),
            req.query_string(),
            req.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            connection.clone(),
        )
        .into()
    }
}

/// Convert an actix-web HttpResponse into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpResponse, &literals::DPConnection) {
    fn to_expression(&self) -> expressions::DPExpr {
        let res = self.0;
        let head = res.head();
        literals::HttpResponse::new(
            format!("{:?}", head.version).as_str(),
            res.status().as_u16(),
            head.reason,
            res.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            self.1.clone(),
        )
        .into()
    }
}
