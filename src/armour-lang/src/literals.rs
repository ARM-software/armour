use super::pretty::TPrettyLit;
use super::{
    interpret::TInterpret,
    labels, 
    parser, 
    types::{CPFlatTyp, CPTyp, Typ, FlatTyp, TFlatTyp, TTyp}
};
use super::policies::{TProtocol, DPPolicies};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display};
use std::str::FromStr;
use std::marker::PhantomData;

//Helpers to build patterns
//Defined here since macros can only be used after they have been defined
#[macro_export]
macro_rules! dpflatlit (
    ($i: ident ) => (
        DPFlatLiteral::$i
    );
    ($i: ident ($($args:tt)*) ) => (
        DPFlatLiteral::$i($($args)*)
    );
);


#[macro_export]
macro_rules! cpflatlit (
    ($i: ident ) => (
        CPFlatLiteral::$i
    );
    ($i: ident ($($args:tt)*) ) => (
        CPFlatLiteral::$i($($args)*)
    );
);

#[macro_export]
macro_rules! cpdpflatlit (
    ($i: ident ($($args:tt)*) ) => (
        CPFlatLiteral::DPFlatLiteral(DPFlatLiteral::$i($($args)*))
    );
);
#[macro_export]
macro_rules! dplit (
    ($i: ident) => (
        Literal::FlatLiteral(DPFlatLiteral::$i)
    );
    ($i: ident ($($args:tt)*) ) => (
        Literal::FlatLiteral(DPFlatLiteral::$i($($args)*))
    );
);
#[macro_export]
macro_rules! cplit (
    ($i: ident ($($args:tt)*) ) => (
        Literal::FlatLiteral(CPFlatLiteral::$i($($args)*))
    );
);

#[macro_export]
macro_rules! cpdplit (
    ($i: ident ($($args:tt)*) ) => (
        Literal::FlatLiteral(CPFlatLiteral::DPFlatLiteral(DPFlatLiteral::$i($($args)*)))
    );
);

#[derive( PartialEq, Debug, Display, Clone, Serialize, Deserialize)]
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

#[derive( PartialEq, Debug, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum Version {
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

#[derive( PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Headers<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub headers: BTreeMap<String, Vec<Vec<u8>>>,
    phantom: PhantomData<(FlatTyp, FlatLiteral)>,
}

pub type DPHeaders = Headers<FlatTyp, FlatLiteral>;
pub type CPHeaders = Headers<CPFlatTyp, CPFlatLiteral>;

