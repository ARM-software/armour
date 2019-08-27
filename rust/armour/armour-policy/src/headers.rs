use super::types::{Signature, Typ};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Error(pub String);

impl Error {
    pub fn new<D: std::fmt::Display>(e: D) -> Error {
        Error(e.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Headers(HashMap<String, Signature>);

impl Headers {
    pub fn add_function(&mut self, name: &str, sig: Signature) -> Result<(), Error> {
        if self.0.insert(name.to_string(), sig).is_some() {
            Err(Error::new(format!("duplicate function \"{}\"", name)))
        } else {
            Ok(())
        }
    }
    pub fn builtins(f: &str) -> Option<Signature> {
        let sig = |args, ty| Some(Signature::new(args, ty));
        match f {
            "option::Some" => sig(vec![Typ::Return], Typ::Return),
            "option::is_none" => sig(vec![Typ::any_option()], Typ::Bool),
            "option::is_some" => sig(vec![Typ::any_option()], Typ::Bool),
            "i64::abs" => sig(vec![Typ::I64], Typ::I64),
            "i64::to_str" => sig(vec![Typ::I64], Typ::Str),
            "str::len" => sig(vec![Typ::Str], Typ::I64),
            "str::to_lowercase" => sig(vec![Typ::Str], Typ::Str),
            "str::to_uppercase" => sig(vec![Typ::Str], Typ::Str),
            "str::trim_start" => sig(vec![Typ::Str], Typ::Str),
            "str::trim_end" => sig(vec![Typ::Str], Typ::Str),
            "str::as_bytes" => sig(vec![Typ::Str], Typ::Data),
            "str::from_utf8" => sig(vec![Typ::Data], Typ::Str),
            "str::to_base64" => sig(vec![Typ::Str], Typ::Str),
            "data::to_base64" => sig(vec![Typ::Data], Typ::Str),
            "data::len" => sig(vec![Typ::Data], Typ::I64),
            "i64::pow" => sig(vec![Typ::I64, Typ::I64], Typ::I64),
            "i64::min" => sig(vec![Typ::I64, Typ::I64], Typ::I64),
            "i64::max" => sig(vec![Typ::I64, Typ::I64], Typ::I64),
            "str::starts_with" => sig(vec![Typ::Str, Typ::Str], Typ::Bool),
            "str::ends_with" => sig(vec![Typ::Str, Typ::Str], Typ::Bool),
            "str::contains" => sig(vec![Typ::Str, Typ::Str], Typ::Bool),
            "list::len" => sig(vec![Typ::List(Box::new(Typ::Return))], Typ::I64),
            "list::reduce" => sig(vec![Typ::List(Box::new(Typ::Return))], Typ::Return.option()),
            "HttpRequest::GET" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::POST" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::PUT" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::DELETE" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::HEAD" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::OPTIONS" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::CONNECT" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::PATCH" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::TRACE" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::method" => sig(vec![Typ::HttpRequest], Typ::Str),
            "HttpRequest::version" => sig(vec![Typ::HttpRequest], Typ::Str),
            "HttpRequest::path" => sig(vec![Typ::HttpRequest], Typ::Str),
            "HttpRequest::route" => sig(vec![Typ::HttpRequest], Typ::List(Box::new(Typ::Str))),
            "HttpRequest::query" => sig(vec![Typ::HttpRequest], Typ::Str),
            "HttpRequest::query_pairs" => sig(
                vec![Typ::HttpRequest],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Str]))),
            ),
            "HttpRequest::header" => sig(
                vec![Typ::HttpRequest, Typ::Str],
                Typ::List(Box::new(Typ::Data)).option(),
            ),
            "HttpRequest::unique_header" => {
                sig(vec![Typ::HttpRequest, Typ::Str], Typ::Data.option())
            }
            "HttpRequest::headers" => sig(vec![Typ::HttpRequest], Typ::List(Box::new(Typ::Str))),
            "HttpRequest::header_pairs" => sig(
                vec![Typ::HttpRequest],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Data]))),
            ),
            "HttpRequest::set_path" => sig(vec![Typ::HttpRequest, Typ::Str], Typ::HttpRequest),
            "HttpRequest::set_query" => sig(vec![Typ::HttpRequest, Typ::Str], Typ::HttpRequest),
            "HttpRequest::set_header" => sig(
                vec![Typ::HttpRequest, Typ::Str, Typ::Data],
                Typ::HttpRequest,
            ),
            "IpAddr::lookup" => sig(
                vec![Typ::Str],
                Typ::Tuple(vec![Typ::List(Box::new(Typ::IpAddr))]),
            ),
            "IpAddr::reverse_lookup" => sig(
                vec![Typ::IpAddr],
                Typ::Tuple(vec![Typ::List(Box::new(Typ::Str))]),
            ),
            "IpAddr::localhost" => sig(vec![], Typ::IpAddr),
            "IpAddr::from" => sig(vec![Typ::I64, Typ::I64, Typ::I64, Typ::I64], Typ::IpAddr),
            "IpAddr::octets" => sig(
                vec![Typ::IpAddr],
                Typ::Tuple(vec![Typ::I64, Typ::I64, Typ::I64, Typ::I64]),
            ),
            "ID::default" => sig(vec![], Typ::ID),
            "ID::hosts" => sig(vec![Typ::ID], Typ::List(Box::new(Typ::Str))),
            "ID::ips" => sig(vec![Typ::ID], Typ::List(Box::new(Typ::IpAddr))),
            "ID::port" => sig(vec![Typ::ID], Typ::I64.option()),
            "ID::add_host" => sig(vec![Typ::ID, Typ::Str], Typ::ID),
            "ID::add_ip" => sig(vec![Typ::ID, Typ::IpAddr], Typ::ID),
            "ID::set_port" => sig(vec![Typ::ID, Typ::I64], Typ::ID),
            _ => None,
        }
    }
    pub fn is_builtin(name: &str) -> bool {
        Headers::builtins(name).is_some() || name.parse::<usize>().is_ok()
    }
    pub fn typ(&self, name: &str) -> Option<Signature> {
        (Headers::builtins(name).or_else(|| self.0.get(name).cloned()))
    }
    pub fn return_typ(&self, name: &str) -> Result<Typ, Error> {
        Ok(self
            .typ(name)
            .ok_or_else(|| Error::new("no current function"))?
            .typ())
    }
    pub fn resolve(name: &str, typs: &[Typ]) -> String {
        if name.starts_with(".::") {
            let rest = name.trim_start_matches(".::");
            let ty = typs
                .get(0)
                .expect("dot methods should have at least one argument");
            if let Some(intrinsic) = Typ::intrinsic(ty) {
                let s = format!("{}::{}", intrinsic, rest);
                if Headers::is_builtin(&s) {
                    s
                } else {
                    rest.to_string()
                }
            } else {
                rest.to_string()
            }
        } else {
            name.to_string()
        }
    }
}
