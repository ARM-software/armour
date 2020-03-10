/// policy language interpreter
// NOTE: no optimization
use super::expressions::{Block, Error, Expr, Pattern};
use super::externals::{Call, ExternalActor};
use super::headers::Headers;
use super::labels::Label;
use super::lang::{Code, Program};
use super::literals::{Connection, HttpRequest, HttpResponse, Literal, Method, VecSet};
use super::meta::{Egress, IngressEgress, Meta};
use super::parser::{As, Infix, Iter, Pat, PolicyRegex, Prefix};
use actix::prelude::*;
use futures::future::{BoxFuture, FutureExt};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct Env {
    internal: Arc<Code>,
    external: Addr<ExternalActor>,
    meta: Addr<IngressEgress>,
}

impl Default for Env {
    fn default() -> Self {
        Env::new(&Program::default())
    }
}

impl Env {
    pub fn new(prog: &Program) -> Self {
        Env {
            internal: Arc::new(prog.code.clone()),
            external: ExternalActor::new(prog).start(),
            meta: IngressEgress::start_default(),
        }
    }
    fn get(&self, name: &str) -> Option<Expr> {
        self.internal.0.get(name).cloned()
    }
    pub fn set_meta(&mut self, meta: IngressEgress) {
        self.meta = meta.start()
    }
    pub async fn egress(&self) -> Option<Meta> {
        self.meta.send(Egress).await.ok()?.ok()
    }
}

