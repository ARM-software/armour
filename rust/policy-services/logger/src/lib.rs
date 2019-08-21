use chrono::{NaiveDateTime, Utc};
// use chrono_tz::GMT;
use petgraph::graphmap::DiGraphMap;
use policy_service::rpc::Literal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

mod dotgraph;

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
    fn from_literal(v: &[Literal]) -> Option<Self> {
        match v {
            [Literal::List(hosts), Literal::List(ips), Literal::Tuple(port)] => {
                let hosts: Option<Vec<String>> = hosts
                    .iter()
                    .map(|x| match x {
                        Literal::Str(host) => Some(host.clone()),
                        _ => None,
                    })
                    .collect();
                let ips: Option<Vec<std::net::IpAddr>> = ips
                    .iter()
                    .map(|x| Endpoint::ip_addr_from_literal(x))
                    .collect();
                let port = match port.as_slice() {
                    [Literal::Int(n)] => Some(*n as u16),
                    _ => None,
                };
                Some(Endpoint {
                    hosts: hosts?,
                    ips: ips?,
                    port,
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Connection {
    date: chrono::NaiveDateTime,
    from: Endpoint,
    to: Endpoint,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Connections(pub Vec<Connection>);

impl Connections {
    fn to_graph(&self) -> ConnectionGraph {
        let mut g = ConnectionGraph::new();
        for connection in self.0.iter() {
            if let (Some(from), Some(to), Some(port)) = (
                connection.from.hosts.get(0),
                connection.to.hosts.get(0),
                connection.to.port,
            ) {
                if let Some(edge) = g.graph.edge_weight_mut(from.as_str(), to.as_str()) {
                    edge.insert(port);
                } else {
                    let mut edge = BTreeSet::new();
                    edge.insert(port);
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
    pub fn export_pdf<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> std::io::Result<()> {
        self.to_graph().export_pdf(path)
    }
}

pub struct ConnectionGraph<'a> {
    graph: DiGraphMap<&'a str, BTreeSet<u16>>,
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
                    "rectangle",
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
        for (from, to, ports) in self.graph.all_edges() {
            let source = node_ids.binary_search(&from).expect("missing <from>");
            let target = node_ids.binary_search(&to).expect("missing <to>");
            let label = ports
                .iter()
                .map(|port| port.to_string())
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
    fn export_pdf<P: AsRef<std::ffi::OsStr>>(&self, filename: P) -> std::io::Result<()> {
        let path = std::path::Path::new(&filename).with_extension("dot");
        let mut file = std::fs::File::create(&path)?;
        dot::render(&self.dotgraph(), &mut file)?;
        log::debug!("generated graph: {}", path.display());
        // requires graphviz with cairo, e.g.
        // brew install graphviz --with-pango
        if cfg!(target_family = "unix") {
            let pdf = std::path::Path::new(&filename).with_extension("pdf");
            std::process::Command::new("dot")
                .arg("-Tpdf")
                .arg("-Gdpi=80")
                .arg("-Earrowsize=0.6")
                .arg(path.to_str().unwrap())
                .arg("-o")
                .arg(pdf.to_str().unwrap())
                .spawn()
                .map(|_| log::debug!("converting to PDF"))
        } else {
            Ok(())
        }
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
                [Literal::Str(message_date), Literal::Tuple(from), Literal::Tuple(to)]
                    if from.len() == 3 && to.len() == 3 =>
                {
                    if let (Some(from), Some(to)) =
                        (Endpoint::from_literal(from), Endpoint::from_literal(to))
                    {
                        let date =
                            NaiveDateTime::parse_from_str(message_date, "%a, %e %b %Y %T %Z")
                                .unwrap_or_else(|_| Utc::now().naive_utc());
                        let connection = Connection { date, from, to };
                        log::debug!("{:?}", connection);
                        let mut v = self.0.lock().unwrap();
                        v.0.push(connection)
                    } else {
                        log::warn!("incomplete ID");
                        self.log(name, args)
                    }
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
