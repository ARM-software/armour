/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use super::types::{
    CPTyp, CPFlatTyp, CPSignature,
    Signature, DPTyp, DPSignature,
    FlatTyp, Typ, TBuiltin, TFlatTyp,
    TTyp
}; 
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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


#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Headers<FlatTyp:TFlatTyp >(pub BTreeMap<String, Signature<FlatTyp>>);
pub type DPHeaders = Headers<FlatTyp>;
pub type CPHeaders = Headers<CPFlatTyp>;

impl<FlatTyp:TFlatTyp> Headers<FlatTyp> {
    pub fn merge(&self, other: &Self) -> Self{
        Headers(self.0.clone().into_iter().chain(other.0.clone().into_iter()).collect())
    }
}

impl From<CPHeaders> for DPHeaders {
    fn from(cph: CPHeaders) -> Self{
        Headers( cph.0.into_iter().map(|(s, sig)| (s, DPSignature::from(sig))).collect())
    }
}

impl<FlatTyp:TFlatTyp> Default for Headers<FlatTyp> {
    fn default() -> Self {
        Headers(BTreeMap::new())
    }
}

pub trait THeaders<FlatTyp:TFlatTyp> {
    fn insert(&mut self, key: String, value: Signature<FlatTyp>) -> Option<Signature<FlatTyp>>;
    fn remove(&mut self, key: &String) -> Option<Signature<FlatTyp>>; 
    fn get(&self, s:&str) -> Option<&Signature<FlatTyp>>;

