use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use url;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Policy {
    Accept,
    Forward,
    Reject,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
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
    pub fn method(&self) -> String {
        // TODO: fmt for Method
        format!("{:?}", self.method)
    }
    pub fn version(&self) -> String {
        // TODO: fmt for Version
        format!("{:?}", self.version)
    }
    pub fn path(&self) -> String {
        self.path.to_string()
    }
    pub fn set_path(&self, s: &str) -> HttpRequest {
        let mut new = self.clone();
        new.path = s.to_string();
        new
    }
    pub fn split_path(&self) -> Vec<String> {
        self.path.trim_matches('/').split('/').map(|s| s.to_string()).collect()
    }
    pub fn query(&self) -> String {
        self.query.to_string()
    }
    pub fn set_query(&self, s: &str) -> HttpRequest {
        let mut new = self.clone();
        new.query = s.to_string();
        new
    }
    pub fn query_pairs(&self) -> Vec<(String, String)> {
        if let Ok(url) = url::Url::parse(&format!("http://x/?{}", self.query)) {
            url.query_pairs().into_owned().collect()
        } else {
            Vec::new()
        }
    }
    pub fn header(&self, s: &str) -> Vec<Vec<u8>> {
        self.headers.get(s).unwrap_or(&Vec::new()).to_vec()
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
    pub fn headers(&self) -> Vec<String> {
        self.headers.keys().cloned().collect()
    }
    pub fn header_pairs(&self) -> Vec<(String, Vec<u8>)> {
        let mut pairs = Vec::new();
        for (k, vs) in self.headers.iter() {
            for v in vs {
                pairs.push((k.clone(), v.clone()))
            }
        }
        pairs
    }
}

impl From<(&str, &str, &str, &str, Vec<(&str, &[u8])>)> for HttpRequest {
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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    DataLiteral(Vec<u8>),
    StringLiteral(String),
    PolicyLiteral(Policy),
    List(Vec<Literal>),
    Tuple(Vec<Literal>),
    HttpRequestLiteral(HttpRequest),
    Unit,
}

impl Literal {
    fn is_tuple(&self) -> bool {
        match self {
            Literal::Tuple(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::IntLiteral(i) => write!(f, "{}", i),
            Literal::FloatLiteral(d) => {
                if 8 < d.abs().log10() as usize {
                    write!(f, "{:e}", d)
                } else if d.trunc() == *d {
                    write!(f, "{:.1}", d)
                } else {
                    write!(f, "{}", d)
                }
            }
            Literal::BoolLiteral(b) => write!(f, "{}", b),
            Literal::DataLiteral(d) => write!(f, "{:x?}", d),
            Literal::StringLiteral(s) => write!(f, r#""{}""#, s),
            Literal::PolicyLiteral(p) => write!(f, "{:?}", p),
            Literal::List(lits) | Literal::Tuple(lits) => {
                let s = lits
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                if self.is_tuple() {
                    write!(f, "({})", s)
                } else {
                    write!(f, "[{}]", s)
                }
            }
            Literal::HttpRequestLiteral(r) => write!(f, "{:?}", r),
            Literal::Unit => write!(f, "()"),
        }
    }
}
