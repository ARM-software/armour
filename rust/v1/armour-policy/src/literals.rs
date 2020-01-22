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
struct Headers {
    headers: BTreeMap<String, Vec<Vec<u8>>>,
}

impl Headers {
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
    pub fn set_header(&mut self, k: &str, v: &[u8]) {
        let s = v.to_vec();
        if let Some(l) = self.headers.get_mut(k) {
            l.push(s)
        } else {
            self.headers.insert(k.to_string(), vec![s]);
        }
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

impl From<Vec<(&str, &[u8])>> for Headers {
    fn from(h: Vec<(&str, &[u8])>) -> Self {
        let mut headers: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
        for (k, v) in h.iter() {
            if let Some(l) = headers.get_mut(&(*k).to_string()) {
                l.push(v.to_vec())
            } else {
                headers.insert((*k).to_string(), vec![v.to_vec()]);
            }
        }
        Headers { headers }
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

impl From<std::net::IpAddr> for ID {
    fn from(ip: std::net::IpAddr) -> Self {
        let mut ips = BTreeSet::new();
        if ip.is_ipv4() {
            ips.insert(ip);
        }
        let mut hosts = BTreeSet::new();
        // log::warn!("performing reverse DNS lookup!");
        if let Ok(host) = dns_lookup::lookup_addr(&ip) {
            hosts.insert(host);
        }
        ID {
            hosts,
            ips,
            port: None,
        }
    }
}

impl From<std::net::SocketAddr> for ID {
    fn from(s: std::net::SocketAddr) -> Self {
        let mut id = ID::from(s.ip());
        id.port = Some(s.port());
        id
    }
}

impl From<(Option<&str>, Option<u16>)> for ID {
    fn from(s: (Option<&str>, Option<u16>)) -> Self {
        let (host, port) = s;
        let mut hosts = BTreeSet::new();
        let mut ips = BTreeSet::new();
        if let Some(host) = host {
            hosts.insert(host.to_string());
            // log::warn!("performing DNS lookup!");
            if let Ok(host_ips) = dns_lookup::lookup_host(host) {
                for ip in host_ips.iter().filter(|ip| ip.is_ipv4()) {
                    ips.insert(*ip);
                }
            }
        }
        ID { hosts, ips, port }
    }
}

#[derive(PartialEq, Default, Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    from: ID,
    to: ID,
    number: i64,
}

impl Connection {
    pub fn literal(from: &ID, to: &ID, number: i64) -> Literal {
        Literal::Connection(Connection {
            from: from.clone(),
            to: to.clone(),
            number,
        })
    }
    pub fn from_to(&self) -> Literal {
        Literal::Tuple(vec![self.from_lit(), self.to_lit()])
    }
    pub fn from_lit(&self) -> Literal {
        Literal::ID(self.from.clone())
    }
    pub fn to_lit(&self) -> Literal {
        Literal::ID(self.to.clone())
    }
    pub fn number(&self) -> Literal {
        Literal::Int(self.number)
    }
    pub fn set_from(&self, from: &ID) -> Connection {
        let mut conn = self.clone();
        conn.from = from.clone();
        conn
    }
    pub fn set_to(&self, to: &ID) -> Connection {
        let mut conn = self.clone();
        conn.to = to.clone();
        conn
    }
    pub fn set_number(&self, number: i64) -> Connection {
        let mut conn = self.clone();
        conn.number = number;
        conn
    }
}

impl From<(&ID, &ID, usize)> for Connection {
    fn from(conn: (&ID, &ID, usize)) -> Self {
        let (from, to, number) = conn;
        Connection {
            from: from.clone(),
            to: to.clone(),
            number: number as i64,
        }
    }
}

#[derive(PartialEq, Default, Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    payload: Vec<u8>,
    connection: Connection,
}

impl Payload {
    pub fn literal(data: &[u8], connection: &Connection) -> Literal {
        Literal::Payload(Payload {
            payload: data.to_vec(),
            connection: connection.clone(),
        })
    }
    pub fn data(&self) -> Literal {
        self.payload.clone().into()
    }
    pub fn connection(&self) -> Literal {
        self.connection.clone().into()
    }
}

impl From<(&[u8], &Connection)> for Payload {
    fn from(pld: (&[u8], &Connection)) -> Self {
        Payload {
            payload: pld.0.to_vec(),
            connection: pld.1.clone(),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRequest {
    method: Method,
    version: Version,
    path: String,
    query: String,
    headers: Headers,
    connection: Connection,
}

impl HttpRequest {
    pub fn new(method: Method) -> HttpRequest {
        let mut new = HttpRequest::default();
        new.method = method;
        new
    }
    pub fn connection(&self) -> Literal {
        self.connection.clone().into()
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
        self.headers.header(s)
    }
    pub fn unique_header(&self, s: &str) -> Literal {
        self.headers.unique_header(s)
    }
    pub fn set_header(&self, k: &str, v: &[u8]) -> HttpRequest {
        let mut new = self.clone();
        new.headers.set_header(k, v);
        new
    }
    pub fn headers(&self) -> Literal {
        self.headers.headers()
    }
    pub fn header_pairs(&self) -> Literal {
        self.headers.header_pairs()
    }
    pub fn set_connection(&self, c: &Connection) -> HttpRequest {
        let mut new = self.clone();
        new.connection = c.clone();
        new
    }
}

impl From<(&str, &str, &str, &str, Vec<(&str, &[u8])>, Connection)> for HttpRequest {
    #[allow(clippy::type_complexity)]
    fn from(req: (&str, &str, &str, &str, Vec<(&str, &[u8])>, Connection)) -> Self {
        let (method, version, path, query, h, connection) = req;
        HttpRequest {
            method: method.parse().unwrap_or_default(),
            version: version.parse().unwrap_or_default(),
            path: path.to_owned(),
            query: query.to_owned(),
            headers: Headers::from(h),
            connection,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpResponse {
    version: Version,
    status: u16,
    headers: Headers,
    reason: Option<String>,
    connection: Connection,
}

impl HttpResponse {
    pub fn literal(status: u16) -> Literal {
        let mut new = HttpResponse::default();
        new.status = status;
        Literal::HttpResponse(new)
    }
    pub fn connection(&self) -> Literal {
        self.connection.clone().into()
    }
    pub fn status(&self) -> Literal {
        Literal::Int(i64::from(self.status))
    }
    pub fn version(&self) -> Literal {
        Literal::Str(self.version.to_string())
    }
    pub fn header(&self, s: &str) -> Literal {
        self.headers.header(s)
    }
    pub fn unique_header(&self, s: &str) -> Literal {
        self.headers.unique_header(s)
    }
    pub fn reason(&self) -> Literal {
        if let Some(ref reason) = self.reason {
            Literal::Str(reason.clone()).some()
        } else {
            Literal::none()
        }
    }
    pub fn set_reason(&self, reason: &str) -> HttpResponse {
        let mut new = self.clone();
        new.reason = Some(reason.to_string());
        new
    }
    pub fn set_header(&self, k: &str, v: &[u8]) -> HttpResponse {
        let mut new = self.clone();
        new.headers.set_header(k, v);
        new
    }
    pub fn headers(&self) -> Literal {
        self.headers.headers()
    }
    pub fn header_pairs(&self) -> Literal {
        self.headers.header_pairs()
    }
    pub fn set_connection(&self, c: &Connection) -> HttpResponse {
        let mut new = self.clone();
        new.connection = c.clone();
        new
    }
}

impl From<(&str, u16, Option<&str>, Vec<(&str, &[u8])>, Connection)> for HttpResponse {
    #[allow(clippy::type_complexity)]
    fn from(res: (&str, u16, Option<&str>, Vec<(&str, &[u8])>, Connection)) -> Self {
        let (version, status, reason, h, connection) = res;
        HttpResponse {
            version: version.parse().unwrap_or_default(),
            status,
            reason: reason.map(|s| s.to_string()),
            headers: Headers::from(h),
            connection,
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
    Connection(Connection),
    Data(Vec<u8>),
    Float(f64),
    HttpRequest(HttpRequest),
    HttpResponse(HttpResponse),
    ID(ID),
    Int(i64),
    IpAddr(std::net::IpAddr),
    List(Vec<Literal>),
    Payload(Payload),
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
            Literal::Connection(_) => Typ::Connection,
            Literal::Data(_) => Typ::Data,
            Literal::Float(_) => Typ::F64,
            Literal::HttpRequest(_) => Typ::HttpRequest,
            Literal::HttpResponse(_) => Typ::HttpResponse,
            Literal::ID(_) => Typ::ID,
            Literal::Int(_) => Typ::I64,
            Literal::IpAddr(_) => Typ::IpAddr,
            Literal::List(l) => l.get(0).map(|t| t.typ()).unwrap_or(Typ::Return),
            Literal::Payload(_) => Typ::Payload,
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
            Literal::Connection(c) => write!(f, "{:?}", c),
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
            Literal::HttpResponse(r) => write!(f, "{:?}", r),
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
            Literal::Payload(p) => write!(f, "{:?}", p),
            // Literal::Policy(p) => write!(f, "{:?}", p),
            Literal::Unit => write!(f, "()"),
        }
    }
}

impl From<bool> for Literal {
    fn from(b: bool) -> Self {
        Literal::Bool(b)
    }
}

impl From<Connection> for Literal {
    fn from(conn: Connection) -> Self {
        Literal::Connection(conn)
    }
}

impl From<&Connection> for Literal {
    fn from(conn: &Connection) -> Self {
        Literal::Tuple(vec![
            (&conn.from).into(),
            (&conn.to).into(),
            conn.number.into(),
        ])
    }
}

impl From<Vec<u8>> for Literal {
    fn from(d: Vec<u8>) -> Self {
        Literal::Data(d)
    }
}

impl From<&[u8]> for Literal {
    fn from(d: &[u8]) -> Self {
        Literal::Data(d.to_vec())
    }
}

impl From<f64> for Literal {
    fn from(n: f64) -> Self {
        Literal::Float(n)
    }
}

impl From<HttpRequest> for Literal {
    fn from(r: HttpRequest) -> Self {
        Literal::HttpRequest(r)
    }
}

impl From<&HttpRequest> for Literal {
    fn from(req: &HttpRequest) -> Self {
        Literal::Tuple(vec![
            req.method(),
            req.version(),
            req.path(),
            req.query(),
            req.header_pairs(),
            (&req.connection).into(),
        ])
    }
}

impl From<HttpResponse> for Literal {
    fn from(r: HttpResponse) -> Self {
        Literal::HttpResponse(r)
    }
}

impl From<&HttpResponse> for Literal {
    fn from(res: &HttpResponse) -> Self {
        Literal::Tuple(vec![
            res.version(),
            res.status(),
            res.header_pairs(),
            (&res.connection).into(),
        ])
    }
}

impl From<ID> for Literal {
    fn from(id: ID) -> Self {
        Literal::ID(id)
    }
}

impl From<&ID> for Literal {
    fn from(id: &ID) -> Self {
        Literal::Tuple(vec![id.hosts(), id.ips(), id.port()])
    }
}

impl From<std::net::SocketAddr> for Literal {
    fn from(s: std::net::SocketAddr) -> Self {
        Literal::ID(s.into())
    }
}

impl From<usize> for Literal {
    fn from(n: usize) -> Self {
        Literal::Int(n as i64)
    }
}

impl From<i64> for Literal {
    fn from(n: i64) -> Self {
        Literal::Int(n)
    }
}

impl From<std::net::IpAddr> for Literal {
    fn from(ip: std::net::IpAddr) -> Self {
        Literal::IpAddr(ip)
    }
}

impl From<&std::net::IpAddr> for Literal {
    fn from(ip: &std::net::IpAddr) -> Self {
        match ip {
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
                    Literal::from(&std::net::IpAddr::V4(ipv4))
                } else {
                    Literal::none()
                }
            }
        }
    }
}

impl From<Payload> for Literal {
    fn from(pld: Payload) -> Self {
        Literal::Payload(pld)
    }
}

impl From<&Payload> for Literal {
    fn from(pld: &Payload) -> Self {
        Literal::Tuple(vec![
            pld.payload.as_slice().into(),
            (&pld.connection).into(),
        ])
    }
}

impl From<&str> for Literal {
    fn from(s: &str) -> Self {
        Literal::Str(s.to_string())
    }
}

impl From<()> for Literal {
    fn from(_: ()) -> Self {
        Literal::Unit
    }
}

impl std::convert::TryFrom<Literal> for bool {
    type Error = ();
    fn try_from(l: Literal) -> Result<bool, Self::Error> {
        if let Literal::Bool(b) = l {
            Ok(b)
        } else {
            Err(())
        }
    }
}

impl std::convert::TryFrom<Literal> for () {
    type Error = ();
    fn try_from(l: Literal) -> Result<(), Self::Error> {
        if l == Literal::Unit {
            Ok(())
        } else {
            Err(())
        }
    }
}