    fn add_function(&mut self, name: &str, sig: Signature<FlatTyp>) -> Result<(), Error> {
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
    fn typ(&self, name: &str) -> Option<Signature<FlatTyp>> {
        Typ::builtins(name)
            .or_else(|| Typ::internal_service(name).or_else(|| self.get(name).cloned()))
    }

    fn return_typ(&self, name: &str) -> Result<Typ<FlatTyp>, Error> {
        Ok(self
            .typ(name)
            .ok_or_else(|| Error::new("no current function"))?
            .typ())
    }

    fn is_builtin(name: &str) -> bool {
        let optsig : Option<Signature<FlatTyp>> = Typ::builtins(name);
        optsig.is_some() || name.parse::<usize>().is_ok()
    }
    fn is_internal(name: &str) -> bool {
        let optsig : Option<Signature<FlatTyp>> = Typ::internal_service(name);
        optsig.is_some() || Self::is_builtin(name)
    }
    fn split(name: &str) -> Option<(&str, &str)> {
        if let [module, method] = name.split("::").collect::<Vec<&str>>().as_slice() {
            Some((module, method))
        } else {
            None
        }
    }
    fn method(name: &str) -> Option<&str> {
        if let Some(Some(args)) = Typ::builtins(name).map(|ty:Signature<FlatTyp>| ty.args()) {
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
    fn resolve(name: &str, typs: &[Typ<FlatTyp>]) -> String {
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
impl<FlatTyp: TFlatTyp> THeaders<FlatTyp> for Headers<FlatTyp> {
    fn insert(
        &mut self, 
        key: String, 
        value: Signature<FlatTyp>
    ) -> Option<Signature<FlatTyp>>{
        self.0.insert(key, value)
    }
    fn remove(&mut self, key: &String) -> Option<Signature<FlatTyp>>{
        self.0.remove(key)
    } 
    fn get(&self, s:&str) -> Option<&Signature<FlatTyp>> {
        self.0.get(s)
    }
}

impl TBuiltin<FlatTyp> for FlatTyp {
    fn builtins(f: &str) -> Option<DPSignature> {
        let sig = |args:Vec<FlatTyp>, ty| Some(
            Signature::new(
                args.into_iter().map(|x:FlatTyp| Typ::FlatTyp(x)).collect(),
                Typ::FlatTyp(ty)
            )
        );
        match f {
            "option::Some" => sig(vec![FlatTyp::Return], FlatTyp::Return),
            "i64::abs" => sig(vec![FlatTyp::I64], FlatTyp::I64),
            "i64::to_str" => sig(vec![FlatTyp::I64], FlatTyp::Str),
            "str::len" => sig(vec![FlatTyp::Str], FlatTyp::I64),
            "str::to_lowercase" => sig(vec![FlatTyp::Str], FlatTyp::Str),
            "str::to_uppercase" => sig(vec![FlatTyp::Str], FlatTyp::Str),
            "str::trim_start" => sig(vec![FlatTyp::Str], FlatTyp::Str),
            "str::trim_end" => sig(vec![FlatTyp::Str], FlatTyp::Str),
            "str::as_bytes" => sig(vec![FlatTyp::Str], FlatTyp::Data),
            "str::from_utf8" => sig(vec![FlatTyp::Data], FlatTyp::Str),
            "str::to_base64" => sig(vec![FlatTyp::Str], FlatTyp::Str),
            "str::is_match" => sig(
                vec![FlatTyp::Str, FlatTyp::Regex], 
                FlatTyp::Bool
            ),
            "regex::is_match" => sig(
                vec![FlatTyp::Regex, FlatTyp::Str], 
                FlatTyp::Bool
            ),
            "data::to_base64" => sig(vec![FlatTyp::Data], FlatTyp::Str),
            "data::len" => sig(vec![FlatTyp::Data], FlatTyp::I64),
            "i64::pow" => sig(vec![FlatTyp::I64, FlatTyp::I64], FlatTyp::I64),
            "i64::min" => sig(vec![FlatTyp::I64, FlatTyp::I64], FlatTyp::I64),
            "i64::max" => sig(vec![FlatTyp::I64, FlatTyp::I64], FlatTyp::I64),
            "str::starts_with" => sig(
                vec![FlatTyp::Str, FlatTyp::Str], 
                FlatTyp::Bool
            ),
            "str::ends_with" => sig(
                vec![FlatTyp::Str, FlatTyp::Str], 
                FlatTyp::Bool
            ),
            "str::contains" => sig(
                vec![FlatTyp::Str, FlatTyp::Str], 
                FlatTyp::Bool
            ),
            "HttpRequest::GET" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::POST" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::PUT" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::DELETE" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::HEAD" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::OPTIONS" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::CONNECT" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::PATCH" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::TRACE" => sig(vec![], FlatTyp::HttpRequest),
            "HttpRequest::connection" => sig(
                vec![FlatTyp::HttpRequest], 
                FlatTyp::Connection
            ),
            "HttpRequest::from" => sig(vec![FlatTyp::HttpRequest], FlatTyp::ID),
            "HttpRequest::to" => sig(vec![FlatTyp::HttpRequest], FlatTyp::ID),
            "HttpRequest::set_from" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::ID],
                FlatTyp::HttpRequest
            ),
            "HttpRequest::set_to" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::ID], 
                FlatTyp::HttpRequest
            ),
            "HttpRequest::method" => sig(vec![FlatTyp::HttpRequest], FlatTyp::Str),
            "HttpRequest::version" => sig(vec![FlatTyp::HttpRequest], FlatTyp::Str),
            "HttpRequest::path" => sig(vec![FlatTyp::HttpRequest], FlatTyp::Str),
            "HttpRequest::query" => sig(vec![FlatTyp::HttpRequest], FlatTyp::Str),
            "HttpRequest::set_path" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::Str], 
                FlatTyp::HttpRequest
            ),
            "HttpRequest::set_query" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::Str], 
                FlatTyp::HttpRequest
            ),
            "HttpRequest::set_header" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::Str, FlatTyp::Data],
                FlatTyp::HttpRequest,
            ),
            "HttpRequest::set_connection" => sig(
                vec![FlatTyp::HttpRequest, FlatTyp::Connection], 
                FlatTyp::HttpRequest
            ),
            "HttpResponse::new" => sig(vec![FlatTyp::I64], FlatTyp::HttpResponse),
            "HttpResponse::connection" => sig(vec![FlatTyp::HttpResponse], FlatTyp::Connection),
            "HttpResponse::from" => sig(vec![FlatTyp::HttpResponse], FlatTyp::ID),
            "HttpResponse::to" => sig(vec![FlatTyp::HttpResponse], FlatTyp::ID),
            "HttpResponse::set_from" => sig(
                vec![FlatTyp::HttpResponse, FlatTyp::ID], 
                FlatTyp::HttpResponse
            ),
            "HttpResponse::set_to" => sig(
                vec![FlatTyp::HttpResponse, FlatTyp::ID], 
                FlatTyp::HttpResponse
            ),
            "HttpResponse::status" => sig(vec![FlatTyp::HttpResponse], FlatTyp::I64),
            "HttpResponse::version" => sig(vec![FlatTyp::HttpResponse], FlatTyp::Str),
            "HttpResponse::set_reason" => sig(
                vec![FlatTyp::HttpResponse, FlatTyp::Str], 
                FlatTyp::HttpResponse
            ),
            "HttpResponse::set_header" => sig(
                vec![FlatTyp::HttpResponse, FlatTyp::Str, FlatTyp::Data],
                FlatTyp::HttpResponse,
            ),
            "HttpResponse::set_connection" => {
                sig(vec![FlatTyp::HttpResponse, FlatTyp::Connection], FlatTyp::HttpResponse)
            }
            "IpAddr::localhost" => sig(vec![], FlatTyp::IpAddr),
            "IpAddr::from" => sig(
                vec![FlatTyp::I64, FlatTyp::I64, FlatTyp::I64, FlatTyp::I64], 
                FlatTyp::IpAddr
            ),
            "ID::default" => sig(vec![], FlatTyp::ID),
            "ID::add_label" => sig(vec![FlatTyp::ID, FlatTyp::Label], FlatTyp::ID),
            "ID::add_host" => sig(vec![FlatTyp::ID, FlatTyp::Str], FlatTyp::ID),
            "ID::add_ip" => sig(vec![FlatTyp::ID, FlatTyp::IpAddr], FlatTyp::ID),
            "ID::set_port" => sig(vec![FlatTyp::ID, FlatTyp::I64], FlatTyp::ID),
            "ID::has_label" => sig(vec![FlatTyp::ID, FlatTyp::Label], FlatTyp::Bool),
            "ID::has_host" => sig(vec![FlatTyp::ID, FlatTyp::Str], FlatTyp::Bool),
            "ID::has_ip" => sig(vec![FlatTyp::ID, FlatTyp::IpAddr], FlatTyp::Bool),
            "Connection::default" => sig(vec![], FlatTyp::Connection),
            "Connection::new" => sig(
                vec![FlatTyp::ID, FlatTyp::ID, FlatTyp::I64], 
                FlatTyp::Connection
            ),
            "Connection::from" => sig(vec![FlatTyp::Connection], FlatTyp::ID),
            "Connection::to" => sig(vec![FlatTyp::Connection], FlatTyp::ID),
            "Connection::number" => sig(vec![FlatTyp::Connection], FlatTyp::I64),
            "Connection::set_from" => sig(
                vec![FlatTyp::Connection, FlatTyp::ID], 
                FlatTyp::Connection
            ),
            "Connection::set_to" => sig(
                vec![FlatTyp::Connection, FlatTyp::ID], 
                FlatTyp::Connection
            ),
            "Connection::set_number" => sig(
                vec![FlatTyp::Connection, FlatTyp::I64], 
                FlatTyp::Connection
            ),
            "Label::is_match" => sig(
                vec![FlatTyp::Label, FlatTyp::Label], 
                FlatTyp::Bool
            ),
            "System::getCurrentTime" => sig(vec![], FlatTyp::I64), 
            _ => None,
        }
    }
    fn internal_service(f: &str) -> Option<DPSignature> {
        let sig = |args:Vec<FlatTyp>, ty| Some(
            Signature::new(
                args.into_iter().map(|x:FlatTyp| Typ::FlatTyp(x)).collect(), 
                Typ::FlatTyp(ty)
            )
        );
        match f {
            "Ingress::has_label" => sig(vec![FlatTyp::Label], FlatTyp::Bool),
            "Egress::set_id" => sig(vec![], FlatTyp::Unit),
            "Egress::has_label" => sig(vec![FlatTyp::Label], FlatTyp::Bool),
            "Egress::push" => sig(vec![FlatTyp::Data], FlatTyp::Unit),
            "Egress::add_label" => sig(vec![FlatTyp::Label], FlatTyp::Unit),
            "Egress::remove_label" => sig(vec![FlatTyp::Label], FlatTyp::Unit),
            "Egress::wipe" => sig(vec![], FlatTyp::Unit),
            _ => None,
        }
    }
}
impl<FlatTyp:TFlatTyp> TBuiltin<FlatTyp> for Typ<FlatTyp> {
    fn builtins(f: &str) -> Option<Signature<FlatTyp>> {
        let sig = |args, ty| Some(Signature::new(args, ty));
        match f {
            "option::is_none" => sig(vec![Typ::any_option()], Typ::bool()),
            "option::is_some" => sig(vec![Typ::any_option()], Typ::bool()),
            "list::len" => sig(
                vec![Typ::List(Box::new(Typ::rreturn()))], 
                Typ::i64()
            ),
            "list::reduce" => sig(
                vec![Typ::List(Box::new(Typ::rreturn()))], 
                Typ::rreturn().option()
            ),
            "list::is_subset" => sig(
                vec![
                    Typ::List(Box::new(Typ::rreturn())),
                    Typ::List(Box::new(Typ::rreturn())),
                ],
                Typ::bool(),
            ),
            "list::is_disjoint" => sig(
                vec![
                    Typ::List(Box::new(Typ::rreturn())),
                    Typ::List(Box::new(Typ::rreturn())),
                ],
                Typ::bool(),
            ),
            "list::difference" => sig(
                vec![
                    Typ::List(Box::new(Typ::rreturn())),
                    Typ::List(Box::new(Typ::rreturn())),
                ],
                Typ::List(Box::new(Typ::rreturn())),
            ),
            "list::intersection" => sig(
                vec![
                    Typ::List(Box::new(Typ::rreturn())),
                    Typ::List(Box::new(Typ::rreturn())),
                ],
                Typ::List(Box::new(Typ::rreturn())),
            ),
            "HttpRequest::query_pairs" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_request())],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::str(), Typ::str()]))),
            ),
            "HttpRequest::from_to" => sig(
                vec![Typ::http_request()], 
                Typ::Tuple(vec![Typ::id(), Typ::id()])
            ),
            "HttpRequest::header" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_request()), Typ::str()],
                Typ::List(Box::new(Typ::data())).option(),
            ),
            "HttpRequest::header_pairs" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_request())],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::str(), Typ::data()]))),
            ),
            "HttpResponse::reason" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response())], 
                Typ::str().option()
            ),
            "HttpRequest::unique_header" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_request()), 
                Typ::str()], Typ::data().option()
            ),
            "HttpResponse::from_to" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response())],
                Typ::Tuple(vec![Typ::id(), Typ::id()])
            ),
            "HttpResponse::header" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response()), Typ::str()],
                Typ::List(Box::new(Typ::data())).option(),
            ),
            "HttpResponse::headers" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response())], 
                Typ::List(Box::new(Typ::str()))
            ),
            "HttpResponse::header_pairs" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response())],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::str(), Typ::data()]))),
            ),
            "HttpResponse::unique_header" => sig(
                vec![Typ::FlatTyp(FlatTyp::http_response()), 
                Typ::str()], Typ::data().option()
            ),
            "IpAddr::lookup" => sig(
                vec![Typ::str()], 
                Typ::List(Box::new(Typ::ip_addr())).option()
            ),
            "IpAddr::octets" => sig(
                vec![Typ::ip_addr()],
                Typ::Tuple(vec![Typ::i64(), Typ::i64(), Typ::i64(), Typ::i64()]),
            ),
            "ID::find_label" => sig(vec![Typ::id(), Typ::label()], Typ::label().option()),
            "ID::labels" => sig(vec![Typ::id()], Typ::List(Box::new(Typ::label()))),
            "ID::hosts" => sig(vec![Typ::id()], Typ::List(Box::new(Typ::str()))),
            "ID::ips" => sig(vec![Typ::id()], Typ::List(Box::new(Typ::ip_addr()))),
            "ID::port" => sig(vec![Typ::id()], Typ::i64().option()),
            "Label::captures" => sig(
                vec![Typ::label(), Typ::label()],
                Typ::List(Box::new(Typ::Tuple(vec![Typ::str(), Typ::str()]))).option(),
            ),
            "Label::concat" => sig(
                vec![Typ::label(), Typ::label()],
                Typ::label(),
            ),
            "Label::parts" => sig(
                vec![Typ::label()], 
                Typ::List(Box::new(Typ::str())).option()
            ),
            f => FlatTyp::builtins(f),
        }
    }
    fn internal_service(f: &str) -> Option<Signature<FlatTyp>> {
        let sig = |args, ty| Some(Signature::new(args, ty));
        match f {
            "Egress::find_label" => sig(vec![Typ::label()], Typ::label().option()),
            "Egress::id" => sig(vec![], Typ::label().option()),
            "Egress::data" => sig(vec![], Typ::List(Box::new(Typ::data()))),
            "Egress::pop" => sig(vec![], Typ::data().option()),
            "Ingress::id" => sig(vec![], Typ::label().option()),
            "Ingress::data" => sig(vec![], Typ::List(Box::new(Typ::data()))),
            "Ingress::find_label" => sig(vec![Typ::label()], Typ::label().option()),
            f => FlatTyp::internal_service(f)
        }
    }
}

