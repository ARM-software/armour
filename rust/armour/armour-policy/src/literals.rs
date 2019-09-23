use super::{parser, types::Typ};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display};
use std::str::FromStr;
use url;

// #[derive(PartialEq, Debug, Display, Clone, Serialize, Deserialize)]
// pub enum Policy {
//     Accept,
//     Forward,
//     Reject,
// }

#[derive(PartialEq, Debug, Display, Clone, Serialize, Deserialize)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    CONNECT,
    PATCH,
    TRACE,
}

impl FromStr for Method {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            "CONNECT" => Ok(Method::CONNECT),
            "PATCH" => Ok(Method::PATCH),
            "TRACE" => Ok(Method::TRACE),
            _ => Err(()),
        }
    }
}

impl Default for Method {
    fn default() -> Self {
        Method::GET
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
enum Version {
    HTTP_09,
    HTTP_10,
    HTTP_11,
    HTTP_20,
}

impl FromStr for Version {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HTTP/0.9" | "HTTP_09" => Ok(Version::HTTP_09),
            "HTTP/1.0" | "HTTP_10" => Ok(Version::HTTP_10),
            "HTTP/1.1" | "HTTP_11" => Ok(Version::HTTP_11),
            "HTTP/2.0" | "HTTP_20" => Ok(Version::HTTP_20),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Version::HTTP_09 => "HTTP/0.9",
            Version::HTTP_10 => "HTTP/1.0",
            Version::HTTP_11 => "HTTP/1.1",
            Version::HTTP_20 => "HTTP/2.0",
        };
        write!(f, "{}", s)
    }
}

impl Default for Version {
    fn default() -> Self {
        Version::HTTP_11
    }
}

#[derive(PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRequest {
    method: Method,
    version: Version,
    path: String,
    query: String,
    headers: BTreeMap<String, Vec<Vec<u8>>>,
}

impl HttpRequest {
    pub fn new(method: Method) -> HttpRequest {
        let mut new = HttpRequest::default();
        new.method = method;
        new
    }
    pub fn method(&self) -> Literal {
        Literal::Str(self.method.to_string())
    }
    pub fn version(&self) -> Literal {
        Literal::Str(self.version.to_string())
    }
    pub fn path(&self) -> Literal {
        Literal::Str(self.path.to_string())
    }
    pub fn set_path(&self, s: &str) -> HttpRequest {
        let mut new = self.clone();
        new.path = s.to_string();
        new
    }
    pub fn route(&self) -> Literal {
        Literal::List(
            self.path
                .trim_matches('/')
                .split('/')
                .map(|s| Literal::Str(s.to_string()))
                .collect(),
        )
    }
    pub fn query(&self) -> Literal {
        Literal::Str(self.query.to_string())
    }
    pub fn set_query(&self, s: &str) -> HttpRequest {
        let mut new = self.clone();
        new.query = s.to_string();
        new
    }
    pub fn query_pairs(&self) -> Literal {
        if let Ok(url) = url::Url::parse(&format!("http://x/?{}", self.query)) {
            Literal::List(
                url.query_pairs()
                    .map(|(k, v)| {
                        Literal::Tuple(vec![
                            Literal::Str(k.to_string()),
                            Literal::Str(v.to_string()),
                        ])
                    })
                    .collect(),
            )
        } else {
            Literal::List(Vec::new())
        }
    }
    pub fn header(&self, s: &str) -> Literal {
        match self.headers.get(s) {
            None => Literal::none(),
            Some(vs) => Literal::List(vs.iter().map(|v| Literal::Data(v.clone())).collect()).some(),
        }
    }
    pub fn unique_header(&self, s: &str) -> Literal {
        match self.headers.get(s) {
            Some(v) => match v.as_slice() {
                [d] => Literal::Data(d.clone()).some(),
                _ => Literal::none(),
            },
            _ => Literal::none(),
        }
    }
    pub fn set_header(&self, k: &str, v: &[u8]) -> HttpRequest {
        let mut new = self.clone();
        let s = v.to_vec();
        if let Some(l) = new.headers.get_mut(k) {
            l.push(s)
        } else {
            new.headers.insert(k.to_string(), vec![s]);
        }
        new
    }
    pub fn headers(&self) -> Literal {
        Literal::List(
            self.headers
                .keys()
                .map(|k| Literal::Str(k.to_string()))
                .collect(),
        )
    }
    pub fn header_pairs(&self) -> Literal {
        let mut pairs = Vec::new();
        for (k, vs) in self.headers.iter() {
            for v in vs {
                pairs.push(Literal::Tuple(vec![
                    Literal::Str(k.to_string()),
                    Literal::Data(v.to_vec()),
                ]))
            }
        }
        Literal::List(pairs)
    }
}

