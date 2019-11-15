use super::graph::{ConnectionEdge, ConnectionGraph};
use chrono::{NaiveDateTime, TimeZone, Utc};
use chrono_tz::GMT;
use policy_service::rpc::Literal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use v_htmlescape::escape;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Endpoint {
    hosts: Vec<String>,
    pub ips: Vec<std::net::IpAddr>,
    port: Option<u16>,
}

impl Endpoint {
    fn ip_addr_from_literal(lit: &Literal) -> Option<std::net::IpAddr> {
        match lit {
            Literal::Tuple(ip) => match ip.as_slice() {
                [Literal::Int(a), Literal::Int(b), Literal::Int(c), Literal::Int(d)] => {
                    Some(std::net::IpAddr::V4(std::net::Ipv4Addr::from([
                        *a as u8, *b as u8, *c as u8, *d as u8,
                    ])))
                }
                _ => None,
            },
            _ => None,
        }
    }
    fn from_literal(v: &[Literal], service: bool) -> Option<Self> {
        match v {
            [Literal::List(hosts), Literal::List(ips), Literal::Tuple(port)] => {
                let (port, suffix) = match port.as_slice() {
                    [Literal::Int(n)] => (
                        Some(*n as u16),
                        if service {
                            format!(":{}", n)
                        } else {
                            String::new()
                        },
                    ),
                    _ => (None, String::new()),
                };
                let hosts: Option<Vec<String>> = hosts
                    .iter()
                    .map(|x| match x {
                        Literal::Str(host) => Some(format!("{}{}", host, suffix)),
                        _ => None,
                    })
                    .collect();
                let ips: Option<Vec<std::net::IpAddr>> = ips
                    .iter()
                    .map(|x| Endpoint::ip_addr_from_literal(x))
                    .collect();
                Some(Endpoint {
                    hosts: hosts?,
                    ips: ips?,
                    port,
                })
            }
            _ => None,
        }
    }
    pub fn from(v: &[Literal]) -> Option<Self> {
        Endpoint::from_literal(v, false)
    }
    pub fn to(v: &[Literal]) -> Option<Self> {
        Endpoint::from_literal(v, true)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ConnectionType {
    TCP,
    REST { method: String, path: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Info {
    typ: ConnectionType,
    timestamp: chrono::NaiveDateTime,
    sent: usize,
    received: usize,
}

impl Info {
    pub fn rest(date: &str, method: &str, path: &str) -> Info {
        let timestamp = NaiveDateTime::parse_from_str(date, "%a, %e %b %Y %T %Z")
            .unwrap_or_else(|_| Utc::now().naive_utc());
        Info {
            typ: ConnectionType::REST {
                method: method.to_string(),
                path: path.to_string(),
            },
            timestamp,
            sent: 0,
            received: 0,
        }
    }
    pub fn tcp() -> Info {
        Info {
            typ: ConnectionType::TCP,
            timestamp: Utc::now().naive_utc(),
            sent: 0,
            received: 0,
        }
    }
    pub fn label(&self) -> String {
        match &self.typ {
            ConnectionType::TCP => "TCP".to_string(),
            ConnectionType::REST { method, path } => format!("{} {}", method, path),
        }
    }
    pub fn timestamp(&self) -> chrono::NaiveDateTime {
        self.timestamp
    }
    pub fn sent(&self) -> usize {
        self.sent
    }
    pub fn received(&self) -> usize {
        self.received
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Connection {
    info: Info,
    from: Endpoint,
    to: Endpoint,
}

impl Connection {
    pub fn new(info: Info, from: Endpoint, to: Endpoint) -> Connection {
        Connection { info, from, to }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Connections(BTreeMap<u64, Connection>, u64);

impl Connections {
    pub fn clear(&mut self) {
        self.0.clear();
        self.1 = 0
    }
    pub fn add_connection(&mut self, number: u64, connection: Connection) {
        // if a connection number is too low then the proxy must have been restarted
        // and so we wipe all the connection records
        if number <= self.1 && self.1 != 0 {
            log::warn!("clearing connection records");
            self.0.clear()
        };
        self.0.insert(number, connection);
        self.1 = number
    }
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(&self.0).unwrap()
    }
    pub fn to_yaml_summary(&self) -> String {
        serde_yaml::to_string(&self.to_summary()).unwrap()
    }
    pub fn set_sent(&mut self, number: u64, size: usize) {
        if let Some(connection) = self.0.get_mut(&number) {
            connection.info.sent = size
        } else {
            log::warn!("could not find connection [{}]", number)
        }
    }
    pub fn set_received(&mut self, number: u64, size: usize) {
        if let Some(connection) = self.0.get_mut(&number) {
            connection.info.received = size
        } else {
            log::warn!("could not find connection [{}]", number)
        }
    }
    fn to_graph(&self, services: bool) -> ConnectionGraph {
        let mut g = ConnectionGraph::new();
        for (_number, connection) in self.0.iter() {
            if let (Some(from), Some(to)) =
                (connection.from.hosts.get(0), connection.to.hosts.get(0))
            {
                let from = from.as_str();
                let (to, port) = if services {
                    (to.as_str(), None)
                } else {
                    (
                        to.as_str()
                            .trim_end_matches(char::is_numeric)
                            .trim_end_matches(':'),
                        connection.to.port,
                    )
                };
                if let Some(edge) = g.graph.edge_weight_mut(from, to) {
                    edge.update_with_info(&connection.info)
                } else {
                    g.graph
                        .add_edge(from, to, ConnectionEdge::from_info(&connection.info));
                }
                let received = connection.info.received();
                if received != 0 {
                    if let Some(edge) = g.graph.edge_weight_mut(to, from) {
                        edge.update_with_received(received)
                    } else {
                        g.graph
                            .add_edge(to, from, ConnectionEdge::from_received(received));
                    }
                }
                g.update_endpoint_meta(from, &connection.from, None);
                g.update_endpoint_meta(to, &connection.to, port)
            } else {
                log::debug!("incomplete connection: {:?}", connection)
            }
        }
        g
    }
    pub fn export_pdf<P: AsRef<std::ffi::OsStr>>(
        &self,
        path: P,
        service: bool,
        wait: bool,
    ) -> std::io::Result<()> {
        self.to_graph(service).export_pdf(path, wait)
    }
    pub fn export_svg<P: AsRef<std::ffi::OsStr>>(
        &self,
        path: P,
        service: bool,
        wait: bool,
    ) -> std::io::Result<()> {
        self.to_graph(service).export_svg(path, wait)
    }
    pub fn to_summary(&self) -> SummaryMap {
        let mut summary = SummaryMap::default();
        for (_number, connection) in self.0.iter() {
            if let (Some(from), Some(to)) =
                (connection.from.hosts.get(0), connection.to.hosts.get(0))
            {
                summary.add(from, to, &connection.info)
            } else {
                log::debug!("incomplete connection: {:?}", connection)
            }
        }
        summary
    }
    pub fn html_table(&self) -> String {
        self.to_summary().html_table()
    }
}

#[derive(Deserialize, Serialize)]
pub struct Summary {
    first: chrono::NaiveDateTime,
    last: chrono::NaiveDateTime,
    methods: BTreeMap<String, usize>,
    sent: usize,
    received: usize,
}

impl Summary {
    fn methods(&self) -> String {
        self.methods
            .iter()
            .map(|(method, count)| format!("{} ({})", method, count))
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct SummaryMap(BTreeMap<String, BTreeMap<String, Summary>>);

impl SummaryMap {
    fn add(&mut self, from: &str, to: &str, info: &Info) {
        if let Some(to_entry) = self.0.get_mut(to) {
            if let Some(from_to) = to_entry.get_mut(from) {
                let label = info.label();
                if let Some(count) = from_to.methods.get_mut(&label) {
                    *count += 1
                } else {
                    from_to.methods.insert(label, 1);
                }
                let timestamp = info.timestamp();
                if timestamp < from_to.first {
                    from_to.first = timestamp
                }
                if timestamp > from_to.last {
                    from_to.last = timestamp
                }
                from_to.sent += info.sent();
                from_to.received += info.received()
            } else {
                let mut methods = BTreeMap::new();
                methods.insert(info.label(), 1);
                let timestamp = info.timestamp();
                let from_to = Summary {
                    first: timestamp,
                    last: timestamp,
                    methods,
                    sent: info.sent(),
                    received: info.received(),
                };
                to_entry.insert(from.to_string(), from_to);
            }
        } else {
            self.0.insert(to.to_string(), BTreeMap::new());
            self.add(from, to, info)
        }
    }
    fn html_table(&self) -> String {
        let mut s = String::from(r#"<table border=1 frame=hsides rules=rows style="width:100%"><tr><th>service</th><th>client</th><th>first</th><th>last</th><th>methods</th><th>sent</th><th>received</th></tr>"#);
        for (to, from) in self.0.iter() {
            let len = from.len();
            let mut iter = from.iter();
            if let Some((from, summary)) = iter.next() {
                let rowspan = if 1 < len {
                    format!(r#" rowspan="{}" style="vertical-align: top;""#, len)
                } else {
                    "".to_string()
                };
                s.push_str(&format!(
                    r#"<tr><td{}>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                    rowspan,
                    escape(to),
                    escape(from),
                    GMT.from_utc_datetime(&summary.first)
                        .format("%a, %e %b %Y %T %Z"),
                    GMT.from_utc_datetime(&summary.last)
                        .format("%a, %e %b %Y %T %Z"),
                    escape(&summary.methods()),
                    summary.sent,
                    summary.received,
                ))
            }
            for (from, summary) in iter {
                s.push_str(&format!(
                    r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                    escape(from),
                    GMT.from_utc_datetime(&summary.first)
                        .format("%a, %e %b %Y %T %Z"),
                    GMT.from_utc_datetime(&summary.last)
                        .format("%a, %e %b %Y %T %Z"),
                    escape(&summary.methods()),
                    summary.sent,
                    summary.received,
                ))
            }
        }
        s.push_str("</table>");
        s
    }
}