impl TBuiltin<CPFlatTyp> for CPFlatTyp {
    fn builtins(f: &str) -> Option<CPSignature> {
        let sig = |args:Vec<CPTyp>, ty| Some(Signature::new(args, ty));
        let convertsig = |sigopt:Option<DPSignature>| match sigopt {
            None => None,
            Some(sig) => { 
                match sig.split() { 
                    (None, ty) => Some(Signature::new_noargs(CPTyp::from(ty))),
                    (Some(args), ty) => Some(
                        Signature::new(
                            args.into_iter().map(|ty:DPTyp| CPTyp::from(ty)).collect(),
                            CPTyp::from(ty)
                        )
                    ) 
                }
            }
        };

        match f {
            //Onboarding policy
            "allow_egress" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Policy)),
            "allow_ingress" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Policy)),
            "Primitive::allow_rest_request" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Primitive)),
            "Primitive::allow_rest_response" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Primitive)),
            "Primitive::allow_tcp_connection" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Primitive)),
            "compile_egress" => sig(
                vec![CPTyp::primitive(), CPTyp::id()], 
                CPTyp::FlatTyp(CPFlatTyp::Policy)
            ),
            "compile_ingress" => sig(
                vec![CPTyp::primitive(), CPTyp::id()], 
                CPTyp::FlatTyp(CPFlatTyp::Policy)
            ),
            "deny_egress" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Policy)),
            "deny_ingress" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Policy)),
            "Primitive::on_tcp_disconnect" => sig(vec![], CPTyp::FlatTyp(CPFlatTyp::Primitive)),
            "ControlPlane::onboard" => sig(vec![CPTyp::id()], CPTyp::bool()),
            "ControlPlane::onboarded" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData)], 
                CPTyp::id().option()
            ),
            "ControlPlane::newID" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData)], 
                CPTyp::id()
            ),
            "ControlPlane::verify_credentials" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData), CPTyp::label()],//TODO add a second arg 
                CPTyp::bool()
            ),
            "Label::new" => sig(vec![CPTyp::str()], CPTyp::label()),
            "Label::login_time" => sig(vec![CPTyp::i64()], CPTyp::label()),
            "OnboardingData::proposed_labels" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData)],
                CPTyp::List(Box::new(CPTyp::label()))
            ),
            "OnboardingData::has_proposed_label" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData), CPTyp::label()], 
                CPTyp::bool()
            ),
            "OnboardingData::has_ip" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData), CPTyp::ip_addr()], 
                CPTyp::bool()
            ),
            "OnboardingData::host" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData)], 
                CPTyp::label()
            ),
            "OnboardingData::service" => sig(
                vec![CPTyp::FlatTyp(CPFlatTyp::OnboardingData)], 
                CPTyp::label()
            ),
            "OnboardingResult::Ok" => sig(
                vec![
                    CPTyp::id(), 
                    CPTyp::FlatTyp(CPFlatTyp::Policy), 
                    CPTyp::FlatTyp(CPFlatTyp::Policy)
                ],
                CPTyp::FlatTyp(CPFlatTyp::OnboardingResult)),
            "OnboardingResult::Err" => sig(
                vec![
                    CPTyp::str(),
                    CPTyp::id(), 
                    CPTyp::FlatTyp(CPFlatTyp::Policy), 
                    CPTyp::FlatTyp(CPFlatTyp::Policy)
                ],
                CPTyp::FlatTyp(CPFlatTyp::OnboardingResult)),
            "OnboardingResult::ErrID" => sig(
                vec![
                    CPTyp::str(),
                    CPTyp::id(), 
                ],
                CPTyp::FlatTyp(CPFlatTyp::OnboardingResult)),
            "OnboardingResult::ErrStr" => sig(
                vec![
                    CPTyp::str(),
                ],
                CPTyp::FlatTyp(CPFlatTyp::OnboardingResult)),
            _ => convertsig(FlatTyp::builtins(f)),
        }
    }
    fn internal_service(f: &str) -> Option<CPSignature> {
        let convertsig = |sigopt:Option<DPSignature>| match sigopt {
            None => None,
            Some(sig) => { 
                match sig.split() { 
                    (None, ty) => Some(Signature::new_noargs(CPTyp::from(ty))),
                    (Some(args), ty) => Some(
                        Signature::new(
                            args.into_iter().map(|ty:DPTyp| CPTyp::from(ty)).collect(),
                            CPTyp::from(ty)
                        )
                    ) 
                }
            }
        };
        match f {
            _ => convertsig(FlatTyp::internal_service(f)),
        }
    }
}