impl From<CPHeaders> for DPHeaders {
    fn from(h: CPHeaders) -> DPHeaders {
        DPHeaders{
            headers: h.headers,
            phantom: PhantomData
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Headers<FlatTyp, FlatLiteral> {
    pub fn header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        match self.headers.get(s) {
            None => Literal::none(),
            Some(vs) => Literal::List(vs.iter().map(|v| Literal::data(v.clone())).collect()).some(),
        }
    }
    pub fn unique_header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        match self.headers.get(s) {
            Some(v) => match v.as_slice() {
                [d] => Literal::data(d.clone()).some(),
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
    pub fn headers(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            self.headers
                .keys()
                .map(|k| Literal::FlatLiteral(FlatLiteral::str(k.to_string())))
                .collect(),
        )
    }
    pub fn header_pairs(&self) -> Literal<FlatTyp, FlatLiteral> {
        let mut pairs = Vec::new();
        for (k, vs) in self.headers.iter() {
            for v in vs {
                pairs.push(Literal::Tuple(vec![
                    Literal::FlatLiteral(FlatLiteral::str(k.to_string())),
                    Literal::FlatLiteral(FlatLiteral::data(v.to_vec())),
                ]))
            }
        }
        Literal::List(pairs)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Vec<(&str, &[u8])>> for Headers<FlatTyp, FlatLiteral> {
    fn from(h: Vec<(&str, &[u8])>) -> Self {
        let mut headers: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
        for (k, v) in h.iter() {
            if let Some(l) = headers.get_mut(&(*k).to_string()) {
                l.push(v.to_vec())
            } else {
                headers.insert((*k).to_string(), vec![v.to_vec()]);
            }
        }
        Headers { headers, phantom: PhantomData}
    }
}

#[derive( PartialEq, Default, Debug, Clone, Serialize, Deserialize)]
pub struct ID<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub hosts: BTreeSet<String>,
    pub ips: BTreeSet<std::net::IpAddr>,

    #[serde(default)]
    #[serde(serialize_with = "port_serde::serialize")]
    #[serde(deserialize_with = "port_serde::deserialize")]
    pub port: Option<u16>,
    pub labels: BTreeSet<labels::Label>,
    phantom : PhantomData<(FlatTyp, FlatLiteral)>,
}
pub type DPID = ID<FlatTyp, FlatLiteral>;
pub type CPID = ID<CPFlatTyp, CPFlatLiteral>;

impl From<CPID> for DPID {
    fn from(cpid: CPID) -> Self {
        DPID{
            hosts: cpid.hosts,
            ips: cpid.ips,
            port: cpid.port,
            labels: cpid.labels,
            phantom: PhantomData
        }
    }
}
impl From<DPID> for CPID {
    fn from(dpid: DPID) -> Self {
        CPID{
            hosts: dpid.hosts,
            ips: dpid.ips,
            port: dpid.port,
            labels: dpid.labels,
            phantom: PhantomData
        }
    }
}

mod port_serde {
    use serde::{self, de, Serializer, Deserializer};
    use std::fmt;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
    where D: Deserializer<'de> {
        
        struct FromPort ();

        impl<'de> de::Visitor<'de> for FromPort {
            type Value = Option<u16>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an Option<u16>")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> 
            where
                E: de::Error,
            {
                Ok(None)
            }    
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,            
            {
                Ok(Some(bson::compat::u2f::deserialize(deserializer)?))
            }
        }
        
        
        deserializer.deserialize_option(FromPort())
    }

    pub fn serialize<S>(x: &Option<u16>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {   
        match *x {
            Some(p) => bson::compat::u2f::serialize(&p, s),
            None => s.serialize_none()
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> ID<FlatTyp, FlatLiteral> {
    pub fn new(
        hosts: BTreeSet<String>,
        ips: BTreeSet<std::net::IpAddr>,
        port: Option<u16>,
        labels: BTreeSet<labels::Label>,
    ) -> Self {
        ID {
            hosts,
            ips,
            port,
            labels,
            phantom: PhantomData
        }
    }
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
    pub fn labels(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            self.labels
                .iter()
                .map(|l| Literal::label(l.clone()))
                .collect(),
        )
    }
    pub fn hosts(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            self.hosts
                .iter()
                .map(|s| Literal::str(s.to_string()))
                .collect(),
        )
    }
    pub fn ips(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(self.ips.iter().map(|ip| Literal::ip_addr(*ip)).collect())
    }

    pub fn port(&self) -> Option<u16> {
        self.port
    } 

    pub fn port_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        match self.port {
            Some(p) => Literal::int(p.into()).some(),
            None => Literal::none(),
        }
    }
    pub fn add_label(&self, label: &labels::Label) -> Self {
        let mut new = self.clone();
        new.labels.insert(label.clone());
        new
    }
    pub fn add_host(&self, host: &str) -> Self {
        let mut new = self.clone();
        new.hosts.insert(host.to_string());
        new
    }
    pub fn add_ip(&self, ip: std::net::IpAddr) -> Self {
        let mut new = self.clone();
        new.ips.insert(ip);
        new
    }
    pub fn set_port(&self, port: u16) -> Self {
        let mut new = self.clone();
        new.port = Some(port);
        new
    }
    pub fn has_label(&self, label: &labels::Label) -> bool {
        self.labels.iter().any(|x| label.matches_with(x))
    }
    pub fn find_label(&self, label: &labels::Label) -> Option<&labels::Label> {
        self.labels.iter().find(|x| label.matches_with(x))
    }
    pub fn has_host(&self, host: &str) -> bool {
        self.hosts.iter().any(|x| x == host)
    }
    pub fn has_ip(&self, ip: &std::net::IpAddr) -> bool {
        self.ips.iter().any(|x| x == ip)
    }
}

#[derive( PartialEq, Default, Debug, Clone, Serialize, Deserialize)]
pub struct Connection<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub from: ID<FlatTyp, FlatLiteral>,
    pub to: ID<FlatTyp, FlatLiteral>,
    pub number: i64,
    phantom : PhantomData<(FlatTyp, FlatLiteral)>,
}
pub type DPConnection = Connection<FlatTyp, FlatLiteral>;
pub type CPConnection = Connection<CPFlatTyp, CPFlatLiteral>;

impl From<CPConnection> for DPConnection {
    fn from(cpco: CPConnection) -> DPConnection {
        DPConnection {
            from: DPID::from(cpco.from),
            to: DPID::from(cpco.to),
            number: cpco.number,
            phantom: PhantomData
        }
    }
}



impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Connection<FlatTyp, FlatLiteral> {
    pub fn new(from: &ID<FlatTyp, FlatLiteral>, to: &ID<FlatTyp, FlatLiteral>, number: i64) -> Self {
        Connection {
            from: from.clone(),
            to: to.clone(), 
            number: number,
            phantom: PhantomData
        }
    }
    pub fn literal(from: &ID<FlatTyp, FlatLiteral>, to: &ID<FlatTyp, FlatLiteral>, number: i64) -> Literal<FlatTyp, FlatLiteral> {
        Literal::FlatLiteral(FlatLiteral::connection_from(from, to, number))//Since we can not do overloading, 
        
    }
    pub fn from_to(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::Tuple(vec![self.from_lit(), self.to_lit()])
    }
    pub fn from_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::id(self.from.clone())
    }
    pub fn to_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::id(self.to.clone())
    }
    pub fn number(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::int(self.number)
    }
    pub fn set_from(&self, from: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut conn = self.clone();
        conn.from = from.clone();
        conn
    }
    pub fn set_to(&self, to: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut conn = self.clone();
        conn.to = to.clone();
        conn
    }
    pub fn set_number(&self, number: i64) -> Self {
        let mut conn = self.clone();
        conn.number = number;
        conn
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<(&ID<FlatTyp, FlatLiteral>, &ID<FlatTyp, FlatLiteral>, usize)> for Connection<FlatTyp, FlatLiteral> {
    fn from(conn: (&ID<FlatTyp, FlatLiteral>, &ID<FlatTyp, FlatLiteral>, usize)) -> Self {
        let (from, to, number) = conn;
        Connection {
            from: from.clone(),
            to: to.clone(),
            number: number as i64,
            phantom: PhantomData
        }
    }
}

#[derive( PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpRequest<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub method: Method,
    pub version: Version,
    pub path: String,
    pub query: String,
    pub headers: Headers<FlatTyp, FlatLiteral>,
    pub connection: Connection<FlatTyp, FlatLiteral>,
}

pub type DPHttpRequest = HttpRequest<FlatTyp, FlatLiteral>;
pub type CPHttpRequest = HttpRequest<CPFlatTyp, CPFlatLiteral>;

impl From<CPHttpRequest> for DPHttpRequest{
    fn from(req: CPHttpRequest) -> Self {
        HttpRequest {
            method: req.method, 
            version: req.version,
            path: req.path,
            query: req.query,
            headers: DPHeaders::from(req.headers),
            connection: Connection::from(req.connection),
        }
    } 
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> HttpRequest<FlatTyp, FlatLiteral> {
    pub fn new(
        method: &str,
        version: &str,
        path: &str,
        query: &str,
        headers: Vec<(&str, &[u8])>,
        connection: Connection<FlatTyp, FlatLiteral>,
    ) -> Self {
        HttpRequest {
            method: method.parse().unwrap_or_default(),
            version: version.parse().unwrap_or_default(),
            path: path.to_owned(),
            query: query.to_owned(),
            headers: Headers::from(headers),
            connection,
        }
    }
    pub fn connection(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.clone().into()
    }
    pub fn method(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::str(self.method.to_string())
    }
    pub fn version(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::str(self.version.to_string())
    }
    pub fn path(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::str(self.path.to_string())
    }
    pub fn set_path(&self, s: &str) -> Self {
        let mut new = self.clone();
        new.path = s.to_string();
        new
    }
    pub fn route(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            self.path
                .trim_matches('/')
                .split('/')
                .map(|s| Literal::str(s.to_string()))
                .collect(),
        )
    }
    pub fn query(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::str(self.query.to_string())
    }
    pub fn set_query(&self, s: &str) -> Self {
        let mut new = self.clone();
        new.query = s.to_string();
        new
    }
    pub fn query_pairs(&self) -> Literal<FlatTyp, FlatLiteral> {
        if let Ok(url) = url::Url::parse(&format!("http://x/?{}", self.query)) {
            Literal::List(
                url.query_pairs()
                    .map(|(k, v)| {
                        Literal::Tuple(vec![
                            Literal::str(k.to_string()),
                            Literal::str(v.to_string()),
                        ])
                    })
                    .collect(),
            )
        } else {
            Literal::List(Vec::new())
        }
    }
    pub fn header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.header(s)
    }
    pub fn unique_header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.unique_header(s)
    }
    pub fn set_header(&self, k: &str, v: &[u8]) -> Self {
        let mut new = self.clone();
        new.headers.set_header(k, v);
        new
    }
    pub fn headers(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.headers()
    }
    pub fn header_pairs(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.header_pairs()
    }
    pub fn set_connection(&self, c: &Connection<FlatTyp, FlatLiteral>) -> Self {
        let mut new = self.clone();
        new.connection = c.clone();
        new
    }
    pub fn from_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.from_lit()
    }
    pub fn to_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.to_lit()
    }
    pub fn from_to(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.from_to()
    }
    pub fn set_from(&self, from: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut req = self.clone();
        req.connection.from = from.clone();
        req
    }
    pub fn set_to(&self, to: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut req = self.clone();
        req.connection.to = to.clone();
        req
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Method> for HttpRequest<FlatTyp, FlatLiteral> {
    fn from(method: Method) -> Self {
        let mut new = Self::default();
        new.method = method;
        new
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Method> for Literal<FlatTyp, FlatLiteral> {
    fn from(method: Method) -> Self {
        Self::http_request(Box::new(method.into()))
    }
}

#[derive( PartialEq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpResponse<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>  {
    version: Version,
    status: u16,
    headers: Headers<FlatTyp, FlatLiteral>,
    reason: Option<String>,
    connection: Connection<FlatTyp, FlatLiteral>,
}

pub type DPHttpResponse = HttpResponse<FlatTyp, FlatLiteral>;
pub type CPHttpResponse = HttpResponse<CPFlatTyp, CPFlatLiteral>;

impl From<CPHttpResponse> for DPHttpResponse{
    fn from(req: CPHttpResponse) -> Self {
        HttpResponse {
            version: req.version,
            status: req.status,
            headers: Headers::from(req.headers),
            reason: req.reason, 
            connection: Connection::from(req.connection),
        }
    } 
}
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> HttpResponse<FlatTyp, FlatLiteral> {
    pub fn new(
        version: &str,
        status: u16,
        reason: Option<&str>,
        headers: Vec<(&str, &[u8])>,
        connection: Connection<FlatTyp, FlatLiteral>,
    ) -> Self {
        HttpResponse {
            version: version.parse().unwrap_or_default(),
            status,
            reason: reason.map(|s| s.to_string()),
            headers: Headers::from(headers),
            connection,
        }
    }
    pub fn literal(status: u16) -> Literal<FlatTyp, FlatLiteral> {
        let mut new = Self::default();
        new.status = status;
        new.into()
    }
    pub fn connection(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.clone().into()
    }
    pub fn status(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::int(i64::from(self.status))
    }
    pub fn version(&self) -> Literal<FlatTyp, FlatLiteral> {
        Literal::str(self.version.to_string())
    }
    pub fn header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.header(s)
    }
    pub fn unique_header(&self, s: &str) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.unique_header(s)
    }
    pub fn reason(&self) -> Literal<FlatTyp, FlatLiteral> {
        if let Some(ref reason) = self.reason {
            Literal::str(reason.clone()).some()
        } else {
            Literal::none()
        }
    }
    pub fn set_reason(&self, reason: &str) -> Self {
        let mut new = self.clone();
        new.reason = Some(reason.to_string());
        new
    }
    pub fn set_header(&self, k: &str, v: &[u8]) -> Self {
        let mut new = self.clone();
        new.headers.set_header(k, v);
        new
    }
    pub fn headers(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.headers()
    }
    pub fn header_pairs(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.headers.header_pairs()
    }
    pub fn set_connection(&self, c: &Connection<FlatTyp, FlatLiteral>) -> Self {
        let mut new = self.clone();
        new.connection = c.clone();
        new
    }
    pub fn from_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.from_lit()
    }
    pub fn to_lit(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.to_lit()
    }
    pub fn from_to(&self) -> Literal<FlatTyp, FlatLiteral> {
        self.connection.from_to()
    }
    pub fn set_from(&self, from: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut res = self.clone();
        res.connection.from = from.clone();
        res
    }
    pub fn set_to(&self, to: &ID<FlatTyp, FlatLiteral>) -> Self {
        let mut res = self.clone();
        res.connection.to = to.clone();
        res
    }
}

