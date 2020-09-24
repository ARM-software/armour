use super::types::{Signature, Typ, TTyp};
use super::types_cp::{CPTyp};
use super::parser;
use super::parser::TPTyp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::marker::PhantomData;

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

//TBuiltin is a work around since specialization is unsable 
pub trait TBuiltin<PTyp: TPTyp> : TTyp<PTyp>{
    fn builtins(f: &str) -> Option<Signature<PTyp, Self>> {None}
    fn internal_service(f: &str) -> Option<Signature<PTyp, Self>> {None}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Headers<PTyp: TPTyp, Typ:TTyp<PTyp> + TBuiltin<PTyp> >(pub BTreeMap<String, Signature<PTyp, Typ>>, pub PhantomData<PTyp>);

impl<PTyp: TPTyp,  Typ:TTyp<PTyp>+TBuiltin<PTyp>> Default for Headers<PTyp, Typ> {
    fn default() -> Self {
        Headers(BTreeMap::new(), PhantomData)
    }
}

pub trait THeaders<PTyp: TPTyp, Typ:TTyp<PTyp>+TBuiltin<PTyp>> {
    fn insert(&mut self, key: String, value: Signature<PTyp, Typ>) -> Option<Signature<PTyp, Typ>>;
    fn remove(&mut self, key: &String) -> Option<Signature<PTyp, Typ>>; 
    fn get(&self, s:&str) -> Option<&Signature<PTyp, Typ>>;

    fn add_function(&mut self, name: &str, sig: Signature<PTyp, Typ>) -> Result<(), Error> {
        if self.insert(name.to_string(), sig).is_some() {
            Err(Error::new(format!("duplicate function \"{}\"", name)))
        } else {
            Ok(())
        }
    }
    fn cut(&mut self, set: &[String]) {
        for s in set.iter() {
            self.remove(s);
        }
    }
    fn typ(&self, name: &str) -> Option<Signature<PTyp, Typ>> {
        Typ::builtins(name)
            .or_else(|| Typ::internal_service(name).or_else(|| self.get(name).cloned()))
    }

    fn return_typ(&self, name: &str) -> Result<Typ, Error> {
        Ok(self
            .typ(name)
            .ok_or_else(|| Error::new("no current function"))?
            .typ())
    }

