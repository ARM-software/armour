use super::types::{Signature, Typ};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Error(pub String);

impl Error {
    pub fn new(e: &str) -> Error {
        Error(e.to_string())
    }
}

#[derive(Clone)]
pub struct Headers {
    functions: HashMap<String, Signature>,
    return_typ: Option<Typ>,
}

impl Headers {
    pub fn new() -> Headers {
        Headers {
            functions: HashMap::new(),
            return_typ: None,
        }
    }
    pub fn add_function(&mut self, name: &str, args: Vec<Typ>, ret: &Typ) -> Result<(), Error> {
        if self
            .functions
            .insert(name.to_string(), (args, ret.to_owned()))
            .is_some()
        {
            Err(Error::new(&format!("duplicate function \"{}\"", name)))
        } else {
            Ok(())
        }
    }
    pub fn return_typ(&self) -> Option<Typ> {
        self.return_typ.clone()
    }
    pub fn clear_return_typ(&mut self) {
        self.return_typ = None
    }
    pub fn set_return_typ(&mut self, typ: Typ) {
        self.return_typ = Some(typ)
    }
    pub fn return_typ_for_function(&mut self, name: &str) -> Result<Typ, Error> {
        Ok(self.typ(name).ok_or(Error::new("no current function"))?.1)
    }
    pub fn builtins(f: &str) -> Option<Signature> {
        match f {
            "i64::abs" => Some((vec![Typ::I64], Typ::I64)),
            "str::len" => Some((vec![Typ::Str], Typ::I64)),
            "str::to_lowercase" => Some((vec![Typ::Str], Typ::Str)),
            "str::to_uppercase" => Some((vec![Typ::Str], Typ::Str)),
            "str::trim_start" => Some((vec![Typ::Str], Typ::Str)),
            "str::trim_end" => Some((vec![Typ::Str], Typ::Str)),
            "str::as_bytes" => Some((vec![Typ::Str], Typ::Data)),
            "str::from_utf8" => Some((vec![Typ::Data], Typ::Str)),
            "data::len" => Some((vec![Typ::Str], Typ::I64)),
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
            "HttpRequest::query" => Some((vec![Typ::HttpRequest], Typ::Str)),
            "HttpRequest::headers" => Some((vec![Typ::HttpRequest], Typ::List(Box::new(Typ::Str)))),
            "HttpRequest::set_header" => {
                Some((vec![Typ::HttpRequest, Typ::Str, Typ::Str], Typ::HttpRequest))
            }
            "HttpRequest::header" => Some((
                vec![Typ::HttpRequest, Typ::Str],
                Typ::List(Box::new(Typ::Str)),
            )),
            _ => None,
        }
    }
    pub fn is_builtin(name: &str) -> bool {
        Headers::builtins(name).is_some()
    }
    pub fn typ(&self, name: &str) -> Option<Signature> {
        (Headers::builtins(name).or(self.functions.get(name).cloned()))
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