pub struct VecSet<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    phantom: PhantomData<(FlatTyp, FlatLiteral)>,
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> VecSet<FlatTyp, FlatLiteral> {
    pub fn contains(l: &[Literal<FlatTyp, FlatLiteral>], x: &Literal<FlatTyp, FlatLiteral>) -> Literal<FlatTyp, FlatLiteral> {
        Literal::bool(l.iter().any(|y| x == y))
    }
    pub fn is_subset(x: &[Literal<FlatTyp, FlatLiteral>], y: &[Literal<FlatTyp, FlatLiteral>]) -> Literal<FlatTyp, FlatLiteral> {
        Literal::bool(x.iter().all(|ex| y.iter().any(|ey| ex == ey)))
    }
    pub fn is_disjoint(x: &[Literal<FlatTyp, FlatLiteral>], y: &[Literal<FlatTyp, FlatLiteral>]) -> Literal<FlatTyp, FlatLiteral> {
        Literal::bool(!x.iter().any(|ex| y.iter().any(|ey| ex == ey)))
    }
    pub fn difference(x: &[Literal<FlatTyp, FlatLiteral>], y: &[Literal<FlatTyp, FlatLiteral>]) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            x.to_owned()
                .into_iter()
                .filter(|ex| !y.iter().any(|ey| ex == ey))
                .collect(),
        )
    }
    pub fn intersection(x: &[Literal<FlatTyp, FlatLiteral>], y: &[Literal<FlatTyp, FlatLiteral>]) -> Literal<FlatTyp, FlatLiteral> {
        Literal::List(
            x.to_owned()
                .into_iter()
                .filter(|ex| y.iter().any(|ey| ex == ey))
                .collect(),
        )
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingData {
    //host description                
    host: labels::Label, 

    //service description
    service: labels::Label,                  
    port: Option<u16>,
    
    //TODO

    //authentification description
    //TODO

    //exported labels by the proxy, i.e, from the armour-compose file
    proposed_labels: labels::Labels,
    ips: BTreeSet<std::net::IpAddr>
}

//FIXME duplicated
impl OnboardingData {
    pub fn new(
        host: labels::Label,
        service: labels::Label,
        port: Option<u16>,
        proposed_labels: labels::Labels,
        ips: BTreeSet<std::net::IpAddr>
    ) -> Self {
        OnboardingData {
            host,
            service,
            port,
            proposed_labels,
            ips
        }
    }
    pub fn service(&self) -> labels::Label {
        self.service.clone()
    }
    pub fn host(&self) -> labels::Label {
        self.host.clone()
    }
    pub fn port(&self) -> Option<u16> {
        self.port.clone()
    }
    pub fn proposed_labels(&self) -> CPLiteral {
        Literal::List(
            self.proposed_labels.clone().into_iter()
                .map(|x| Literal::label(x) )
                .collect()
        )
    }
    pub fn has_proposed_label(&self, label: &labels::Label) -> bool {
        self.proposed_labels.iter().any(|x| label.matches_with(x))
    }
    pub fn find_proposed_label(&self, label: &labels::Label) -> Option<&labels::Label> {
        self.proposed_labels.iter().find(|x| label.matches_with(x))
    }
    pub fn has_ip(&self, ip: &std::net::IpAddr) -> bool {
        self.ips.iter().any(|x| x == ip)
    }
    pub fn service_lit(&self) -> CPLiteral {
        cpdplit!(Label(self.service.clone()))
    }
    pub fn host_lit(&self) -> CPLiteral {
        cpdplit!(Label(self.host.clone()))
    }
}

//TODO find a better structure 
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum OnboardingResult {
    Ok(CPID, (Policy, Policy)),//ingress policy, egress policy
    Err(String, Option<CPID>, Option<(Policy, Policy)>)//ingress policy, egress policy
}

impl OnboardingResult {
    pub fn new_ok(id: CPID, p: (Policy, Policy) ) -> Self {
        Self::Ok(id, p)
    }
    pub fn new_err(err: String, id: CPID, p : (Policy, Policy) ) -> Self {
        Self::Err(err, Some(id), Some(p))
    }
    pub fn new_err_str(err: String ) -> Self {
        Self::Err(err, None, None)
    }
    pub fn new_ok_lit(id: CPID, p: (Policy, Policy) ) -> CPLiteral {
        Literal::FlatLiteral(CPFlatLiteral::OnboardingResult(Box::new(
            Self::new_ok(id, p)
        )))
    }
    pub fn new_err_lit(err: String, id: CPID, p : (Policy, Policy) ) -> CPLiteral {
        Literal::FlatLiteral(CPFlatLiteral::OnboardingResult(Box::new(
            Self::new_err(err, id, p)
        )))
    }
    pub fn new_err_str_lit(err: String) -> CPLiteral {
        Literal::FlatLiteral(CPFlatLiteral::OnboardingResult(Box::new(
            Self::new_err_str(err)
        )))
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub pol: Box<DPPolicies>
}

use super::externals;

pub trait TFlatLiteral<FlatTyp:TFlatTyp> : std::fmt::Debug + PartialEq + Clone + fmt::Display +
 Unpin + std::marker::Send + Default + externals::TExternals<FlatTyp, Self> + std::marker::Sync 
 + TPrettyLit + TInterpret<FlatTyp, Self> + Serialize + TProtocol<FlatTyp, Self>
//+ From<bool> + From<Connection> +  From<Vec<u8>>  
//+ From<f64> + From<labels::Label> +  From<HttpRequest>
//+ From<HttpResponse> + From<ID<FlatTyp, FlatLiteral>  
//+ From<usize> + From<i64> + From<std::net::IpAddr>
//+ From<String> + From<()>
  {
    fn is_tuple(&self) -> bool; 
    fn typ(&self) -> FlatTyp;
    //fn none() -> Self; not for flatliteral
    //fn some(&self) -> Self;
    fn dest_some(&self) -> Option<Self> ;


    fn bool( b:bool ) -> Self;
    fn is_bool(&self) -> bool;
    fn get_bool(&self) -> bool;
    fn connection( c:Connection<FlatTyp, Self> ) -> Self;
    fn connection_from(from: &ID<FlatTyp, Self>, to: &ID<FlatTyp, Self>, number: i64) -> Self;

    fn data( v:Vec<u8> ) -> Self;
    fn is_data(&self) -> bool;
    fn get_data(&self) -> Vec<u8>;
    fn float( f:f64 ) -> Self;
    fn http_request( r:Box<HttpRequest<FlatTyp, Self>>) -> Self;
    fn http_response( r:Box<HttpResponse<FlatTyp, Self>>) -> Self ;
    fn id( i:ID<FlatTyp, Self> ) -> Self;
    fn int( i:i64) -> Self;
    fn ip_addr( i:std::net::IpAddr) -> Self;
    fn label( ls:labels::Label) -> Self;
    fn is_label(&self) -> bool;
    fn get_label<'a>(&'a self) -> &'a labels::Label;
    fn regex( pr:parser::PolicyRegex) -> Self;
    fn str( s:String ) -> Self;
    fn is_str(&self) -> bool;
    fn get_str<'a>(&'a self) -> &'a str;
    fn unit() -> Self;
    fn is_unit(&self) -> bool;



}

