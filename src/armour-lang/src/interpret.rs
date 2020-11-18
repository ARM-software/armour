/// policy language interpreter
// NOTE: no optimization
use super::expressions::{Block, Error, Expr, Pattern, CPPrefix, DPPrefix};
use super::externals::{Call, ExternalActor};
use super::headers::{Headers, THeaders};
use super::labels::Label;
use super::lang::{Code, Program};
use super::literals::{
    self, Connection, CPLiteral, DPLiteral, HttpRequest, HttpResponse, Literal,
    DPFlatLiteral, CPFlatLiteral, Method,
    OnboardingResult, TFlatLiteral, VecSet
};
use super::meta::{Egress, IngressEgress, Meta};
use super::parser::{As, Infix, Iter, Pat, PolicyRegex, Prefix};
use super::types::{self, CPFlatTyp, TFlatTyp};
use actix::prelude::*;
use futures::future::{BoxFuture, FutureExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use async_trait::async_trait;
use std::time::{SystemTime, UNIX_EPOCH};
use std::str::FromStr;

#[derive(Clone)]
pub struct Env<FlatTyp, FlatLiteral>
where 
    FlatTyp: 'static + std::marker::Send + TFlatTyp,
    FlatLiteral: 'static + std::marker::Send + TFlatLiteral<FlatTyp>
{
    pub internal: Arc<Code<FlatTyp, FlatLiteral>>,
    pub external: Addr<ExternalActor>,
    pub meta: Addr<IngressEgress>,
}

pub type DPEnv = Env<types::FlatTyp, literals::DPFlatLiteral>;
pub type CPEnv = Env<CPFlatTyp, literals::CPFlatLiteral>;

impl<FlatTyp, FlatLiteral> Env<FlatTyp, FlatLiteral>
where
    FlatTyp: std::marker::Send + TFlatTyp,
    FlatLiteral: std::marker::Send + TFlatLiteral<FlatTyp>
{
    pub fn new(prog: &Program<FlatTyp, FlatLiteral>) -> Self {
        Env {
            internal: Arc::new(prog.code.clone()),
            external: ExternalActor::new(prog).start(), //TODO
            meta: IngressEgress::start_default(),
        }
    }
    pub fn get(&self, name: &str) -> Option<Expr<FlatTyp, FlatLiteral>> {
        self.internal.0.get(name).cloned()
    }
    pub fn set_meta(&mut self, meta: IngressEgress) {
        self.meta = meta.start()
    }
    pub async fn egress(&self) -> Option<Meta> {
        self.meta.send(Egress).await.ok()?.ok()
    }
}

#[async_trait]
pub trait TInterpret<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp, 
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn eval_prefix(&self, p: &Prefix<FlatTyp>) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_infix(&self, op: &Infix<FlatTyp>, other: &Self) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_call0(f: &str) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_call1(&self, f: &str) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_call2(&self, f: &str, other: &Self) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_call3(&self, f: &str, l1: &Self, l2: &Self) -> Option<Literal<FlatTyp, FlatLiteral>>;
    fn eval_call4(&self, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<Literal<FlatTyp, FlatLiteral>>;
    async fn helper_evalexpr(e : Expr<FlatTyp, FlatLiteral>, env: Env<FlatTyp, FlatLiteral>) -> Result<Expr<FlatTyp, FlatLiteral>, self::Error>;
}

macro_rules! dpflatlit (
  ($i: ident ($($args:tt)*) ) => (
        DPFlatLiteral::$i($($args)*)
  );
);
macro_rules! cpflatlit (
  ($i: ident ($($args:tt)*) ) => (
        CPFlatLiteral::$i($($args)*)
  );
);
macro_rules! cpdpflatlit (
  ($i: ident ($($args:tt)*) ) => (
        CPFlatLiteral::DPFlatLiteral(DPFlatLiteral::$i($($args)*))
  );
);
macro_rules! dplit (
  ($i: ident ($($args:tt)*) ) => (
      Literal::FlatLiteral(DPFlatLiteral::$i($($args)*))
  );
);
macro_rules! cplit (
  ($i: ident ($($args:tt)*) ) => (
      Literal::FlatLiteral(CPFlatLiteral::$i($($args)*))
  );
);

macro_rules! cpdplit (
  ($i: ident ($($args:tt)*) ) => (
      Literal::FlatLiteral(CPFlatLiteral::DPFlatLiteral(DPFlatLiteral::$i($($args)*)))
  );
);

#[async_trait]
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>+TInterpret<FlatTyp, FlatLiteral>> TInterpret<FlatTyp, FlatLiteral> for Literal<FlatTyp, FlatLiteral> {
    fn eval_prefix(&self, p: &Prefix<FlatTyp>) -> Option<Literal<FlatTyp, FlatLiteral>> {
        match self {
            Literal::FlatLiteral(fl) => fl.eval_prefix(p),
            _ => None,
        }
    }
    fn eval_infix(&self, op: &Infix<FlatTyp>, other: &Self) -> Option<Literal<FlatTyp, FlatLiteral>> {
        match (self, other) {
            (Literal::FlatLiteral(fl), Literal::FlatLiteral(other))  => fl.eval_infix(op, other),
            _ => match (op, self, other) {
                (Infix::Concat, Literal::List(i), Literal::List(j)) => {
                    let mut k = i.clone();
                    k.append(&mut j.clone());
                    Some(Literal::List(k))
                },
                (Infix::In, _, Literal::List(l)) => Some(VecSet::contains(l, self)),
                _ => None,
            }
        }
    }

    fn eval_call0(f: &str) -> Option<Self> {
        FlatLiteral::eval_call0(f) 
    }

    fn eval_call1(&self, f: &str) -> Option<Self> {
        match self {
            Literal::FlatLiteral(fl) => fl.eval_call1(f),
            _ => match (f, self) {
                ("option::is_none", Literal::Tuple(t)) => Some(Literal::bool(t.is_empty())),
                ("option::is_some", Literal::Tuple(t)) => Some(Literal::bool(t.len() == 1)),
                ("list::len", Literal::List(l)) => Some(Literal::int(l.len() as i64)),
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
                (_, Literal::Tuple(l)) => {
                    if let Ok(i) = f.parse::<usize>() {
                        l.get(i).cloned()
                    } else {
                        None
                    }
                }
                _ => None
            }
        }
    }
    fn eval_call2(&self, f: &str, other: &Self) -> Option<Self> {
        match (self, other) {
            (Literal::FlatLiteral(fl), Literal::FlatLiteral(other)) => fl.eval_call2(f, other),
            _ => match (f, self, other) {
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
                _ =>  None
            }
        }
    }
    fn eval_call3(&self, f: &str, l1: &Self, l2: &Self) -> Option<Self> {
        match (self, l1, l2) {
            (Literal::FlatLiteral(fl), Literal::FlatLiteral(l1), Literal::FlatLiteral(l2)) => fl.eval_call3(f, l1, l2),
            _ => None
        }
    }
    #[allow(clippy::many_single_char_names)]
    fn eval_call4(&self, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<Self> {
        match (self, l1, l2, l3) {
            (Literal::FlatLiteral(fl), Literal::FlatLiteral(l1), Literal::FlatLiteral(l2), Literal::FlatLiteral(l3)) => fl.eval_call4(f, l1, l2, l3),
            _ => None
        }
    }

    async fn helper_evalexpr(e : Expr<FlatTyp, FlatLiteral>, env: Env<FlatTyp, FlatLiteral>) -> Result<Expr<FlatTyp, FlatLiteral>, self::Error>{
       FlatLiteral::helper_evalexpr(e, env).await 
    }
}

//TODO factorize using same structure as in types.rs
#[async_trait]
impl TInterpret<types::FlatTyp, DPFlatLiteral> for DPFlatLiteral {
    fn eval_prefix(&self, p: &Prefix<types::FlatTyp>) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (p, self) {
            (Prefix::Not, dpflatlit!(Bool(b))) => Some(dplit!(Bool(b.clone()))),
            (Prefix::Minus, dpflatlit!(Int(i))) => Some(dplit!(Int(-i))),
            _ => None,
        }
    }
    fn eval_infix(&self, op: &Infix<types::FlatTyp>, other: &Self) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (op, self, other) {
            (Infix::Equal, _, _) => Some(dplit!(Bool(self == other))),
            (Infix::NotEqual, _, _) => Some(dplit!(Bool(self != other))),
            (Infix::Plus, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i + j))),
            (Infix::Minus, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i - j))),
            (Infix::Divide, dpflatlit!(Int(_)), dpflatlit!(Int(0))) => None,
            (Infix::Divide, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i / j))),
            (Infix::Multiply, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i * j))),
            (Infix::Remainder, dpflatlit!(Int(_)), dpflatlit!(Int(0))) => None,
            (Infix::Remainder, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i % j))),
            (Infix::LessThan, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Bool(i < j))),
            (Infix::LessThanEqual, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Bool(i <= j))),
            (Infix::GreaterThan, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Bool(i > j))),
            (Infix::GreaterThanEqual, dpflatlit!(Int(i)), dpflatlit!(Int(j))) => {
                Some(dplit!(Bool(i >= j)))
            }
            (Infix::And, dpflatlit!(Bool(i)), dpflatlit!(Bool(j))) => Some(dplit!(Bool(*i && *j))),
            (Infix::Or, dpflatlit!(Bool(i)), dpflatlit!(Bool(j))) => Some(dplit!(Bool(*i || *j))),
            (Infix::ConcatStr, dpflatlit!(Str(i)), dpflatlit!(Str(j))) => {
                Some(dplit!(Str(format!("{}{}", i, j))))
            }
            _ => None,
        }
    }
    fn eval_call0(f: &str) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
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
            "ID::default" => Some(Literal::id(Default::default())),
            "Connection::default" => Some(Literal::connection(Default::default())),
            "IpAddr::localhost" => Some(Literal::ip_addr(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(127, 0, 0, 1),
            ))),
            "System::getCurrentTime" => {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards");
                Some(dplit!(Int(since_the_epoch.as_secs() as i64)))
            }
            _ => None,
        }
    }
    fn eval_call1(&self, f: &str) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (f, self) {
            ("option::Some", _) => Some(Literal::some2(self)),
            ("i64::abs", dpflatlit!(Int(i))) => Some(dplit!(Int(i.abs()))),
            ("i64::to_str", dpflatlit!(Int(i))) => Some(dplit!(Str(i.to_string()))),
            ("str::len", dpflatlit!(Str(s))) => Some(dplit!(Int(s.len() as i64))),
            ("str::to_lowercase", dpflatlit!(Str(s))) => Some(dplit!(Str(s.to_lowercase()))),
            ("str::to_uppercase", dpflatlit!(Str(s))) => Some(dplit!(Str(s.to_uppercase()))),
            ("str::trim_start", dpflatlit!(Str(s))) => Some(dplit!(Str(s.trim_start().to_string()))),
            ("str::trim_end", dpflatlit!(Str(s))) => Some(dplit!(Str(s.trim_end().to_string()))),
            ("str::as_bytes", dpflatlit!(Str(s))) => Some(dplit!(Data(s.as_bytes().to_vec()))),
            ("str::from_utf8", dpflatlit!(Data(s))) => Some(dplit!(Str(
                std::string::String::from_utf8_lossy(s).to_string(),
            ))),
            ("str::to_base64", dpflatlit!(Str(s))) => Some(dplit!(Str(base64::encode(s)))),
            ("data::to_base64", dpflatlit!(Data(d))) => Some(dplit!(Str(base64::encode(d)))),
            ("data::len", dpflatlit!(Data(d))) => Some(dplit!(Int(d.len() as i64))),
            ("HttpRequest::connection", dpflatlit!(HttpRequest(req))) => Some(req.connection()),
            ("HttpRequest::from", dpflatlit!(HttpRequest(req))) => Some(req.from_lit()),
            ("HttpRequest::to", dpflatlit!(HttpRequest(req))) => Some(req.to_lit()),
            ("HttpRequest::from_to", dpflatlit!(HttpRequest(req))) => Some(req.from_to()),
            ("HttpRequest::method", dpflatlit!(HttpRequest(req))) => Some(req.method()),
            ("HttpRequest::version", dpflatlit!(HttpRequest(req))) => Some(req.version()),
            ("HttpRequest::path", dpflatlit!(HttpRequest(req))) => Some(req.path()),
            ("HttpRequest::route", dpflatlit!(HttpRequest(req))) => Some(req.route()),
            ("HttpRequest::query", dpflatlit!(HttpRequest(req))) => Some(req.query()),
            ("HttpRequest::query_pairs", dpflatlit!(HttpRequest(req))) => Some(req.query_pairs()),
            ("HttpRequest::header_pairs", dpflatlit!(HttpRequest(req))) => Some(req.header_pairs()),
            ("HttpRequest::headers", dpflatlit!(HttpRequest(req))) => Some(req.headers()),
            ("HttpResponse::new", dpflatlit!(Int(code))) => Some(HttpResponse::literal(*code as u16)),
            ("HttpResponse::connection", dpflatlit!(HttpResponse(res))) => Some(res.connection()),
            ("HttpResponse::from", dpflatlit!(HttpResponse(res))) => Some(res.from_lit()),
            ("HttpResponse::to", dpflatlit!(HttpResponse(res))) => Some(res.to_lit()),
            ("HttpResponse::from_to", dpflatlit!(HttpResponse(res))) => Some(res.from_to()),
            ("HttpResponse::status", dpflatlit!(HttpResponse(res))) => Some(res.status()),
            ("HttpResponse::version", dpflatlit!(HttpResponse(res))) => Some(res.version()),
            ("HttpResponse::reason", dpflatlit!(HttpResponse(res))) => Some(res.reason()),
            ("HttpResponse::header_pairs", dpflatlit!(HttpResponse(req))) => Some(req.header_pairs()),
            ("HttpResponse::headers", dpflatlit!(HttpResponse(req))) => Some(req.headers()),
            ("IpAddr::octets", dpflatlit!(IpAddr(ip))) => Some(Literal::from(ip)),
            ("ID::labels", dpflatlit!(ID(id))) => Some(id.labels()),
            ("ID::hosts", dpflatlit!(ID(id))) => Some(id.hosts()),
            ("ID::ips", dpflatlit!(ID(id))) => Some(id.ips()),
            ("ID::port", dpflatlit!(ID(id))) => Some(id.port_lit()),
            ("Connection::from_to", dpflatlit!(Connection(c))) => Some(c.from_to()),
            ("Connection::from", dpflatlit!(Connection(c))) => Some(c.from_lit()),
            ("Connection::to", dpflatlit!(Connection(c))) => Some(c.to_lit()),
            ("Connection::number", dpflatlit!(Connection(c))) => Some(c.number()),
            ("Label::parts", dpflatlit!(Label(l))) => Some(l.parts().into()),
            ("IpAddr::reverse_lookup", dpflatlit!(IpAddr(ip))) => {
                Some(if let Ok(res) = dns_lookup::lookup_addr(ip) {
                    dplit!(Str(res)).some()
                } else {
                    Literal::none()
                })
            }
            ("IpAddr::lookup", dpflatlit!(Str(name))) => {
                Some(if let Ok(res) = dns_lookup::lookup_host(name) {
                    Literal::List(
                        res.iter()
                            .filter_map(|ip| {
                                if ip.is_ipv4() {
                                    Some(dplit!(IpAddr(*ip)))
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
    fn eval_call2(&self, f: &str, other: &Self) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (f, self, other) {
            ("i64::pow", dpflatlit!(Int(i)), dpflatlit!(Int(j))) => Some(dplit!(Int(i.pow(*j as u32)))),
            ("i64::min", dpflatlit!(Int(i)), dpflatlit!(Int(j))) => {
                Some(dplit!(Int(std::cmp::min(*i, *j))))
            }
            ("i64::max", dpflatlit!(Int(i)), dpflatlit!(Int(j))) => {
                Some(dplit!(Int(std::cmp::max(*i, *j))))
            }
            ("str::starts_with", dpflatlit!(Str(i)), dpflatlit!(Str(j))) => {
                Some(dplit!(Bool(i.starts_with(j))))
            }
            ("str::ends_with", dpflatlit!(Str(i)), dpflatlit!(Str(j))) => {
                Some(dplit!(Bool(i.ends_with(j))))
            }
            ("str::is_match", dpflatlit!(Str(s)), dpflatlit!(Regex(r)))
            | ("regex::is_match", dpflatlit!(Regex(r)), dpflatlit!(Str(s))) => {
                Some(dplit!(Bool(r.is_match(s))))
            }
            ("str::contains", dpflatlit!(Str(i)), dpflatlit!(Str(j))) => {
                Some(dplit!(Bool(i.contains(j))))
            }
            ("HttpRequest::set_path", dpflatlit!(HttpRequest(req)), dpflatlit!(Str(q))) => {
                Some(req.set_path(q).into())
            }
            ("HttpRequest::set_query", dpflatlit!(HttpRequest(req)), dpflatlit!(Str(q))) => {
                Some(req.set_query(q).into())
            }
            ("HttpRequest::header", dpflatlit!(HttpRequest(req)), dpflatlit!(Str(h))) => {
                Some(req.header(&h))
            }
            ("HttpRequest::unique_header", dpflatlit!(HttpRequest(req)), dpflatlit!(Str(h))) => {
                Some(req.unique_header(&h))
            }
            ("HttpRequest::set_connection", dpflatlit!(HttpRequest(req)), dpflatlit!(Connection(c))) => {
                Some(req.set_connection(c).into())
            }
            ("HttpRequest::set_from", dpflatlit!(HttpRequest(req)), dpflatlit!(ID(f))) => {
                Some(req.set_from(f).into())
            }
            ("HttpRequest::set_to", dpflatlit!(HttpRequest(req)), dpflatlit!(ID(f))) => {
                Some(req.set_to(f).into())
            }
            ("HttpResponse::header", dpflatlit!(HttpResponse(res)), dpflatlit!(Str(h))) => {
                Some(res.header(&h))
            }
            ("HttpResponse::unique_header", dpflatlit!(HttpResponse(res)), dpflatlit!(Str(h))) => {
                Some(res.unique_header(&h))
            }
            ("HttpResponse::set_reason", dpflatlit!(HttpResponse(res)), dpflatlit!(Str(q))) => {
                Some(res.set_reason(q).into())
            }
            (
                "HttpResponse::set_connection",
                dpflatlit!(HttpResponse(res)),
                dpflatlit!(Connection(c)),
            ) => Some(res.set_connection(c).into()),
            ("HttpResponse::set_from", dpflatlit!(HttpResponse(res)), dpflatlit!(ID(f))) => {
                Some(res.set_from(f).into())
            }
            ("HttpResponse::set_to", dpflatlit!(HttpResponse(res)), dpflatlit!(ID(f))) => {
                Some(res.set_to(f).into())
            }
            ("ID::has_label", dpflatlit!(ID(id)), dpflatlit!(Label(l))) => 
                Some(id.has_label(l).into()),
            ("ID::add_label", dpflatlit!(ID(id)), dpflatlit!(Label(l))) => {
                Some(dplit!(ID(id.add_label(l))))
            }
            ("ID::has_host", dpflatlit!(ID(id)), dpflatlit!(Str(h))) => 
                Some(id.has_host(h).into()),
            ("ID::add_host", dpflatlit!(ID(id)), dpflatlit!(Str(h))) => 
                Some(dplit!(ID(id.add_host(h)))),
            ("ID::has_ip", dpflatlit!(ID(id)), dpflatlit!(IpAddr(i))) => 
                Some(id.has_ip(i).into()),
            ("ID::add_ip", dpflatlit!(ID(id)), dpflatlit!(IpAddr(i))) => 
                Some(dplit!(ID(id.add_ip(*i)))),
 
            ("ID::set_port", dpflatlit!(ID(id)), dpflatlit!(Int(q))) => {
                Some(dplit!(ID(id.set_port(*q as u16))))
            }
            ("Connection::set_from", dpflatlit!(Connection(c)), dpflatlit!(ID(f))) => {
                Some(c.set_from(f).into())
            }
            ("Connection::set_to", dpflatlit!(Connection(c)), dpflatlit!(ID(f))) => {
                Some(c.set_to(f).into())
            }
            ("Connection::set_number", dpflatlit!(Connection(c)), dpflatlit!(Int(n))) => {
                Some(c.set_number(*n).into())
            }
            ("Label::captures", dpflatlit!(Label(i)), dpflatlit!(Label(j))) => {
                Some(i.match_with(j).into())
            }
            ("Label::is_match", dpflatlit!(Label(i)), dpflatlit!(Label(j))) => {
                Some(i.matches_with(j).into())
            }
            _ => None,
        }
    }
    fn eval_call3(&self, f: &str, l1: &Self, l2: &Self) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (f, self, l1, l2) {
            (
                "HttpRequest::set_header",
                dpflatlit!(HttpRequest(req)),
                dpflatlit!(Str(h)),
                dpflatlit!(Data(v)),
            ) => Some(req.set_header(h, v).into()),
            (
                "HttpResponse::set_header",
                dpflatlit!(HttpResponse(res)),
                dpflatlit!(Str(h)),
                dpflatlit!(Data(v)),
            ) => Some(res.set_header(h, v).into()),
            ("Connection::new", dpflatlit!(ID(from)), dpflatlit!(ID(to)), dpflatlit!(Int(number))) => {
                Some(Connection::literal(from, to, *number))
            }
            _ => None,
        }
    }
    #[allow(clippy::many_single_char_names)]
    fn eval_call4(&self, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<Literal<types::FlatTyp, DPFlatLiteral>> {
        match (f, self, l1, l2, l3) {
            (
                "IpAddr::from",
                dpflatlit!(Int(a)),
                dpflatlit!(Int(b)),
                dpflatlit!(Int(c)),
                dpflatlit!(Int(d)),
            ) => Some(dplit!(IpAddr(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(*a as u8, *b as u8, *c as u8, *d as u8),
            )))),
            _ => None,
        }
    }

    async fn helper_evalexpr(e : Expr<types::FlatTyp, DPFlatLiteral>, env: Env<types::FlatTyp, DPFlatLiteral>) -> Result<Expr<types::FlatTyp, DPFlatLiteral>, self::Error>{
        match e {
            // short circuit for &&
            Expr::InfixExpr(Infix::And, e1, e2) => match e1.eval(env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(dplit!(Bool(false))) => Ok(r),
                Expr::LitExpr(dplit!(Bool(true))) => match e2.eval(env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(dplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            // short circuit for ||
            Expr::InfixExpr(Infix::Or, e1, e2) => match e1.eval(env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(dplit!(Bool(true))) => Ok(r),
                Expr::LitExpr(dplit!(Bool(false))) => match e2.eval(env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(dplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            _ => unimplemented!()
        }
    }
}

#[async_trait]
impl TInterpret<CPFlatTyp, CPFlatLiteral> for CPFlatLiteral {
    fn eval_prefix(&self, p: &Prefix<CPFlatTyp>) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (p, self) {
            (_, CPFlatLiteral::DPFlatLiteral(dpfl)) => 
                dpfl.eval_prefix( &p.clone().into()).map(CPLiteral::from),
            _ => None,
        }
    }

    fn eval_infix(&self, op: &Infix<CPFlatTyp>, other: &Self) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (op, self, other) {
            (Infix::Equal, _, _) => Some(Literal::bool(self == other)),
            (Infix::NotEqual, _, _) => Some(Literal::bool(self != other)),
            (   
                _, 
                CPFlatLiteral::DPFlatLiteral(dpfl1), 
                CPFlatLiteral::DPFlatLiteral(dpfl2) 
            ) => dpfl1.eval_infix(&op.clone().into(), dpfl2).map(CPLiteral::from),
            _ => None,
        }
    }

    fn eval_call0(f: &str) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match f {
            _ => DPFlatLiteral::eval_call0(f).map(CPLiteral::from),
        }
    }

    fn eval_call1(&self, f: &str) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (f, self) {
            ("Label::new", cpdpflatlit!(Str(s))) => {
                match Label::from_str(s) {
                    Ok(l)=> Some(Literal::label(l)),
                    _ => None 
                }
            },
            ("Label::login_time", cpdpflatlit!(Int(i))) =>
                Some(Literal::label(Label::login_time(*i))),
            ("OnboardingData::host", cpflatlit!(OnboardingData(obd))) => 
                Some(obd.host_lit()),
            ("OnboardingData::proposed_labels", cpflatlit!(OnboardingData(obd))) => 
                Some(obd.proposed_labels()),
            ("OnboardingData::service", cpflatlit!(OnboardingData(obd))) => 
                Some(obd.service_lit()),
            ("OnboardingResult::ErrStr",  cpdpflatlit!(Str(err))) => {
                Some(OnboardingResult::new_err_str_lit(err.clone()))
            },
            (_, CPFlatLiteral::DPFlatLiteral(dpfl)) => dpfl.eval_call1(f).map(CPLiteral::from),
            _ => None,
        }
    }
    fn eval_call2(&self, f: &str, other: &Self) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (f, self, other) {
            (
                "OnboardingData::has_proposed_label",
                cpflatlit!(OnboardingData(obd)), 
                cpdpflatlit!(Label(l))
            ) => 
                Some(obd.has_proposed_label(l).into()),
            (
                "OnboardingData::has_ip", 
                cpflatlit!(OnboardingData(obd)),
                cpdpflatlit!(IpAddr(i))
            ) => 
                Some(obd.has_ip(i).into()),
            (
                _, 
                CPFlatLiteral::DPFlatLiteral(dpfl1), 
                CPFlatLiteral::DPFlatLiteral(dpfl2)
            ) =>
                dpfl1.eval_call2(f, dpfl2).map(CPLiteral::from),
            _ => None,
        }
    }
    fn eval_call3(&self, f: &str, l1: &Self, l2: &Self) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (f, self, l1, l2) {
            (  
                "OnboardingResult::Ok", 
                cpdpflatlit!(ID(id)), 
                cpflatlit!(Policy(p1)),
                cpflatlit!(Policy(p2))
            ) =>{
                Some(OnboardingResult::new_ok_lit(id.clone().into(), (*p1.clone(), *p2.clone())))
            },
            (
                _, 
                CPFlatLiteral::DPFlatLiteral(dpfl1), 
                CPFlatLiteral::DPFlatLiteral(dpfl2), 
                CPFlatLiteral::DPFlatLiteral(dpfl3) 
            ) =>
                dpfl1.eval_call3(f, dpfl2, dpfl3).map(CPLiteral::from),
            _ => None,
        }
    }
    #[allow(clippy::many_single_char_names)]
    fn eval_call4(&self, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<Literal<CPFlatTyp, CPFlatLiteral>> {
        match (f, self, l1, l2, l3) {
            (   
                "OnboardingResult::Err", 
                cpdpflatlit!(Str(err)),
                cpdpflatlit!(ID(id)), 
                cpflatlit!(Policy(p1)), 
                cpflatlit!(Policy(p2))
            ) => {
                Some(OnboardingResult::new_err_lit(
                    err.clone(), 
                    id.clone().into(), 
                    (*p1.clone(), *p2.clone())
                ))
            },  
            (
                _, 
                CPFlatLiteral::DPFlatLiteral(dpfl1), 
                CPFlatLiteral::DPFlatLiteral(dpfl2), 
                CPFlatLiteral::DPFlatLiteral(dpfl3), 
                CPFlatLiteral::DPFlatLiteral(dpfl4) 
            ) =>
                dpfl1.eval_call4(f, dpfl2, dpfl3, dpfl4).map(CPLiteral::from),
            _ => None,
        }
    }
    async fn helper_evalexpr(e : Expr<CPFlatTyp, CPFlatLiteral>, env: Env<CPFlatTyp, CPFlatLiteral>) -> Result<Expr<CPFlatTyp, CPFlatLiteral>, self::Error>{
        match e {
            // short circuit for &&
            Expr::InfixExpr(Infix::And, e1, e2) => match e1.eval(env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(false))) => Ok(r),
                Expr::LitExpr(cpdplit!(Bool(true))) => match e2.eval(env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            // short circuit for ||
            Expr::InfixExpr(Infix::Or, e1, e2) => match e1.eval(env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(true))) => Ok(r),
                Expr::LitExpr(cpdplit!(Bool(false))) => match e2.eval(env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            _ => unimplemented!()
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Expr<FlatTyp, FlatLiteral> 
where FlatTyp: std::marker::Send, FlatLiteral: std::marker::Send + TInterpret<FlatTyp, FlatLiteral>{ //it means the type does not contain any non-static references
    pub fn is_return(&self) -> bool {
        match self {
            Expr::ReturnExpr(_) => true,
            _ => false,
        }
    }
    pub fn literal_vector(args: Vec<Self>) -> Result<Vec<Literal<FlatTyp, FlatLiteral>>, Error> {
        let mut v = Vec::new();
        for a in args {
            match a {
                Expr::LitExpr(l) => v.push(l),
                _ => return Err(Error::new("arg is not a literal")),
            }
        }
        Ok(v)
    }
    pub fn strip_return(self) -> Self {
        match self {
            Expr::ReturnExpr(r) => *r,
            _ => self,
        }
    }
    pub fn perform_match(self, pat: Pattern) -> Option<(Self, Option<BTreeMap<String, Self>>)> {
        match pat {
            Pattern::Regex(re) => self.perform_regex_match(re),
            Pattern::Label(label) => self.perform_label_match(label),
        }
    }
    fn perform_label_match(self, label: Label) -> Option<(Self, Option<BTreeMap<String, Self>>)> {
        match self {
            Expr::ReturnExpr(_) => Some((self, None)),
            Expr::LitExpr(Literal::FlatLiteral(ref fl)) if fl.is_label() => {
                let ref l = fl.get_label();
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
    ) -> Option<(Self, Option<BTreeMap<String, Self>>)> {
        match self {
            Expr::ReturnExpr(_) => Some((self, None)),
            Expr::LitExpr(Literal::FlatLiteral(ref fl)) if fl.is_str() => {
                let s = fl.get_str();
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
                            let mut captures: BTreeMap<String, Self> = BTreeMap::new();
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
    fn eval_call(function: &str, args: Vec<Self>) -> Result<Self, self::Error> {
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
            x => Err(Error::from(format!("eval, call ({}): {}: {:?}", x.len(), function, x))),
        }
    }
    #[allow(clippy::cognitive_complexity)]
    fn eval(self, env: Env<FlatTyp, FlatLiteral>) -> BoxFuture<'static, Result<Self, self::Error>> {
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
                Expr::InfixExpr(Infix::And, _, _) => FlatLiteral::helper_evalexpr(self, env).await, 
                // short circuit for ||
                Expr::InfixExpr(Infix::Or, _, _) => FlatLiteral::helper_evalexpr(self, env).await,
                Expr::InfixExpr(op, e1, e2) => {
                    let r1 = e1.eval(env.clone()).await?;
                    match (r1, e2.eval(env).await?) {
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
                            Literal::unit()
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
                            _ => match Self::literal_vector(rs) {
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
                Expr::Iter(op, vs, e1, e2, acc_opt) => match e1.eval(env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::List(lits)) => {
                        let mut res = Vec::new();
                        let mut acc_opt = match acc_opt {
                            Some(e) => Some(e.eval(env.clone()).await?),
                            _=> None
                        };

                        for l in lits.iter() {
                            match l {
                                Literal::Tuple(ref ts) if vs.len() != 1 => {
                                    if vs.len() == ts.len() {
                                        let mut e = *e2.clone();

                                        //Apply the accumulator if any
                                        if acc_opt.is_some() { //FIXME Duplicated
                                            let acc = acc_opt.clone().unwrap();
                                            e = e.apply(&acc)?;
                                        }

                                        for (v, lit) in vs.iter().zip(ts) {
                                            if v != "_" {
                                                e = e.apply(&Expr::LitExpr(lit.clone()))?
                                            }
                                        }
                                        
                                        //Update the acc if any
                                        if acc_opt.is_some() { //FIXME Duplicated
                                            let tmp = e.eval(env.clone()).await?;
                                            acc_opt = Some(tmp.clone());
                                            res.push(tmp)    
                                        } else {
                                            res.push(e.eval(env.clone()).await?)
                                        }
                                    } else {
                                        return Err(Error::new(
                                            "eval, iter-expression (tuple length mismatch)",
                                        ));
                                    }
                                }
                                _ => {
                                    if vs.len() == 1 {
                                        let mut e = *e2.clone();
                                        
                                        //Apply the accumulator if any
                                        if acc_opt.is_some() { //FIXME Duplicated
                                            let acc = acc_opt.clone().unwrap();
                                            e = e.apply(&acc)?;
                                        }

                                        if vs[0] != "_" {
                                            e = e.clone().apply(&Expr::LitExpr(l.clone()))?
                                        }
                                        
                                        //Update the acc if any
                                        if acc_opt.is_some() { //FIXME Duplicated
                                            let tmp = e.eval(env.clone()).await?;
                                            acc_opt = Some(tmp.clone());
                                            res.push(tmp)    
                                        } else {
                                            res.push(e.eval(env.clone()).await?)
                                        }  
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
                            None if op == Iter::Fold => {
                                match acc_opt.unwrap() {
                                    Expr::LitExpr(l) => Ok(l.into()),
                                    _ => Err(Error::new("arg is not a literal")),
                                }
                            }
                            None => match Self::literal_vector(res) {
                                Ok(iter_lits) => match op {
                                    Iter::Map => Ok(Literal::List(iter_lits).into()),
                                    Iter::ForEach => Ok(Expr::from(())),
                                    Iter::Filter => {
                                        let filtered_lits = lits
                                            .into_iter()
                                            .zip(iter_lits.into_iter())
                                            .filter_map(
                                                |(l, b)| if b.get_bool() { Some(l) } else { None },
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
                                    Iter::Fold => unreachable!(),
                                    Iter::All => Ok(iter_lits.iter().all(|l| l.get_bool()).into()),
                                    Iter::Any => Ok(iter_lits.iter().any(|l| l.get_bool()).into()),
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
                    Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && fl.get_bool() => consequence.eval(env).await,
                    Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && !fl.get_bool() => match alternative {
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
                                let mut all_captures: BTreeMap<String, Self> = BTreeMap::new();
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
                            } else if Headers::<FlatTyp>::is_builtin(&function) {
                                Expr::eval_call(function.as_str(), args)
                            } else if let Some((external, method)) = Headers::<FlatTyp>::split(&function) {
                                // external function (RPC) or "Ingress/Egress" metadata
                                let args = Self::literal_vector(args)?;
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
                },
                Expr::Phantom(_) => unreachable!()
            }
        }
        .boxed()
    }
    pub async fn evaluate(self, env: Env<FlatTyp, FlatLiteral>) -> Result<Self, self::Error> {
        Ok(self.eval(env).await?.strip_return())
    }
}