    fn is_builtin(name: &str) -> bool {
        Typ::builtins(name).is_some() || name.parse::<usize>().is_ok()
    }
    fn is_internal(name: &str) -> bool {
        Typ::internal_service(name).is_some() || Self::is_builtin(name)
    }
    fn split(name: &str) -> Option<(&str, &str)> {
        if let [module, method] = name.split("::").collect::<Vec<&str>>().as_slice() {
            Some((module, method))
        } else {
            None
        }
    }
    fn method(name: &str) -> Option<&str> {
        if let Some(Some(args)) = Typ::builtins(name).map(|ty| ty.args()) {
            if let Some(ty) = args.iter().next() {
                if let Some((module, method)) = Self::split(name) {
                    if module == ty.to_string().as_str() {
                        Some(method)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
    fn resolve(name: &str, typs: &[Typ]) -> String {
        if name.starts_with(".::") {
            let rest = name.trim_start_matches(".::");
            let ty = typs
                .get(0)
                .expect("dot methods should have at least one argument");
            if let Some(intrinsic) = Typ::intrinsic(ty) {
                let s = format!("{}::{}", intrinsic, rest);
                if Self::is_builtin(&s) {
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
impl<PTyp: TPTyp, Typ:TTyp<PTyp>+TBuiltin<PTyp>> THeaders<PTyp, Typ> for Headers<PTyp, Typ> {
    fn insert(&mut self, key: String, value: Signature<PTyp, Typ>) -> Option<Signature<PTyp, Typ>>{
        self.0.insert(key, value)
    }
    fn remove(&mut self, key: &String) -> Option<Signature<PTyp, Typ>>{
        self.0.remove(key)
    } 
    fn get(&self, s:&str) -> Option<&Signature<PTyp, Typ>> {
        self.0.get(s)
    }
}

impl TBuiltin<parser::Typ> for Typ {
    fn builtins(f: &str) -> Option<Signature<parser::Typ, Typ>> {
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
            "str::is_match" => sig(vec![Typ::Str, Typ::Regex], Typ::Bool),
            "regex::is_match" => sig(vec![Typ::Regex, Typ::Str], Typ::Bool),
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
            "list::is_subset" => sig(
                vec![
                    Typ::List(Box::new(Typ::Return)),
                    Typ::List(Box::new(Typ::Return)),
                ],
                Typ::Bool,
            ),
            "list::is_disjoint" => sig(
                vec![
                    Typ::List(Box::new(Typ::Return)),
                    Typ::List(Box::new(Typ::Return)),
                ],
                Typ::Bool,
            ),
            "list::difference" => sig(
                vec![
                    Typ::List(Box::new(Typ::Return)),
                    Typ::List(Box::new(Typ::Return)),
                ],
                Typ::List(Box::new(Typ::Return)),
            ),
            "list::intersection" => sig(
                vec![
                    Typ::List(Box::new(Typ::Return)),
                    Typ::List(Box::new(Typ::Return)),
                ],
                Typ::List(Box::new(Typ::Return)),
            ),
            "HttpRequest::GET" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::POST" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::PUT" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::DELETE" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::HEAD" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::OPTIONS" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::CONNECT" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::PATCH" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::TRACE" => sig(vec![], Typ::HttpRequest),
            "HttpRequest::connection" => sig(vec![Typ::HttpRequest], Typ::Connection),
            "HttpRequest::from" => sig(vec![Typ::HttpRequest], Typ::ID),
            "HttpRequest::to" => sig(vec![Typ::HttpRequest], Typ::ID),
            "HttpRequest::from_to" => {
                sig(vec![Typ::HttpRequest], Typ::Tuple(vec![Typ::ID, Typ::ID]))
            }
            "HttpRequest::set_from" => sig(vec![Typ::HttpRequest, Typ::ID], Typ::HttpRequest),
            "HttpRequest::set_to" => sig(vec![Typ::HttpRequest, Typ::ID], Typ::HttpRequest),
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
            "HttpRequest::set_connection" => {
                sig(vec![Typ::HttpRequest, Typ::Connection], Typ::HttpRequest)
            }
            "HttpResponse::new" => sig(vec![Typ::I64], Typ::HttpResponse),
            "HttpResponse::connection" => sig(vec![Typ::HttpResponse], Typ::Connection),
            "HttpResponse::from" => sig(vec![Typ::HttpResponse], Typ::ID),
            "HttpResponse::to" => sig(vec![Typ::HttpResponse], Typ::ID),
            "HttpResponse::from_to" => {
                sig(vec![Typ::HttpResponse], Typ::Tuple(vec![Typ::ID, Typ::ID]))
            }
            "HttpResponse::set_from" => sig(vec![Typ::HttpResponse, Typ::ID], Typ::HttpResponse),
            "HttpResponse::set_to" => sig(vec![Typ::HttpResponse, Typ::ID], Typ::HttpResponse),
            "HttpResponse::status" => sig(vec![Typ::HttpResponse], Typ::I64),
            "HttpResponse::version" => sig(vec![Typ::HttpResponse], Typ::Str),
            "HttpResponse::reason" => sig(vec![Typ::HttpResponse], Typ::Str.option()),
            "HttpResponse::set_reason" => sig(vec![Typ::HttpResponse, Typ::Str], Typ::HttpResponse),
            "HttpResponse::header" => sig(
                vec![Typ::HttpResponse, Typ::Str],
                Typ::List(Box::new(Typ::Data)).option(),
            ),
            "HttpResponse::unique_header" => {
                sig(vec![Typ::HttpResponse, Typ::Str], Typ::Data.option())
            }
            "HttpResponse::headers" => sig(vec![Typ::HttpResponse], Typ::List(Box::new(Typ::Str))),
            "HttpResponse::header_pairs" => sig(
                vec![Typ::HttpResponse],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Data]))),
            ),
            "HttpResponse::set_header" => sig(
                vec![Typ::HttpResponse, Typ::Str, Typ::Data],
                Typ::HttpResponse,
            ),
            "HttpResponse::set_connection" => {
                sig(vec![Typ::HttpResponse, Typ::Connection], Typ::HttpResponse)
            }
            "IpAddr::lookup" => sig(vec![Typ::Str], Typ::List(Box::new(Typ::IpAddr)).option()),
            "IpAddr::reverse_lookup" => sig(vec![Typ::IpAddr], Typ::Str.option()),
            "IpAddr::localhost" => sig(vec![], Typ::IpAddr),
            "IpAddr::from" => sig(vec![Typ::I64, Typ::I64, Typ::I64, Typ::I64], Typ::IpAddr),
            "IpAddr::octets" => sig(
                vec![Typ::IpAddr],
                Typ::Tuple(vec![Typ::I64, Typ::I64, Typ::I64, Typ::I64]),
            ),
            "ID::default" => sig(vec![], Typ::ID),
            "ID::labels" => sig(vec![Typ::ID], Typ::List(Box::new(Typ::Label))),
            "ID::hosts" => sig(vec![Typ::ID], Typ::List(Box::new(Typ::Str))),
            "ID::ips" => sig(vec![Typ::ID], Typ::List(Box::new(Typ::IpAddr))),
            "ID::port" => sig(vec![Typ::ID], Typ::I64.option()),
            "ID::add_label" => sig(vec![Typ::ID, Typ::Label], Typ::ID),
            "ID::add_host" => sig(vec![Typ::ID, Typ::Str], Typ::ID),
            "ID::add_ip" => sig(vec![Typ::ID, Typ::IpAddr], Typ::ID),
            "ID::set_port" => sig(vec![Typ::ID, Typ::I64], Typ::ID),
            "ID::has_label" => sig(vec![Typ::ID, Typ::Label], Typ::Bool),
            "ID::has_host" => sig(vec![Typ::ID, Typ::Str], Typ::Bool),
            "ID::has_ip" => sig(vec![Typ::ID, Typ::IpAddr], Typ::Bool),
            "Connection::default" => sig(vec![], Typ::Connection),
            "Connection::new" => sig(vec![Typ::ID, Typ::ID, Typ::I64], Typ::Connection),
            "Connection::from_to" => sig(vec![Typ::Connection], Typ::Tuple(vec![Typ::ID, Typ::ID])),
            "Connection::from" => sig(vec![Typ::Connection], Typ::ID),
            "Connection::to" => sig(vec![Typ::Connection], Typ::ID),
            "Connection::number" => sig(vec![Typ::Connection], Typ::I64),
            "Connection::set_from" => sig(vec![Typ::Connection, Typ::ID], Typ::Connection),
            "Connection::set_to" => sig(vec![Typ::Connection, Typ::ID], Typ::Connection),
            "Connection::set_number" => sig(vec![Typ::Connection, Typ::I64], Typ::Connection),
            "Label::captures" => sig(
                vec![Typ::Label, Typ::Label],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::Str, Typ::Str]))).option(),
            ),
            "Label::parts" => sig(vec![Typ::Label], Typ::List(Box::new(Typ::Str)).option()),
            "Label::is_match" => sig(vec![Typ::Label, Typ::Label], Typ::Bool),
            _ => None,
        }
    }
    fn internal_service(f: &str) -> Option<Signature<parser::Typ, Typ>> {
        let sig = |args, ty| Some(Signature::new(args, ty));
        match f {
            "Ingress::id" => sig(vec![], Typ::Label.option()),
            "Ingress::data" => sig(vec![], Typ::List(Box::new(Typ::Data))),
            "Ingress::has_label" => sig(vec![Typ::Label], Typ::Bool),
            "Egress::id" => sig(vec![], Typ::Label.option()),
            "Egress::set_id" => sig(vec![], Typ::Unit),
            "Egress::data" => sig(vec![], Typ::List(Box::new(Typ::Data))),
            "Egress::has_label" => sig(vec![Typ::Label], Typ::Bool),
            "Egress::push" => sig(vec![Typ::Data], Typ::Unit),
            "Egress::pop" => sig(vec![], Typ::Data.option()),
            "Egress::add_label" => sig(vec![Typ::Label], Typ::Unit),
            "Egress::remove_label" => sig(vec![Typ::Label], Typ::Unit),
            "Egress::wipe" => sig(vec![], Typ::Unit),
            _ => None,
        }
    }
}

impl TBuiltin<parser::CPTyp> for CPTyp {}