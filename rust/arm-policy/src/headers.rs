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

#[derive(Clone, Serialize, Deserialize)]
pub struct Headers(HashMap<String, Signature>);

impl Headers {
    pub fn new() -> Headers {
        Headers(HashMap::new())
    }
    pub fn add_function(&mut self, name: &str, args: Vec<Typ>, ret: &Typ) -> Result<(), Error> {
        if self
            .0
            .insert(name.to_string(), (args, ret.to_owned()))
            .is_some()
        {
            Err(Error::new(format!("duplicate function \"{}\"", name)))
        } else {
            Ok(())
        }
    }
    pub fn builtins(f: &str) -> Option<Signature> {
        match f {
            "i64::abs" => Some((vec![Typ::I64], Typ::I64)),
            "i64::to_str" => Some((vec![Typ::I64], Typ::Str)),
            "str::len" => Some((vec![Typ::Str], Typ::I64)),
            "str::to_lowercase" => Some((vec![Typ::Str], Typ::Str)),
            "str::to_uppercase" => Some((vec![Typ::Str], Typ::Str)),
            "str::trim_start" => Some((vec![Typ::Str], Typ::Str)),
            "str::trim_end" => Some((vec![Typ::Str], Typ::Str)),
            "str::as_bytes" => Some((vec![Typ::Str], Typ::Data)),
            "str::from_utf8" => Some((vec![Typ::Data], Typ::Str)),
            "str::to_base64" => Some((vec![Typ::Str], Typ::Str)),
            "data::to_base64" => Some((vec![Typ::Data], Typ::Str)),
            "data::len" => Some((vec![Typ::Data], Typ::I64)),
            "i64::pow" => Some((vec![Typ::I64, Typ::I64], Typ::I64)),
            "i64::min" => Some((vec![Typ::I64, Typ::I64], Typ::I64)),
            "i64::max" => Some((vec![Typ::I64, Typ::I64], Typ::I64)),
            "str::starts_with" => Some((vec![Typ::Str, Typ::Str], Typ::Bool)),
            "str::ends_with" => Some((vec![Typ::Str, Typ::Str], Typ::Bool)),
            "str::contains" => Some((vec![Typ::Str, Typ::Str], Typ::Bool)),
            "list::len" => Some((vec![Typ::List(Box::new(Typ::Return))], Typ::I64)),
            "HttpRequest::default" => Some((vec![], Typ::HttpRequest)),
            "HttpRequest::method" => Some((vec![Typ::HttpRequest], Typ::Str)),
            "HttpRequest::version" => Some((vec![Typ::HttpRequest], Typ::Str)),
            "HttpRequest::path" => Some((vec![Typ::HttpRequest], Typ::Str)),
            "HttpRequest::route" => Some((vec![Typ::HttpRequest], Typ::List(Box::new(Typ::Str)))),
            "HttpRequest::query" => Some((vec![Typ::HttpRequest], Typ::Str)),
            "HttpRequest::query_pairs" => Some((
                vec![Typ::HttpRequest],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Str]))),
            )),
            "HttpRequest::header" => Some((
                vec![Typ::HttpRequest, Typ::Str],
                Typ::List(Box::new(Typ::Data)),
            )),
            "HttpRequest::headers" => Some((vec![Typ::HttpRequest], Typ::List(Box::new(Typ::Str)))),
            "HttpRequest::header_pairs" => Some((
                vec![Typ::HttpRequest],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Data]))),
            )),
            "HttpRequest::set_path" => Some((vec![Typ::HttpRequest, Typ::Str], Typ::HttpRequest)),
            "HttpRequest::set_query" => Some((vec![Typ::HttpRequest, Typ::Str], Typ::HttpRequest)),
            "HttpRequest::set_header" => {
                Some((vec![Typ::HttpRequest, Typ::Str, Typ::Data], Typ::HttpRequest))
            }
            _ => None,
        }
    }
    pub fn is_builtin(name: &str) -> bool {
        Headers::builtins(name).is_some() || name.parse::<usize>().is_ok()
    }
    pub fn typ(&self, name: &str) -> Option<Signature> {
        (Headers::builtins(name).or(self.0.get(name).cloned()))
    }
    pub fn return_typ(&self, name: &str) -> Result<Typ, Error> {
        Ok(self.typ(name).ok_or(Error::new("no current function"))?.1)
    }
    pub fn resolve(name: &str, typs: &Vec<Typ>) -> String {
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
