#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;
use actix_web::web;
use armour_policy::{lang, literals};

#[derive(Message)]
pub struct Stop;

pub mod http_policy;
pub mod http_proxy;
pub mod policy;
pub mod tcp_policy;
pub mod tcp_proxy;

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_expression(&self) -> lang::Expr;
}

// convert payloads into Armour-language expressions
impl ToArmourExpression for usize {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::i64(*self as i64)
    }
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

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::Bytes {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

// convert payloads into Armour-language expressions
impl ToArmourExpression for web::BytesMut {
    fn to_expression(&self) -> lang::Expr {
        lang::Expr::data(self)
    }
}

// convert socket addresses into Armour-language expressions (of type ID)
impl ToArmourExpression for std::net::SocketAddr {
    fn to_expression(&self) -> lang::Expr {
        let mut id = literals::ID::default();
        let ip = self.ip();
        if ip.is_ipv4() {
            id = id.add_ip(ip)
        };
        id = id.set_port(self.port());
        if let Ok(host) = dns_lookup::lookup_addr(&ip) {
            id = id.add_host(&host)
        }
        lang::Expr::id(id)
    }
}

impl ToArmourExpression for Option<std::net::SocketAddr> {
    fn to_expression(&self) -> lang::Expr {
        if let Some(addr) = self {
            addr.to_expression()
        } else {
            lang::Expr::id(literals::ID::default())
        }
    }
}

// convert URLs into Armour-language expressions (of type ID)
impl ToArmourExpression for url::Url {
    fn to_expression(&self) -> lang::Expr {
        let mut id = literals::ID::default();
        if let Some(host) = self.host_str() {
            id = id.add_host(host);
            if let Ok(ips) = dns_lookup::lookup_host(host) {
                for ip in ips.iter().filter(|ip| ip.is_ipv4()) {
                    id = id.add_ip(*ip)
                }
            }
        }
        if let Some(port) = self.port() {
            id = id.set_port(port)
        } else {
            match self.scheme() {
                "https" => id = id.set_port(443),
                "http" => id = id.set_port(80),
                s => log::debug!("scheme is: {}", s),
            }
        }
        lang::Expr::id(id)
    }
}
