#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;
use actix_web::{http::uri, web};
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

// convert payloads into Armour-language expressions
impl ToArmourExpression for usize {
    fn to_expression(&self) -> expressions::Expr {
        expressions::Expr::i64(*self as i64)
    }
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for web::HttpRequest {
    fn to_expression(&self) -> expressions::Expr {
        expressions::Expr::http_request(literals::HttpRequest::from((
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

/// Convert an actix-web HttpResponse into an equivalent Armour language literal
impl ToArmourExpression for web::HttpResponse {
    fn to_expression(&self) -> expressions::Expr {
        let head = self.head();
        expressions::Expr::http_response(literals::HttpResponse::from((
            format!("{:?}", head.version).as_str(),
            self.status().as_u16(),
            head.reason,
            self.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
        )))
    }
}

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::Bytes {
    fn to_expression(&self) -> expressions::Expr {
        expressions::Expr::data(self)
    }
}

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::BytesMut {
    fn to_expression(&self) -> expressions::Expr {
        expressions::Expr::data(self)
    }
}

// convert socket addresses into Armour-language expressions (of type ID)
impl ToArmourExpression for std::net::SocketAddr {
    fn to_expression(&self) -> expressions::Expr {
        let mut id = literals::ID::default();
        let ip = self.ip();
        if ip.is_ipv4() {
            id = id.add_ip(ip)
        };
        id = id.set_port(self.port());
        if let Ok(host) = dns_lookup::lookup_addr(&ip) {
            id = id.add_host(&host)
        }
        expressions::Expr::id(id)
    }
}

impl ToArmourExpression for Option<std::net::SocketAddr> {
    fn to_expression(&self) -> expressions::Expr {
        if let Some(addr) = self {
            addr.to_expression()
        } else {
            expressions::Expr::id(literals::ID::default())
        }
    }
}

// convert URLs into Armour-language expressions (of type ID)
impl ToArmourExpression for uri::Uri {
    fn to_expression(&self) -> expressions::Expr {
        // new default ID
        let mut id = literals::ID::default();
        // try to set the host and add IP addresses
        if let Some(host) = self.host() {
            id = id.add_host(host);
            if let Ok(ips) = dns_lookup::lookup_host(host) {
                for ip in ips.iter().filter(|ip| ip.is_ipv4()) {
                    id = id.add_ip(*ip)
                }
            }
        }
        // try to set the port
        if let Some(port) = self.port_u16() {
            id = id.set_port(port)
        } else if let Some(scheme) = self.scheme_part() {
            if *scheme == uri::Scheme::HTTPS {
                id = id.set_port(443)
            } else if *scheme == uri::Scheme::HTTP {
                id = id.set_port(80)
            }
        }
        expressions::Expr::id(id)
    }
}