#[derive( PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum FlatLiteral {
    Bool(bool),
    Connection(Connection<FlatTyp, FlatLiteral>),
    Data(Vec<u8>),
    Float(f64),
    HttpRequest(Box<HttpRequest<FlatTyp, FlatLiteral>>),
    HttpResponse(Box<HttpResponse<FlatTyp, FlatLiteral>>),
    ID(ID<FlatTyp, FlatLiteral>),
    Int(i64),
    IpAddr(std::net::IpAddr),
    Label(labels::Label),
    Regex(parser::PolicyRegex),
    Str(String),
    Unit,
}
impl Default for FlatLiteral {
    fn default() -> Self { Self::Unit }
}
impl Default for CPFlatLiteral {
    fn default() -> Self {
        Self::DPFlatLiteral(FlatLiteral::default())
    }
}

pub type DPFlatLiteral = FlatLiteral;
impl TFlatLiteral<FlatTyp> for DPFlatLiteral {
    fn bool( b:bool ) -> Self {
        Self::Bool(b) 
    }
    
    fn is_bool(&self) -> bool {
        match self {
            Self::Bool(_) => true,
            _ => false
        }
    }

    fn get_bool(&self) -> bool {
        match self {
            Self::Bool(l) => l.clone(),
            _ => panic!() 
        }
    }

