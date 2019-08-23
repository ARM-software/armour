use chrono::{NaiveDateTime, TimeZone, Utc};
use chrono_tz::GMT;
use petgraph::graphmap::DiGraphMap;
use policy_service::rpc::Literal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use v_htmlescape::escape;

mod dotgraph;
pub mod web;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Endpoint {
    hosts: Vec<String>,
    ips: Vec<std::net::IpAddr>,
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
    fn from(v: &[Literal]) -> Option<Self> {
        Endpoint::from_literal(v, false)
    }
    fn to(v: &[Literal]) -> Option<Self> {
        Endpoint::from_literal(v, true)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Info {
    TCP {
        date: chrono::NaiveDateTime,
    },
    REST {
        date: chrono::NaiveDateTime,
        method: String,
        path: String,
    },
}

impl Info {
    fn label(&self) -> String {
        match self {
            Info::TCP { .. } => "TCP".to_string(),
            Info::REST { method, path, .. } => format!("{} {}", method, path),
        }
    }
    fn date(&self) -> chrono::NaiveDateTime {
        match self {
            Info::TCP { date } => *date,
            Info::REST { date, .. } => *date,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Connection {
    info: Info,
    from: Endpoint,
    to: Endpoint,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Connections(pub Vec<Connection>);

impl Connections {
    fn to_graph(&self) -> ConnectionGraph {
        let mut g = ConnectionGraph::new();
        for connection in self.0.iter() {
            if let (Some(from), Some(to)) =
                (connection.from.hosts.get(0), connection.to.hosts.get(0))
            {
                let label = connection.info.label();
                if let Some(edge) = g.graph.edge_weight_mut(from.as_str(), to.as_str()) {
                    if let Some(count) = edge.get_mut(&label) {
                        *count += 1
                    } else {
                        edge.insert(label, 1);
                    }
                } else {
                    let mut edge = BTreeMap::new();
                    edge.insert(label, 1);
                    g.graph.add_edge(from, to, edge);
                }
                g.add_endpoint_ips(from.as_str(), &connection.from);
                g.add_endpoint_ips(to.as_str(), &connection.to);
            } else {
                log::debug!("incomplete connection: {:?}", connection)
            }
        }
        g
    }
    pub fn export_pdf<P: AsRef<std::ffi::OsStr>>(
        &self,
        path: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.to_graph().export_pdf(path, wait)
    }
    pub fn export_svg<P: AsRef<std::ffi::OsStr>>(
        &self,
        path: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.to_graph().export_svg(path, wait)
    }
    pub fn to_summary(&self) -> SummaryMap {
        let mut summary = SummaryMap::default();
        for connection in self.0.iter() {
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
            if let Some(from_entry) = to_entry.get_mut(from) {
                let label = info.label();
                if let Some(count) = from_entry.methods.get_mut(&label) {
                    *count += 1
                } else {
                    from_entry.methods.insert(label, 1);
                }
                let date = info.date();
                if date < from_entry.first {
                    from_entry.first = date
                }
                if date > from_entry.last {
                    from_entry.last = date
                }
            } else {
                let mut methods = BTreeMap::new();
                methods.insert(info.label(), 1);
                let date = info.date();
                let from_entry = Summary {
                    first: date,
                    last: date,
                    methods,
                };
                to_entry.insert(from.to_string(), from_entry);
            }
        } else {
            self.0.insert(to.to_string(), BTreeMap::new());
            self.add(from, to, info)
        }
    }
    fn html_table(&self) -> String {
        let mut s = String::from(r#"<table style="width:100%"><tr><th>service</th><th>client</th><th>first</th><th>last</th><th>methods</th></tr>"#);
        for (to, from) in self.0.iter() {
            let len = from.len();
            let mut iter = from.iter();
            if let Some((from, summary)) = iter.next() {
                let rowspan = if 1 < len {
                    format!(r#" rowspan="{}""#, len)
                } else {
                    "".to_string()
                };
                s.push_str(&format!(
                    r#"<tr><td{}>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                    rowspan,
                    escape(to),
                    escape(from),
                    GMT.from_utc_datetime(&summary.first)
                        .format("%a, %e %b %Y %T %Z"),
                    GMT.from_utc_datetime(&summary.last)
                        .format("%a, %e %b %Y %T %Z"),
                    escape(&summary.methods())
                ))
            }
            for (from, summary) in iter {
                s.push_str(&format!(
                    r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                    escape(from),
                    GMT.from_utc_datetime(&summary.first)
                        .format("%a, %e %b %Y %T %Z"),
                    GMT.from_utc_datetime(&summary.last)
                        .format("%a, %e %b %Y %T %Z"),
                    escape(&summary.methods())
                ))
            }
        }
        s.push_str("</table>");
        s
    }
}

pub struct ConnectionGraph<'a> {
    graph: DiGraphMap<&'a str, BTreeMap<String, usize>>,
    node_ips: BTreeMap<&'a str, BTreeSet<std::net::IpAddr>>,
}

impl<'a> ConnectionGraph<'a> {
    fn new() -> Self {
        ConnectionGraph {
            graph: DiGraphMap::new(),
            node_ips: BTreeMap::new(),
        }
    }
    fn add_endpoint_ips(&mut self, name: &'a str, e: &Endpoint) {
        if let Some(ips) = self.node_ips.get_mut(name) {
            ips.extend(e.ips.iter())
        } else {
            let ips = e
                .ips
                .iter()
                .cloned()
                .collect::<BTreeSet<std::net::IpAddr>>();
            self.node_ips.insert(name, ips);
        }
    }
    fn ips(&self, name: &str) -> String {
        if let Some(ips) = self.node_ips.get(name) {
            ips.iter()
                .map(|ip| ip.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        } else {
            "".to_string()
        }
    }
    fn scc_colouring(&'a self) -> BTreeMap<&'a str, String> {
        let scc = petgraph::algo::tarjan_scc(&self.graph);
        let mut colour_mapping: BTreeMap<&str, String> = BTreeMap::new();
        let short = scc.len() < 12;
        for (group, colour) in scc.into_iter().zip(dotgraph::ColourIterator::new(short)) {
            for i in group {
                colour_mapping.insert(i, colour.to_string());
            }
        }
        colour_mapping
    }
    fn dotgraph(&self) -> dotgraph::DotGraph {
        let colour = self.scc_colouring();
        let mut nodes: Vec<dotgraph::Node> = self
            .graph
            .nodes()
            .map(|v| {
                dotgraph::Node::new(
                    v,
                    self.ips(v),
                    "oval",
                    colour
                        .get(v)
                        .cloned()
                        .unwrap_or_else(|| "black".to_string()),
                    true,
                )
            })
            .collect();
        nodes.sort();
        let node_ids: Vec<&str> = nodes.iter().map(|x| x.label).collect();
        let mut edges = Vec::new();
        for (from, to, edge) in self.graph.all_edges() {
            let source = node_ids.binary_search(&from).expect("missing <from>");
            let target = node_ids.binary_search(&to).expect("missing <to>");
            let label = edge
                .iter()
                .map(|(label, count)| format!("{} ({})", label, count))
                .collect::<Vec<String>>()
                .join(", ");
            edges.push((source, target, label))
        }
        dotgraph::DotGraph {
            name: "connections",
            nodes,
            edges,
            edge_colour: "grey46",
            node_label_size: 12,
            caption_label_size: 10,
            edge_label_size: 9,
            font: "Franklin Gothic Medium",
        }
    }
    fn export<P: AsRef<std::ffi::OsStr>>(
        &self,
        filename: P,
        extension: &str,
        arg: &str,
        wait: bool,
    ) -> std::io::Result<()> {
        let path = std::path::Path::new(&filename).with_extension("dot");
        let mut file = std::fs::File::create(&path)?;
        dot::render(&self.dotgraph(), &mut file)?;
        log::debug!("generated graph: {}", path.display());
        // requires graphviz with cairo, e.g.
        // brew install graphviz --with-pango
        if cfg!(target_family = "unix") {
            let pdf = std::path::Path::new(&filename).with_extension(extension);
            let mut command = std::process::Command::new("dot");
            command
                .arg(arg)
                .arg("-Gdpi=80")
                .arg("-Earrowsize=0.6")
                .arg(path.to_str().unwrap())
                .arg("-o")
                .arg(pdf.to_str().unwrap());
            let res = if wait {
                command
                    .output()
                    .map(|_| log::info!("{} exported", extension))
            } else {
                command
                    .spawn()
                    .map(|_| log::info!("exporting {}", extension))
            };
            res.map_err(|e| {
                log::warn!("{} export failed: {}", extension, e);
                e
            })
        } else {
            Ok(())
        }
    }
    fn export_pdf<P: AsRef<std::ffi::OsStr>>(
        &self,
        filename: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.export(filename, "pdf", "-Tpdf", wait)
    }
    fn export_svg<P: AsRef<std::ffi::OsStr>>(
        &self,
        filename: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.export(filename, "svg", "-Tsvg", wait)
    }
}

pub struct LoggerService(pub Arc<Mutex<Connections>>);

impl policy_service::rpc::Dispatcher for LoggerService {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, capnp::Error> {
        match name {
            "log" => {
                self.log(name, args);
                Ok(Literal::Unit)
            }
            "connection" => match args {
                [Literal::Str(message_date), Literal::Str(method), Literal::Str(path), Literal::Tuple(from), Literal::Tuple(to)]
                    if from.len() == 3 && to.len() == 3 =>
                {
                    if let (Some(from), Some(to)) = (Endpoint::from(from), Endpoint::to(to)) {
                        let date =
                            NaiveDateTime::parse_from_str(message_date, "%a, %e %b %Y %T %Z")
                                .unwrap_or_else(|_| Utc::now().naive_utc());
                        let connection = Connection {
                            info: Info::REST {
                                date,
                                method: method.to_string(),
                                path: path.to_string(),
                            },
                            from,
                            to,
                        };
                        log::debug!("{:?}", connection);
                        let mut v = self.0.lock().unwrap();
                        v.0.push(connection)
                    } else {
                        log::warn!("incomplete ID");
                        self.log(name, args)
                    }
                    log::info!("logged REST connection");
                    Ok(Literal::Unit)
                }
                _ => Err(capnp::Error::failed(
                    "connection: bad arguments".to_string(),
                )),
            },
            "tcp_connection" => match args {
                [Literal::Tuple(from), Literal::Tuple(to)] if from.len() == 3 && to.len() == 3 => {
                    if let (Some(from), Some(to)) = (Endpoint::from(from), Endpoint::to(to)) {
                        let date = Utc::now().naive_utc();
                        let connection = Connection {
                            info: Info::TCP { date },
                            from,
                            to,
                        };
                        log::debug!("{:?}", connection);
                        let mut v = self.0.lock().unwrap();
                        v.0.push(connection)
                    } else {
                        log::warn!("incomplete ID");
                        self.log(name, args)
                    }
                    log::info!("logged TCP connection");
                    Ok(Literal::Unit)
                }
                _ => Err(capnp::Error::failed(
                    "connection: bad arguments".to_string(),
                )),
            },
            _ => Err(capnp::Error::unimplemented(name.to_string())),
        }
    }
}