impl Literal {
    fn eval_prefix(&self, p: &Prefix) -> Option<Self> {
        match (p, self) {
            (Prefix::Not, Literal::Bool(b)) => Some(Literal::Bool(!b)),
            (Prefix::Minus, Literal::Int(i)) => Some(Literal::Int(-i)),
            _ => None,
        }
    }
    fn eval_infix(&self, op: &Infix, other: &Literal) -> Option<Self> {
        match (op, self, other) {
            (Infix::Equal, _, _) => Some(Literal::Bool(self == other)),
            (Infix::NotEqual, _, _) => Some(Literal::Bool(self != other)),
            (Infix::Plus, Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i + j)),
            (Infix::Minus, Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i - j)),
            (Infix::Divide, Literal::Int(_), Literal::Int(0)) => None,
            (Infix::Divide, Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i / j)),
            (Infix::Multiply, Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i * j)),
            (Infix::Remainder, Literal::Int(_), Literal::Int(0)) => None,
            (Infix::Remainder, Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i % j)),
            (Infix::LessThan, Literal::Int(i), Literal::Int(j)) => Some(Literal::Bool(i < j)),
            (Infix::LessThanEqual, Literal::Int(i), Literal::Int(j)) => Some(Literal::Bool(i <= j)),
            (Infix::GreaterThan, Literal::Int(i), Literal::Int(j)) => Some(Literal::Bool(i > j)),
            (Infix::GreaterThanEqual, Literal::Int(i), Literal::Int(j)) => {
                Some(Literal::Bool(i >= j))
            }
            (Infix::And, Literal::Bool(i), Literal::Bool(j)) => Some(Literal::Bool(*i && *j)),
            (Infix::Or, Literal::Bool(i), Literal::Bool(j)) => Some(Literal::Bool(*i || *j)),
            (Infix::Concat, Literal::List(i), Literal::List(j)) => {
                let mut k = i.clone();
                k.append(&mut j.clone());
                Some(Literal::List(k))
            }
            (Infix::ConcatStr, Literal::Str(i), Literal::Str(j)) => {
                Some(Literal::Str(format!("{}{}", i, j)))
            }
            (Infix::In, _, Literal::List(l)) => Some(VecSet::contains(l, self)),
            _ => None,
        }
    }
    fn eval_call0(f: &str) -> Option<Self> {
        match f {
            "HttpRequest::GET" => Some(HttpRequest::default().into()),
            "HttpRequest::POST" => Some(Method::POST.into()),
            "HttpRequest::PUT" => Some(Method::PUT.into()),
            "HttpRequest::DELETE" => Some(Method::DELETE.into()),
            "HttpRequest::HEAD" => Some(Method::HEAD.into()),
            "HttpRequest::OPTIONS" => Some(Method::OPTIONS.into()),
            "HttpRequest::CONNECT" => Some(Method::CONNECT.into()),
            "HttpRequest::PATCH" => Some(Method::PATCH.into()),
            "HttpRequest::TRACE" => Some(Method::TRACE.into()),
            "ID::default" => Some(Literal::ID(Default::default())),
            "Connection::default" => Some(Literal::Connection(Default::default())),
            "IpAddr::localhost" => Some(Literal::IpAddr(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(127, 0, 0, 1),
            ))),
            _ => None,
        }
    }
    fn eval_call1(&self, f: &str) -> Option<Self> {
        match (f, self) {
            ("option::Some", _) => Some(self.some()),
            ("option::is_none", Literal::Tuple(t)) => Some(Literal::Bool(t.is_empty())),
            ("option::is_some", Literal::Tuple(t)) => Some(Literal::Bool(t.len() == 1)),
            ("i64::abs", Literal::Int(i)) => Some(Literal::Int(i.abs())),
            ("i64::to_str", Literal::Int(i)) => Some(Literal::Str(i.to_string())),
            ("str::len", Literal::Str(s)) => Some(Literal::Int(s.len() as i64)),
            ("str::to_lowercase", Literal::Str(s)) => Some(Literal::Str(s.to_lowercase())),
            ("str::to_uppercase", Literal::Str(s)) => Some(Literal::Str(s.to_uppercase())),
            ("str::trim_start", Literal::Str(s)) => Some(Literal::Str(s.trim_start().to_string())),
            ("str::trim_end", Literal::Str(s)) => Some(Literal::Str(s.trim_end().to_string())),
            ("str::as_bytes", Literal::Str(s)) => Some(Literal::Data(s.as_bytes().to_vec())),
            ("str::from_utf8", Literal::Data(s)) => Some(Literal::Str(
                std::string::String::from_utf8_lossy(s).to_string(),
            )),
            ("str::to_base64", Literal::Str(s)) => Some(Literal::Str(base64::encode(s))),
            ("data::to_base64", Literal::Data(d)) => Some(Literal::Str(base64::encode(d))),
            ("data::len", Literal::Data(d)) => Some(Literal::Int(d.len() as i64)),
            ("HttpRequest::connection", Literal::HttpRequest(req)) => Some(req.connection()),
            ("HttpRequest::from", Literal::HttpRequest(req)) => Some(req.from_lit()),
            ("HttpRequest::to", Literal::HttpRequest(req)) => Some(req.to_lit()),
            ("HttpRequest::method", Literal::HttpRequest(req)) => Some(req.method()),
            ("HttpRequest::version", Literal::HttpRequest(req)) => Some(req.version()),
            ("HttpRequest::path", Literal::HttpRequest(req)) => Some(req.path()),
            ("HttpRequest::route", Literal::HttpRequest(req)) => Some(req.route()),
            ("HttpRequest::query", Literal::HttpRequest(req)) => Some(req.query()),
            ("HttpRequest::query_pairs", Literal::HttpRequest(req)) => Some(req.query_pairs()),
            ("HttpRequest::header_pairs", Literal::HttpRequest(req)) => Some(req.header_pairs()),
            ("HttpRequest::headers", Literal::HttpRequest(req)) => Some(req.headers()),
            ("HttpResponse::new", Literal::Int(code)) => Some(HttpResponse::literal(*code as u16)),
            ("HttpResponse::connection", Literal::HttpResponse(res)) => Some(res.connection()),
            ("HttpResponse::from", Literal::HttpResponse(res)) => Some(res.from_lit()),
            ("HttpResponse::to", Literal::HttpResponse(res)) => Some(res.to_lit()),
            ("HttpResponse::status", Literal::HttpResponse(res)) => Some(res.status()),
            ("HttpResponse::version", Literal::HttpResponse(res)) => Some(res.version()),
            ("HttpResponse::reason", Literal::HttpResponse(res)) => Some(res.reason()),
            ("HttpResponse::header_pairs", Literal::HttpResponse(req)) => Some(req.header_pairs()),
            ("HttpResponse::headers", Literal::HttpResponse(req)) => Some(req.headers()),
            ("list::len", Literal::List(l)) => Some(Literal::Int(l.len() as i64)),
            ("list::reduce", Literal::List(l)) => {
                if let Some(v) = l.get(0) {
                    if l.iter().all(|w| v == w) {
                        Some(v.some())
                    } else {
                        Some(Literal::none())
                    }
                } else {
                    Some(Literal::none())
                }
            }
            ("IpAddr::octets", Literal::IpAddr(ip)) => Some(Literal::from(ip)),
            ("ID::labels", Literal::ID(id)) => Some(id.labels()),
            ("ID::hosts", Literal::ID(id)) => Some(id.hosts()),
            ("ID::ips", Literal::ID(id)) => Some(id.ips()),
            ("ID::port", Literal::ID(id)) => Some(id.port()),
            (_, Literal::Tuple(l)) => {
                if let Ok(i) = f.parse::<usize>() {
                    l.get(i).cloned()
                } else {
                    None
                }
            }
            ("Connection::from_to", Literal::Connection(c)) => Some(c.from_to()),
            ("Connection::from", Literal::Connection(c)) => Some(c.from_lit()),
            ("Connection::to", Literal::Connection(c)) => Some(c.to_lit()),
            ("Connection::number", Literal::Connection(c)) => Some(c.number()),
            ("Label::parts", Literal::Label(l)) => Some(l.parts().into()),
            ("IpAddr::reverse_lookup", Literal::IpAddr(ip)) => {
                Some(if let Ok(res) = dns_lookup::lookup_addr(ip) {
                    Literal::Str(res).some()
                } else {
                    Literal::none()
                })
            }
            ("IpAddr::lookup", Literal::Str(name)) => {
                Some(if let Ok(res) = dns_lookup::lookup_host(name) {
                    Literal::List(
                        res.iter()
                            .filter_map(|ip| {
                                if ip.is_ipv4() {
                                    Some(Literal::IpAddr(*ip))
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                    .some()
                } else {
                    Literal::none()
                })
            }
            _ => None,
        }
    }
    fn eval_call2(&self, f: &str, other: &Literal) -> Option<Self> {
        match (f, self, other) {
            ("i64::pow", Literal::Int(i), Literal::Int(j)) => Some(Literal::Int(i.pow(*j as u32))),
            ("i64::min", Literal::Int(i), Literal::Int(j)) => {
                Some(Literal::Int(std::cmp::min(*i, *j)))
            }
            ("i64::max", Literal::Int(i), Literal::Int(j)) => {
                Some(Literal::Int(std::cmp::max(*i, *j)))
            }
            ("str::starts_with", Literal::Str(i), Literal::Str(j)) => {
                Some(Literal::Bool(i.starts_with(j)))
            }
            ("str::ends_with", Literal::Str(i), Literal::Str(j)) => {
                Some(Literal::Bool(i.ends_with(j)))
            }
            ("str::is_match", Literal::Str(s), Literal::Regex(r))
            | ("Regex::is_match", Literal::Regex(r), Literal::Str(s)) => {
                Some(Literal::Bool(r.is_match(s)))
            }
            ("str::contains", Literal::Str(i), Literal::Str(j)) => {
                Some(Literal::Bool(i.contains(j)))
            }
            ("HttpRequest::set_path", Literal::HttpRequest(req), Literal::Str(q)) => {
                Some(req.set_path(q).into())
            }
            ("HttpRequest::set_query", Literal::HttpRequest(req), Literal::Str(q)) => {
                Some(req.set_query(q).into())
            }
            ("HttpRequest::header", Literal::HttpRequest(req), Literal::Str(h)) => {
                Some(req.header(&h))
            }
            ("HttpRequest::unique_header", Literal::HttpRequest(req), Literal::Str(h)) => {
                Some(req.unique_header(&h))
            }
            ("HttpRequest::set_connection", Literal::HttpRequest(req), Literal::Connection(c)) => {
                Some(req.set_connection(c).into())
            }
            ("HttpRequest::set_from", Literal::HttpRequest(req), Literal::ID(f)) => {
                Some(req.set_from(f).into())
            }
            ("HttpRequest::set_to", Literal::HttpRequest(req), Literal::ID(f)) => {
                Some(req.set_to(f).into())
            }
            ("HttpResponse::header", Literal::HttpResponse(res), Literal::Str(h)) => {
                Some(res.header(&h))
            }
            ("HttpResponse::unique_header", Literal::HttpResponse(res), Literal::Str(h)) => {
                Some(res.unique_header(&h))
            }
            ("HttpResponse::set_reason", Literal::HttpResponse(res), Literal::Str(q)) => {
                Some(res.set_reason(q).into())
            }
            (
                "HttpResponse::set_connection",
                Literal::HttpResponse(res),
                Literal::Connection(c),
            ) => Some(res.set_connection(c).into()),
            ("HttpResponse::set_from", Literal::HttpResponse(res), Literal::ID(f)) => {
                Some(res.set_from(f).into())
            }
            ("HttpResponse::set_to", Literal::HttpResponse(res), Literal::ID(f)) => {
                Some(res.set_to(f).into())
            }
            ("ID::has_label", Literal::ID(id), Literal::Label(l)) => Some(id.has_label(l).into()),
            ("ID::add_label", Literal::ID(id), Literal::Label(l)) => {
                Some(Literal::ID(id.add_label(l)))
            }
            ("ID::has_host", Literal::ID(id), Literal::Str(h)) => Some(id.has_host(h).into()),
            ("ID::add_host", Literal::ID(id), Literal::Str(h)) => Some(Literal::ID(id.add_host(h))),
            ("ID::has_ip", Literal::ID(id), Literal::IpAddr(i)) => Some(id.has_ip(i).into()),
            ("ID::add_ip", Literal::ID(id), Literal::IpAddr(i)) => Some(Literal::ID(id.add_ip(*i))),
            ("ID::set_port", Literal::ID(id), Literal::Int(q)) => {
                Some(Literal::ID(id.set_port(*q as u16)))
            }
            ("list::is_subset", Literal::List(i), Literal::List(j)) => {
                Some(VecSet::is_subset(i, j))
            }
            ("list::is_disjoint", Literal::List(i), Literal::List(j)) => {
                Some(VecSet::is_disjoint(i, j))
            }
            ("list::difference", Literal::List(i), Literal::List(j)) => {
                Some(VecSet::difference(i, j))
            }
            ("list::intersection", Literal::List(i), Literal::List(j)) => {
                Some(VecSet::intersection(i, j))
            }
            ("Connection::set_from", Literal::Connection(c), Literal::ID(f)) => {
                Some(c.set_from(f).into())
            }
            ("Connection::set_to", Literal::Connection(c), Literal::ID(f)) => {
                Some(c.set_to(f).into())
            }
            ("Connection::set_number", Literal::Connection(c), Literal::Int(n)) => {
                Some(c.set_number(*n).into())
            }
            ("Label::captures", Literal::Label(i), Literal::Label(j)) => {
                Some(i.match_with(j).into())
            }
            ("Label::is_match", Literal::Label(i), Literal::Label(j)) => {
                Some(i.matches_with(j).into())
            }
            _ => None,
        }
    }
    fn eval_call3(&self, f: &str, l1: &Literal, l2: &Literal) -> Option<Self> {
        match (f, self, l1, l2) {
            (
                "HttpRequest::set_header",
                Literal::HttpRequest(req),
                Literal::Str(h),
                Literal::Data(v),
            ) => Some(req.set_header(h, v).into()),
            (
                "HttpResponse::set_header",
                Literal::HttpResponse(res),
                Literal::Str(h),
                Literal::Data(v),
            ) => Some(res.set_header(h, v).into()),
            ("Connection::new", Literal::ID(from), Literal::ID(to), Literal::Int(number)) => {
                Some(Connection::literal(from, to, *number))
            }
            _ => None,
        }
    }
    #[allow(clippy::many_single_char_names)]
    fn eval_call4(&self, f: &str, l1: &Literal, l2: &Literal, l3: &Literal) -> Option<Self> {
        match (f, self, l1, l2, l3) {
            (
                "IpAddr::from",
                Literal::Int(a),
                Literal::Int(b),
                Literal::Int(c),
                Literal::Int(d),
            ) => Some(Literal::IpAddr(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(*a as u8, *b as u8, *c as u8, *d as u8),
            ))),
            _ => None,
        }
    }
    pub fn literal_vector(args: Vec<Expr>) -> Result<Vec<Literal>, Error> {
        let mut v = Vec::new();
        for a in args {
            match a {
                Expr::LitExpr(l) => v.push(l),
                _ => return Err(Error::new("arg is not a literal")),
            }
        }
        Ok(v)
    }
    fn is_true(&self) -> bool {
        match self {
            Literal::Bool(true) => true,
            _ => false,
        }
    }
}

impl Expr {
    fn is_return(&self) -> bool {
        match self {
            Expr::ReturnExpr(_) => true,
            _ => false,
        }
    }
    fn strip_return(self) -> Expr {
        match self {
            Expr::ReturnExpr(r) => *r,
            _ => self,
        }
    }
    fn perform_match(self, pat: Pattern) -> Option<(Expr, Option<BTreeMap<String, Expr>>)> {
        match pat {
            Pattern::Regex(re) => self.perform_regex_match(re),
            Pattern::Label(label) => self.perform_label_match(label),
        }
    }
    fn perform_label_match(self, label: Label) -> Option<(Expr, Option<BTreeMap<String, Expr>>)> {
        match self {
            Expr::ReturnExpr(_) => Some((self, None)),
            Expr::LitExpr(Literal::Label(ref l)) => {
                if let Some(m) = label.match_with(l) {
                    let v: Vec<(String, String)> = (&m).into();
                    Some((
                        self,
                        Some(v.into_iter().map(|(x, y)| (x, y.into())).collect()),
                    ))
                } else {
                    Some((self, None))
                }
            }
            _ => None,
        }
    }
    fn perform_regex_match(
        self,
        re: PolicyRegex,
    ) -> Option<(Expr, Option<BTreeMap<String, Expr>>)> {
        match self {
            Expr::ReturnExpr(_) => Some((self, None)),
            Expr::LitExpr(Literal::Str(ref s)) => {
                let names: Vec<&str> = re.capture_names().filter_map(|s| s).collect();
                // if there are no bindings then do a simple "is_match", otherwise collect
                // variable captures
                if names.is_empty() {
                    if re.is_match(s) {
                        Some((self, Some(BTreeMap::new())))
                    } else {
                        Some((self, None))
                    }
                } else {
                    match re.captures(s) {
                        // matches
                        Some(cap) => {
                            let mut is_match = true;
                            let mut captures: BTreeMap<String, Expr> = BTreeMap::new();
                            for name in names {
                                let match_str = cap.name(name).unwrap().as_str();
                                let (s, a) = Pat::strip_as(name);
                                captures.insert(
                                    s,
                                    match a {
                                        As::I64 => match match_str.parse::<i64>() {
                                            Ok(i) => Expr::from(i),
                                            _ => {
                                                is_match = false;
                                                break;
                                            }
                                        },
                                        As::Base64 => match base64::decode(match_str) {
                                            Ok(bytes) => Expr::from(bytes.as_slice()),
                                            _ => {
                                                is_match = false;
                                                break;
                                            }
                                        },
                                        _ => Expr::from(match_str),
                                    },
                                );
                            }
                            if is_match {
                                Some((self, Some(captures)))
                            } else {
                                Some((self, None))
                            }
                        }
                        // not a match
                        None => Some((self, None)),
                    }
                }
            }
            _ => None,
        }
    }
    fn eval_call(function: &str, args: Vec<Expr>) -> Result<Expr, self::Error> {
        // builtin function
        match args.as_slice() {
            [] => match Literal::eval_call0(function) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(0): type error")),
            },
            [Expr::LitExpr(l1)] => match l1.eval_call1(&function) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(1): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2)] => match l1.eval_call2(&function, l2) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(2): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3)] => {
                match l1.eval_call3(&function, l2, l3) {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(3): type error")),
                }
            }
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3), Expr::LitExpr(l4)] => {
                match l1.eval_call4(&function, l2, l3, l4) {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(4): type error")),
                }
            }
            x => Err(Error::from(format!("eval, call: {}: {:?}", function, x))),
        }
    }
    #[allow(clippy::cognitive_complexity)]
    fn eval(self, env: Env) -> BoxFuture<'static, Result<Expr, self::Error>> {
        async {
            match self {
                Expr::Var(_) | Expr::BVar(_, _) => Err(Error::new("eval variable")),
                Expr::LitExpr(_) => Ok(self),
                Expr::Closure(_, _) => Err(Error::new("eval, closure")),
                Expr::ReturnExpr(e) => Ok(Expr::return_expr(e.eval(env).await?)),
                Expr::PrefixExpr(p, e) => match e.eval(env).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(l) => match l.eval_prefix(&p) {
                        Some(r) => Ok(r.into()),
                        None => Err(Error::new("eval prefix: type error")),
                    },
                    _ => Err(Error::new("eval, prefix")),
                },
                // short circuit for &&
                Expr::InfixExpr(Infix::And, e1, e2) => match e1.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::Bool(false)) => Ok(r),
                    Expr::LitExpr(Literal::Bool(true)) => match e2.eval(env).await? {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::Bool(_)) => Ok(r),
                        _ => Err(Error::new("eval, infix")),
                    },
                    _ => Err(Error::new("eval, infix")),
                },
                // short circuit for ||
                Expr::InfixExpr(Infix::Or, e1, e2) => match e1.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::Bool(true)) => Ok(r),
                    Expr::LitExpr(Literal::Bool(false)) => match e2.eval(env).await? {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::Bool(_)) => Ok(r),
                        _ => Err(Error::new("eval, infix")),
                    },
                    _ => Err(Error::new("eval, infix")),
                },
                Expr::InfixExpr(op, e1, e2) => {
                    match (e1.eval(env.clone()).await?, e2.eval(env).await?) {
                        (r @ Expr::ReturnExpr(_), _) => Ok(r),
                        (_, r @ Expr::ReturnExpr(_)) => Ok(r),
                        (Expr::LitExpr(l1), Expr::LitExpr(l2)) => match l1.eval_infix(&op, &l2) {
                            Some(r) => Ok(r.into()),
                            None => Err(Error::new("eval, infix: type error")),
                        },
                        _ => Err(Error::new("eval, infix: failed")),
                    }
                }
                Expr::BlockExpr(b, mut es) => {
                    if es.is_empty() {
                        Ok(Expr::LitExpr(if b == Block::List {
                            Literal::List(Vec::new())
                        } else {
                            Literal::Unit
                        }))
                    } else if b == Block::Block {
                        let e = es.remove(0);
                        let res = e.eval(env.clone()).await?;
                        if res.is_return() || es.is_empty() {
                            Ok(res)
                        } else {
                            Expr::BlockExpr(b, es).eval(env).await
                        }
                    } else {
                        // list or tuple
                        let mut rs = Vec::new();
                        for e in es.into_iter() {
                            rs.push(e.eval(env.clone()).await?)
                        }
                        match rs.iter().find(|r| r.is_return()) {
                            Some(r) => Ok(r.clone()),
                            _ => match Literal::literal_vector(rs) {
                                Ok(lits) => Ok((if b == Block::List {
                                    Literal::List(lits)
                                } else {
                                    Literal::Tuple(lits)
                                })
                                .into()),
                                Err(err) => Err(err),
                            },
                        }
                    }
                }
                Expr::Let(vs, e1, e2) => match e1.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::Tuple(lits)) => {
                        let lits_len = lits.len();
                        if 1 < lits_len && vs.len() == lits_len {
                            let mut e2a = *e2.clone();
                            for (v, lit) in vs.iter().zip(lits) {
                                if v != "_" {
                                    e2a = e2a.apply(&Expr::LitExpr(lit))?
                                }
                            }
                            e2a.eval(env).await
                        } else if vs.len() == 1 {
                            e2.apply(&Expr::LitExpr(Literal::Tuple(lits)))?
                                .eval(env)
                                .await
                        } else {
                            Err(Error::new("eval, let-expression (tuple length mismatch)"))
                        }
                    }
                    l @ Expr::LitExpr(_) => {
                        if vs.len() == 1 {
                            e2.apply(&l)?.eval(env).await
                        } else {
                            Err(Error::new("eval, let-expression (literal not a tuple)"))
                        }
                    }
                    _ => Err(Error::new("eval, let-expression")),
                },
                Expr::Iter(op, vs, e1, e2) => match e1.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::List(lits)) => {
                        let mut res = Vec::new();
                        for l in lits.iter() {
                            match l {
                                Literal::Tuple(ref ts) if vs.len() != 1 => {
                                    if vs.len() == ts.len() {
                                        let mut e = *e2.clone();
                                        for (v, lit) in vs.iter().zip(ts) {
                                            if v != "_" {
                                                e = e.apply(&Expr::LitExpr(lit.clone()))?
                                            }
                                        }
                                        res.push(e.eval(env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "eval, iter-expression (tuple length mismatch)",
                                        ));
                                    }
                                }
                                _ => {
                                    if vs.len() == 1 {
                                        let mut e = *e2.clone();
                                        if vs[0] != "_" {
                                            e = e.clone().apply(&Expr::LitExpr(l.clone()))?
                                        }
                                        res.push(e.eval(env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "eval, iter-expression (not a tuple list)",
                                        ));
                                    }
                                }
                            }
                        }
                        match res.iter().find(|r| r.is_return()) {
                            Some(r) => Ok(r.clone()),
                            None => match Literal::literal_vector(res) {
                                Ok(iter_lits) => match op {
                                    Iter::Map => Ok(Literal::List(iter_lits).into()),
                                    Iter::ForEach => Ok(Expr::from(())),
                                    Iter::Filter => {
                                        let filtered_lits = lits
                                            .into_iter()
                                            .zip(iter_lits.into_iter())
                                            .filter_map(
                                                |(l, b)| if b.is_true() { Some(l) } else { None },
                                            )
                                            .collect();
                                        Ok(Literal::List(filtered_lits).into())
                                    }
                                    Iter::FilterMap => {
                                        let filtered_lits = iter_lits
                                            .iter()
                                            .filter_map(Literal::dest_some)
                                            .collect();
                                        Ok(Literal::List(filtered_lits).into())
                                    }
                                    Iter::All => Ok(iter_lits.iter().all(|l| l.is_true()).into()),
                                    Iter::Any => Ok(iter_lits.iter().any(|l| l.is_true()).into()),
                                },
                                Err(err) => Err(err),
                            },
                        }
                    }
                    _ => Err(Error::new("eval, map-expression")),
                },
                Expr::IfExpr {
                    cond,
                    consequence,
                    alternative,
                } => match cond.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::Bool(true)) => consequence.eval(env).await,
                    Expr::LitExpr(Literal::Bool(false)) => match alternative {
                        Some(alt) => alt.eval(env).await,
                        None => Ok(Expr::from(())),
                    },
                    _ => Err(Error::new("eval, if-expression")),
                },
                Expr::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => match expr.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::Tuple(t)) => {
                        if t.len() == 1 {
                            match consequence.apply(&Expr::LitExpr(t[0].clone())) {
                                Ok(consequence_apply) => consequence_apply.eval(env).await,
                                Err(e) => Err(e),
                            }
                        } else {
                            match alternative {
                                Some(alt) => alt.eval(env).await,
                                None => Ok(Expr::from(())),
                            }
                        }
                    }
                    r => Err(Error::from(format!("eval, if-let-expression: {:#?}", r))),
                },
                Expr::IfMatchExpr {
                    variables,
                    matches,
                    consequence,
                    alternative,
                } => {
                    let mut rs = Vec::new();
                    for (e, re) in matches.into_iter() {
                        if let Some(r) = e.eval(env.clone()).await?.perform_match(re) {
                            rs.push(r)
                        } else {
                            return Err(Error::new("eval, if-match-expression: type error"));
                        }
                    }
                    match rs.iter().find(|(r, _captures)| r.is_return()) {
                        // early exit
                        Some((r, _captures)) => Ok(r.clone()),
                        None => {
                            if rs.iter().any(|(_r, captures)| captures.is_none()) {
                                // failed match
                                match alternative {
                                    None => Ok(Expr::from(())),
                                    Some(alt) => match alt.eval(env).await? {
                                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                                        _ => Err(Error::new("eval, if-match-expression")),
                                    },
                                }
                            } else {
                                // match
                                let mut all_captures: BTreeMap<String, Expr> = BTreeMap::new();
                                for (_r, captures) in rs {
                                    if let Some(caps) = captures {
                                        all_captures.extend(caps)
                                    }
                                }
                                let mut c = *consequence;
                                for v in variables {
                                    if let Some(e) = all_captures.get(&v) {
                                        c = c.apply(e)?
                                    } else {
                                        return Err(Error::new(
                                            "eval, if-match-expression: missing bind",
                                        ));
                                    }
                                }
                                match c.eval(env).await? {
                                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                                    _ => Err(Error::new("eval, if-match-expression")),
                                }
                            }
                        }
                    }
                }
                Expr::CallExpr {
                    function,
                    arguments,
                    is_async,
                } => {
                    let mut args = Vec::new();
                    for e in arguments.into_iter() {
                        args.push(e.eval(env.clone()).await?)
                    }
                    match args.iter().find(|r| r.is_return()) {
                        Some(r) => Ok(r.clone()),
                        None => {
                            if let Some(mut r) = env.get(&function) {
                                // user defined function
                                for a in args {
                                    r = r.apply(&a)?
                                }
                                r.evaluate(env).await
                            } else if Headers::is_builtin(&function) {
                                Expr::eval_call(function.as_str(), args)
                            } else if let Some((external, method)) = Headers::split(&function) {
                                // external function (RPC) or "Ingress/Egress" metadata
                                let args = Literal::literal_vector(args)?;
                                let call = Call::new(external, method, args);
                                if external == "Ingress" || external == "Egress" {
                                    env.meta
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("Metadata call error"))?
                                } else if is_async {
                                    Arbiter::spawn(env.external.send(call).then(|res| {
                                        match res {
                                            Ok(Err(e)) => log::warn!("{}", e),
                                            Err(e) => log::warn!("{}", e),
                                            _ => (),
                                        };
                                        async {}
                                    }));
                                    Ok(Expr::from(()))
                                } else {
                                    env.external
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("capnp error"))?
                                }
                            } else {
                                Err(Error::from(format!("eval, call: {}: {:?}", function, args)))
                            }
                        }
                    }
                }
            }
        }
        .boxed()
    }
    pub async fn evaluate(self, env: Env) -> Result<Expr, self::Error> {
        Ok(self.eval(env).await?.strip_return())
    }
}