    fn connection( c:Connection<FlatTyp, Self> ) -> Self {
        Self::Connection(c)
    }
    
    fn connection_from(from: &ID<FlatTyp, Self>, to: &ID<FlatTyp, Self>, number: i64) -> Self {
        Self::Connection(Connection {
            from: from.clone(),
            to: to.clone(),
            number,
            phantom: PhantomData
        })
    }

    fn data( v:Vec<u8> ) -> Self { 
        Self::Data(v) 
    }

    fn is_data(&self) -> bool { 
        match self { 
            FlatLiteral::Data(_) => true, 
            _ => false 
        }
    }

    fn get_data(&self) -> Vec<u8> { 
        match self { 
            FlatLiteral::Data(d) => d.clone(),
            _ => panic!()
        }
    }

    fn float(f: f64) -> Self { 
        Self::Float(f) 
    }

    fn http_request(r: Box<HttpRequest<FlatTyp, Self>>) -> Self {
        Self::HttpRequest(r)
    }

    fn http_response(r: Box<HttpResponse<FlatTyp, Self>>) -> Self {
        Self::HttpResponse(r)
    }

    fn id(i: ID<FlatTyp, FlatLiteral> ) -> Self {
        Self::ID(i) 
    }

    fn int(i: i64) -> Self {
        Self::Int(i) 
    }

    fn ip_addr(i: std::net::IpAddr) -> Self {
        Self::IpAddr(i)
    }

    fn label(l: labels::Label) -> Self {
        Self::Label(l) 
    }

    fn is_label(&self) -> bool{
        match self {
            Self::Label(_) => true,
            _ => false
        }
    }

    fn get_label<'a>(&'a self) -> &'a labels::Label{
        match self {
            Self::Label(l) => l,
            _ => panic!() 
        }
    }

    fn regex(pr: parser::PolicyRegex) -> Self {
        Self::Regex(pr) 
    }

    fn str(s: String ) -> Self {
        Self::Str(s) 
    }

    fn is_str(&self) -> bool {
        match self {
            Self::Str(_) => true,
            _ => false
        }
    }

    fn get_str<'a>(&'a self) -> &'a str{
        match self {
            Self::Str(l) => l,
            _ => panic!() 
        }
    }

    fn unit() -> Self {
        FlatLiteral::Unit
    }

    fn is_unit(&self) -> bool { *self == FlatLiteral::Unit }

    fn is_tuple(&self) -> bool { false }

    fn typ(&self) -> FlatTyp {
        match self {
            FlatLiteral::Bool(_) => FlatTyp::Bool,
            FlatLiteral::Connection(_) => FlatTyp::Connection,
            FlatLiteral::Data(_) => FlatTyp::Data,
            FlatLiteral::Float(_) => FlatTyp::F64,
            FlatLiteral::HttpRequest(_) => FlatTyp::HttpRequest,
            FlatLiteral::HttpResponse(_) => FlatTyp::HttpResponse,
            FlatLiteral::ID(_) => FlatTyp::ID,
            FlatLiteral::Int(_) => FlatTyp::I64,
            FlatLiteral::IpAddr(_) => FlatTyp::IpAddr,
            FlatLiteral::Label(_) => FlatTyp::Label,
            FlatLiteral::Regex(_) => FlatTyp::Regex,
            FlatLiteral::Str(_) => FlatTyp::Str,
            FlatLiteral::Unit => FlatTyp::Unit,
        }
    }
    
    fn dest_some(&self) -> Option<Self> {
        None
    }
}


#[derive( PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Literal<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    FlatLiteral(FlatLiteral),
    List(Vec<Literal<FlatTyp, FlatLiteral>>),
    Tuple(Vec<Literal<FlatTyp, FlatLiteral>>),
    Phantom(PhantomData<FlatTyp>)
}

//impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> TFlatLiteral<FlatTyp> for Literal<FlatTyp, FlatLiteral> {
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Literal<FlatTyp, FlatLiteral> {
    pub fn bool( b:bool ) -> Self { Self::FlatLiteral(FlatLiteral::bool(b)) }
    pub fn is_bool(&self) -> bool{
        match self {
            Self::FlatLiteral(ref fl) => fl.is_bool(),
            _ => false 
        }
    }
    pub fn get_bool(&self) -> bool{
        match self {
            Self::FlatLiteral(fl) => fl.get_bool(),
            _ => panic!() 
        }
    }
    pub fn connection( c:Connection<FlatTyp, FlatLiteral> ) -> Self {
        Self::FlatLiteral(FlatLiteral::connection(c))
    }
    pub fn data( v:Vec<u8> ) -> Self { Self::FlatLiteral(FlatLiteral::data(v)) }
    pub fn is_data(&self) -> bool { 
        match self {
            Self::FlatLiteral(fl) => fl.is_data(),
            _ => false
        }
    }
    pub fn get_data(&self) -> Vec<u8> { 
        match self { 
            Self::FlatLiteral(fl) => fl.get_data(),
            _ => panic!()
        }
    }
    pub fn float( f:f64 ) -> Self { Self::FlatLiteral(FlatLiteral::float(f)) }
    pub fn http_request( r:Box<HttpRequest<FlatTyp, FlatLiteral>>) -> Self {
        Self::FlatLiteral(FlatLiteral::http_request(r))
    }
    pub fn http_response( r:Box<HttpResponse<FlatTyp, FlatLiteral>>) -> Self {
        Self::FlatLiteral(FlatLiteral::http_response(r))
    }
    pub fn id( i:ID<FlatTyp, FlatLiteral> ) -> Self {
        Self::FlatLiteral(FlatLiteral::id(i)) 
    }
    pub fn int( i:i64) -> Self { Self::FlatLiteral(FlatLiteral::int(i)) }
    pub fn ip_addr( i:std::net::IpAddr) -> Self {
        Self::FlatLiteral(FlatLiteral::ip_addr(i))
    }
    pub fn label( l:labels::Label) -> Self {
        Self::FlatLiteral(FlatLiteral::label(l))
    }
    pub fn regex( pr:parser::PolicyRegex) -> Self {
        Self::FlatLiteral(FlatLiteral::regex(pr)) }
    pub fn str( s:String ) -> Self {
        Self::FlatLiteral(FlatLiteral::str(s))
    }
    pub fn unit() -> Self {
        Literal::FlatLiteral(FlatLiteral::unit())
    }
    pub fn is_unit(&self) -> bool {
        match self {
            Self::FlatLiteral(fl) => fl.is_unit(),
            _ => false
        }
    }

    pub fn is_tuple(&self) -> bool {
        match self {
            Literal::Tuple(_) => true,
            _ => false,
        }
    }
    pub fn typ(&self) -> Typ<FlatTyp> {
        match self {
            Literal::FlatLiteral(fl) => Typ::FlatTyp(fl.typ()),
            Literal::List(l) => l.get(0).map(|t| t.typ()).unwrap_or(Typ::rreturn()),
            Literal::Tuple(l) => Typ::Tuple((*l).iter().map(|t: &Self| t.typ()).collect()),
            Literal::Phantom(_) => unreachable!()
        }
    }
    pub fn dest_some(&self) -> Option<Self> {
        match self {
            Literal::Tuple(v) => match v.as_slice() {
                [ref l] => Some(l.clone()),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn none() -> Self {
        Literal::Tuple(Vec::new())
    }
    pub fn some(&self) -> Self {
        Literal::Tuple(vec![self.clone()])
    }
    pub fn some2(f : &FlatLiteral) -> Self {
        Literal::FlatLiteral(f.clone()).some()
    }
}

pub type DPLiteral = Literal<FlatTyp, DPFlatLiteral>;




impl fmt::Display for FlatLiteral {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FlatLiteral::Bool(b) => write!(f, "{}", b),
            FlatLiteral::Connection(c) => write!(f, "{:?}", c),
            FlatLiteral::Data(d) => {
                if let Ok(s) = std::str::from_utf8(d) {
                    write!(f, r#"b"{}""#, s)
                } else {
                    write!(f, "{:x?}", d)
                }
            }
            FlatLiteral::Float(d) => {
                if 8 < d.abs().log10() as usize {
                    write!(f, "{:e}", d)
                } else if (d.trunc() - *d).abs() < std::f64::EPSILON {
                    write!(f, "{:.1}", d)
                } else {
                    write!(f, "{}", d)
                }
            }
            FlatLiteral::HttpRequest(r) => write!(f, "{:?}", r),
            FlatLiteral::HttpResponse(r) => write!(f, "{:?}", r),
            FlatLiteral::ID(id) => write!(f, "{:?}", id),
            FlatLiteral::Int(i) => write!(f, "{}", i),
            FlatLiteral::IpAddr(ip) => write!(f, "{}", ip),
            FlatLiteral::Label(label) => write!(f, "'{}'", label),
            FlatLiteral::Regex(r) => write!(f, "{:?}", r),
            FlatLiteral::Str(s) => write!(f, r#""{}""#, s),
            FlatLiteral::Unit => write!(f, "()"),
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> fmt::Display for Literal<FlatTyp, FlatLiteral> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::FlatLiteral(fl) => fmt::Display::fmt(fl, f),
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
            },
            Literal::Phantom(_) => unreachable!()
        }
    }
}


#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum CPFlatLiteral {
    DPFlatLiteral(DPFlatLiteral),
    OnboardingData(Box<OnboardingData>),
    OnboardingResult(Box<OnboardingResult>),
    Policy(Box<Policy>),

}

impl From<CPFlatLiteral> for DPFlatLiteral {
    fn from(cl: CPFlatLiteral) -> Self {
        match cl {
            CPFlatLiteral::DPFlatLiteral(l) => l,
            CPFlatLiteral::OnboardingData(_) => panic!("OnboardingData can not be converted to a DPFlatLiteral"),
            CPFlatLiteral::OnboardingResult(_) => panic!("OnboardingResult  can not be converted to a DPFlatLiteral"),
            CPFlatLiteral::Policy(_) => panic!("Policy can not be converted to a DPFlatLiteral"),
        }
    }
}

impl From<DPLiteral> for CPLiteral {
    fn from(lit: DPLiteral) -> Self {
        match lit {
            DPLiteral::FlatLiteral(fl) => Self::FlatLiteral(CPFlatLiteral::DPFlatLiteral(fl)),
            DPLiteral::List(lits) => Self::List(lits.into_iter().map(|l| Self::from(l)).collect()),
            DPLiteral::Tuple(lits) => Self::Tuple(lits.into_iter().map(|l| Self::from(l)).collect()),
            DPLiteral::Phantom(_) => Self::Phantom(PhantomData)
        }
    }
}
impl From<CPLiteral> for DPLiteral {
    fn from(lit: CPLiteral) -> DPLiteral {
        match lit {
            CPLiteral::FlatLiteral(fl) => Self::FlatLiteral(FlatLiteral::from(fl)),
            CPLiteral::List(lits) => Self::List(lits.into_iter().map(|l| Self::from(l)).collect()),
            CPLiteral::Tuple(lits) => Self::Tuple(lits.into_iter().map(|l| Self::from(l)).collect()),
            CPLiteral::Phantom(_) => Self::Phantom(PhantomData)
        }
    }
}

impl CPFlatLiteral {
    pub fn typ(&self) -> CPTyp {
        match self {
            CPFlatLiteral::OnboardingData(_) => CPTyp::onboarding_data(),
            CPFlatLiteral::OnboardingResult(_) => CPTyp::onboarding_result(),
            CPFlatLiteral::Policy(_) => CPTyp::policy(),
            dpft => CPTyp::FlatTyp(CPFlatTyp::DPFlatTyp(
                DPFlatLiteral::from(dpft.clone()).typ()
            )),
        }
    }
}

impl fmt::Display for CPFlatLiteral {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CPFlatLiteral::DPFlatLiteral(dpfl) => DPFlatLiteral::fmt(dpfl, f),
            CPFlatLiteral::OnboardingData(d) => write!(f, "{:?}", d),
            CPFlatLiteral::OnboardingResult(r) => write!(f, "{:?}", r),
            CPFlatLiteral::Policy(p) => write!(f, "{:?}", p),
        }
    }
}
impl TFlatLiteral<CPFlatTyp> for CPFlatLiteral {
    fn bool( b:bool ) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::bool(b))
    }

    fn is_bool(&self) -> bool{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Bool(_)) => true,
            _ => false
        }
    }

    fn get_bool(&self) -> bool{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Bool(l)) => l.clone(),
            _ => unreachable!() 
        }
    }

    fn connection( c:Connection<CPFlatTyp, Self> ) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::connection(c.into()))
    }

    fn connection_from(from: &ID<CPFlatTyp, Self>, to: &ID<CPFlatTyp, Self>, number: i64) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::connection_from(
            &DPID::from(from.clone()),
            &DPID::from(to.clone()),
            number,
        ))
    }
    
    fn data( v:Vec<u8> ) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::data(v))
    }

    fn is_data(&self) -> bool {
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Data(_)) => true,
            _ => false
        }
    }

    fn get_data(&self) -> Vec<u8> { 
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Data(d)) => d.clone(),
            _ => unreachable!()
        }
    }

    fn float(f: f64) -> Self { 
        Self::DPFlatLiteral(DPFlatLiteral::float(f)) 
    }

    fn http_request( r: Box<HttpRequest<CPFlatTyp, Self>>) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::http_request(Box::new((*r).into())))
    }

    fn http_response(r: Box<HttpResponse<CPFlatTyp, Self>>) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::http_response(Box::new((*r).into())))
    }

    fn id(i: ID<CPFlatTyp, Self>) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::id(i.into()))
    }

    fn int(i: i64) -> Self { 
        Self::DPFlatLiteral(DPFlatLiteral::int(i))
    }

    fn ip_addr(i: std::net::IpAddr) -> Self { 
        Self::DPFlatLiteral(DPFlatLiteral::ip_addr(i)) 
    }

    fn label(l: labels::Label) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::label(l)) 
    }

    fn is_label(&self) -> bool{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Label(_)) => true,
            _ => false
        }
    }

    fn get_label<'a>(&'a self) -> &'a labels::Label{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Label(l)) => l,
            _ => unreachable!() 
        }
    }
    fn regex(pr: parser::PolicyRegex) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::regex(pr)) 
    }

    fn str(s: String) -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::str(s)) 
    }

    fn is_str(&self) -> bool{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Str(_)) => true,
            _ => false
        }
    }

    fn get_str<'a>(&'a self) -> &'a str{
        match self {
            Self::DPFlatLiteral(DPFlatLiteral::Str(l)) => l,
            _ => unreachable!() 
        }
    }

    fn unit() -> Self {
        Self::DPFlatLiteral(DPFlatLiteral::Unit)
    }

    fn is_unit(&self) -> bool { 
        *self == Self::DPFlatLiteral(DPFlatLiteral::Unit)
    }

    fn is_tuple(&self) -> bool { false }

    fn typ(&self) -> CPFlatTyp {
        match self {
            CPFlatLiteral::DPFlatLiteral(dpfl) => CPFlatTyp::from(dpfl.typ()),
            CPFlatLiteral::OnboardingData(_) => CPFlatTyp::OnboardingData,
            CPFlatLiteral::OnboardingResult(_) => CPFlatTyp::OnboardingData,
            CPFlatLiteral::Policy(_) => CPFlatTyp::Policy,
        }
    }
    
    fn dest_some(&self) -> Option<Self> {
        None
    }
}