#[derive(PartialEq, Default, Debug, Clone, Serialize, Deserialize)]
pub struct ID {
    hosts: BTreeSet<String>,
    ips: BTreeSet<std::net::IpAddr>,
    port: Option<u16>,
}

impl ID {
    pub fn host(&self) -> Option<String> {
        if let Some(name) = self.hosts.iter().next() {
            if std::net::IpAddr::from_str(name).is_err() {
                Some(name.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn hosts(&self) -> Literal {
        Literal::List(
            self.hosts
                .iter()
                .map(|s| Literal::Str(s.to_string()))
                .collect(),
        )
    }
    pub fn ips(&self) -> Literal {
        Literal::List(self.ips.iter().map(|ip| Literal::IpAddr(*ip)).collect())
    }
    pub fn port(&self) -> Literal {
        match self.port {
            Some(p) => Literal::Int(p.into()).some(),
            None => Literal::none(),
        }
    }
    pub fn add_host(&self, host: &str) -> ID {
        let mut new = self.clone();
        new.hosts.insert(host.to_string());
        new
    }
    pub fn add_ip(&self, ip: std::net::IpAddr) -> ID {
        let mut new = self.clone();
        new.ips.insert(ip);
        new
    }
    pub fn set_port(&self, port: u16) -> ID {
        let mut new = self.clone();
        new.port = Some(port);
        new
    }
}

pub trait ToLiteral {
    fn to_literal(&self) -> Literal;
}

impl ToLiteral for std::net::IpAddr {
    fn to_literal(&self) -> Literal {
        match self {
            std::net::IpAddr::V4(ip) => {
                let [a, b, c, d] = ip.octets();
                #[allow(clippy::cast_lossless)]
                Literal::Tuple(vec![
                    Literal::Int(a as i64),
                    Literal::Int(b as i64),
                    Literal::Int(c as i64),
                    Literal::Int(d as i64),
                ])
            }
            std::net::IpAddr::V6(ip) => {
                if let Some(ipv4) = ip.to_ipv4() {
                    std::net::IpAddr::V4(ipv4).to_literal()
                } else {
                    Literal::none()
                }
            }
        }
    }
}

impl ToLiteral for HttpRequest {
    fn to_literal(&self) -> Literal {
        Literal::Tuple(vec![
            self.method(),
            self.version(),
            self.path(),
            self.query(),
            self.header_pairs(),
        ])
    }
}

impl ToLiteral for ID {
    fn to_literal(&self) -> Literal {
        Literal::Tuple(vec![self.hosts(), self.ips(), self.port()])
    }
}

impl From<(&str, &str, &str, &str, Vec<(&str, &[u8])>)> for HttpRequest {
    #[allow(clippy::type_complexity)]
    fn from(req: (&str, &str, &str, &str, Vec<(&str, &[u8])>)) -> Self {
        let (method, version, path, query, h) = req;
        let mut headers: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
        for (k, v) in h.iter() {
            if let Some(l) = headers.get_mut(&k.to_string()) {
                l.push(v.to_vec())
            } else {
                headers.insert(k.to_string(), vec![v.to_vec()]);
            }
        }
        HttpRequest {
            method: method.parse().unwrap_or_default(),
            version: version.parse().unwrap_or_default(),
            path: path.to_owned(),
            query: query.to_owned(),
            headers,
        }
    }
}

pub struct VecSet;

impl VecSet {
    pub fn contains(l: &[Literal], x: &Literal) -> Literal {
        Literal::Bool(l.iter().any(|y| x == y))
    }
    pub fn is_subset(x: &[Literal], y: &[Literal]) -> Literal {
        Literal::Bool(x.iter().all(|ex| y.iter().any(|ey| ex == ey)))
    }
    pub fn is_disjoint(x: &[Literal], y: &[Literal]) -> Literal {
        Literal::Bool(!x.iter().any(|ex| y.iter().any(|ey| ex == ey)))
    }
    pub fn difference(x: &[Literal], y: &[Literal]) -> Literal {
        Literal::List(
            x.to_owned()
                .into_iter()
                .filter(|ex| !y.iter().any(|ey| ex == ey))
                .collect(),
        )
    }
    pub fn intersection(x: &[Literal], y: &[Literal]) -> Literal {
        Literal::List(
            x.to_owned()
                .into_iter()
                .filter(|ex| y.iter().any(|ey| ex == ey))
                .collect(),
        )
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Bool(bool),
    Data(Vec<u8>),
    Float(f64),
    HttpRequest(HttpRequest),
    ID(ID),
    Int(i64),
    IpAddr(std::net::IpAddr),
    List(Vec<Literal>),
    // Policy(Policy),
    Regex(parser::PolicyRegex),
    Str(String),
    Tuple(Vec<Literal>),
    Unit,
}

impl Literal {
    fn is_tuple(&self) -> bool {
        match self {
            Literal::Tuple(_) => true,
            _ => false,
        }
    }
    pub fn typ(&self) -> Typ {
        match self {
            Literal::Bool(_) => Typ::Bool,
            Literal::Data(_) => Typ::Data,
            Literal::Float(_) => Typ::F64,
            Literal::HttpRequest(_) => Typ::HttpRequest,
            Literal::ID(_) => Typ::ID,
            Literal::Int(_) => Typ::I64,
            Literal::IpAddr(_) => Typ::IpAddr,
            Literal::List(l) => l.get(0).map(|t| t.typ()).unwrap_or(Typ::Return),
            // Literal::Policy(_) => Typ::Policy,
            Literal::Regex(_) => Typ::Regex,
            Literal::Str(_) => Typ::Str,
            Literal::Tuple(l) => Typ::Tuple((*l).iter().map(|t: &Literal| t.typ()).collect()),
            Literal::Unit => Typ::Unit,
        }
    }
    pub fn none() -> Literal {
        Literal::Tuple(Vec::new())
    }
    pub fn some(&self) -> Literal {
        Literal::Tuple(vec![self.clone()])
    }
    pub fn dest_some(&self) -> Option<Literal> {
        match self {
            Literal::Tuple(v) => match v.as_slice() {
                [ref l] => Some(l.clone()),
                _ => None,
            },
            _ => None,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::Data(d) => {
                if let Ok(s) = std::str::from_utf8(d) {
                    write!(f, r#"b"{}""#, s)
                } else {
                    write!(f, "{:x?}", d)
                }
            }
            Literal::Float(d) => {
                if 8 < d.abs().log10() as usize {
                    write!(f, "{:e}", d)
                } else if (d.trunc() - *d).abs() < std::f64::EPSILON {
                    write!(f, "{:.1}", d)
                } else {
                    write!(f, "{}", d)
                }
            }
            Literal::HttpRequest(r) => write!(f, "{:?}", r),
            Literal::ID(id) => write!(f, "{:?}", id),
            Literal::Int(i) => write!(f, "{}", i),
            Literal::IpAddr(ip) => write!(f, "{}", ip),
            Literal::List(lits) | Literal::Tuple(lits) => {
                let s = lits
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                if self.is_tuple() {
                    match lits.len() {
                        0 => write!(f, "None"),
                        1 => write!(f, "Some({})", s),
                        _ => write!(f, "({})", s),
                    }
                } else {
                    write!(f, "[{}]", s)
                }
            }
            Literal::Regex(r) => write!(f, "{:?}", r),
            Literal::Str(s) => write!(f, r#""{}""#, s),
            // Literal::Policy(p) => write!(f, "{:?}", p),
            Literal::Unit => write!(f, "()"),
        }
    }
}