pub type CPLiteral = Literal<CPFlatTyp, CPFlatLiteral>;

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<bool> for Literal<FlatTyp, FlatLiteral> {
    fn from(b: bool) -> Self {
        Self::bool(b)
    }
}
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Connection<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(conn: Connection<FlatTyp, FlatLiteral>) -> Self {
        Literal::connection(conn)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&Connection<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(conn: &Connection<FlatTyp, FlatLiteral>) -> Self {
        Literal::Tuple(vec![
            (&conn.from).into(),
            (&conn.to).into(),
            conn.number.into(),
        ])
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Vec<u8>> for Literal<FlatTyp, FlatLiteral> {
    fn from(d: Vec<u8>) -> Self {
        Literal::data(d)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&[u8]> for Literal<FlatTyp, FlatLiteral> {
    fn from(d: &[u8]) -> Self {
        Literal::data(d.to_vec())
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<f64> for Literal<FlatTyp, FlatLiteral> {
    fn from(n: f64) -> Self {
        Literal::float(n)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<labels::Label> for Literal<FlatTyp, FlatLiteral> {
    fn from(l: labels::Label) -> Self {
        Literal::label(l)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&labels::Label> for Literal<FlatTyp, FlatLiteral> {
    fn from(l: &labels::Label) -> Self {
        l.clone().into()
    }
}

impl From<OnboardingData> for CPLiteral {
    fn from(data: OnboardingData) -> Self {
        Literal::FlatLiteral(
            CPFlatLiteral::OnboardingData(Box::new(data))
        )
    }
}
impl From<&OnboardingData> for CPLiteral {
    fn from(data: &OnboardingData) -> Self {
        Literal::FlatLiteral(
            CPFlatLiteral::OnboardingData(Box::new(data.clone()))
        )
    }
}
impl From<OnboardingResult> for CPLiteral {
    fn from(res: OnboardingResult) -> Self {
        Literal::FlatLiteral(
            CPFlatLiteral::OnboardingResult(Box::new(res))
        )
    }
}
impl From<&OnboardingResult> for CPLiteral {
    fn from(res: &OnboardingResult) -> Self {
        Literal::FlatLiteral(CPFlatLiteral::OnboardingResult(Box::new(res.clone())))
    }
}
impl From<Policy> for CPLiteral {
    fn from(pol: Policy) -> Self {
        Literal::FlatLiteral(
            CPFlatLiteral::Policy(Box::new(pol))
        )
    }
}
impl From<&Policy> for CPLiteral {
    fn from(pol: &Policy) -> Self {
        Literal::FlatLiteral(CPFlatLiteral::Policy(Box::new(pol.clone())))
    }
}
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<HttpRequest<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(r: HttpRequest<FlatTyp, FlatLiteral>) -> Self {
        Literal::http_request(Box::new(r))
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&HttpRequest<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(req: &HttpRequest<FlatTyp, FlatLiteral>) -> Self {
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

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<HttpResponse<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(r: HttpResponse<FlatTyp, FlatLiteral>) -> Self {
        Literal::http_response(Box::new(r))
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&HttpResponse<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(res: &HttpResponse<FlatTyp, FlatLiteral>) -> Self {
        Literal::Tuple(vec![
            res.version(),
            res.status(),
            res.header_pairs(),
            (&res.connection).into(),
        ])
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<ID<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(id: ID<FlatTyp, FlatLiteral>) -> Self {
        Literal::id(id)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&ID<FlatTyp, FlatLiteral>> for Literal<FlatTyp, FlatLiteral> {
    fn from(id: &ID<FlatTyp, FlatLiteral>) -> Self {
        Literal::Tuple(vec![id.hosts(), id.ips(), id.port_lit(), id.labels()])
    }
}

// impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<(std::net::SocketAddr, Option<&BTreeSet<labels::Label>>)> for Literal<FlatTyp, FlatLiteral> {
//     fn from(s: (std::net::SocketAddr, Option<&BTreeSet<labels::Label>>)) -> Self {
//         Literal::ID(s.into())
//     }
// }

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<usize> for Literal<FlatTyp, FlatLiteral> {
    fn from(n: usize) -> Self {
        Literal::int(n as i64)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<i64> for Literal<FlatTyp, FlatLiteral> {
    fn from(n: i64) -> Self {
        Literal::int(n)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<std::net::IpAddr> for Literal<FlatTyp, FlatLiteral> {
    fn from(ip: std::net::IpAddr) -> Self {
        Literal::ip_addr(ip)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&std::net::IpAddr> for Literal<FlatTyp, FlatLiteral> {
    fn from(ip: &std::net::IpAddr) -> Self {
        match ip {
            std::net::IpAddr::V4(ip) => {
                let [a, b, c, d] = ip.octets();
                #[allow(clippy::cast_lossless)]
                Literal::Tuple(vec![
                    Literal::int(a as i64),
                    Literal::int(b as i64),
                    Literal::int(c as i64),
                    Literal::int(d as i64),
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

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&str> for Literal<FlatTyp, FlatLiteral> {
    fn from(s: &str) -> Self {
        Literal::str(s.to_string())
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<String> for Literal<FlatTyp, FlatLiteral> {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<()> for Literal<FlatTyp, FlatLiteral> {
    fn from(_: ()) -> Self {
        Literal::unit()
    }
}

impl<T, FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Vec<T>> for Literal<FlatTyp, FlatLiteral>
where
    T: Into<Literal<FlatTyp, FlatLiteral>>,
{
    fn from(x: Vec<T>) -> Self {
        Literal::List(x.into_iter().map(|x| x.into()).collect())
    }
}

impl<T, FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<BTreeSet<T>> for Literal<FlatTyp, FlatLiteral>
where
    T: Into<Literal<FlatTyp, FlatLiteral>>,
{
    fn from(x: BTreeSet<T>) -> Self {
        Literal::List(x.into_iter().map(|x| x.into()).collect())
    }
}

impl<T, FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<Option<T>> for Literal<FlatTyp, FlatLiteral>
where
    T: Into<Literal<FlatTyp, FlatLiteral>>,
{
    fn from(x: Option<T>) -> Self {
        if let Some(v) = x {
            v.into().some()
        } else {
            Literal::none()
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<&labels::Match> for Literal<FlatTyp, FlatLiteral> {
    fn from(m: &labels::Match) -> Self {
        let v: Vec<(String, String)> = m.into();
        Literal::List(
            v.into_iter()
                .map(|(x, y)| Literal::Tuple(vec![Literal::str(x), Literal::str(y)]))
                .collect(),
        )
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> From<labels::Match> for Literal<FlatTyp, FlatLiteral> {
    fn from(m: labels::Match) -> Self {
        (&m).into()
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> std::convert::TryFrom<Literal<FlatTyp, FlatLiteral>> for bool {
    type Error = ();
    fn try_from(l: Literal<FlatTyp, FlatLiteral>) -> Result<bool, Self::Error> {
        if l.is_bool() {
            Ok(l.get_bool())
        } else {
            Err(())
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> std::convert::TryFrom<Literal<FlatTyp, FlatLiteral>> for () {
    type Error = ();
    fn try_from(l: Literal<FlatTyp, FlatLiteral>) -> Result<(), Self::Error> {
        if l.is_unit() {
            Ok(()) 
        } else {
            Err(())
        }
    }
}


