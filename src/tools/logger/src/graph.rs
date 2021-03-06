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

use super::{connections, dotgraph};
use petgraph::graphmap::DiGraphMap;
use std::collections::{BTreeMap, BTreeSet};

pub struct ConnectionEdge {
    typs: BTreeMap<String, usize>,
    bytes: usize,
}

impl ConnectionEdge {
    pub fn from_info(info: &connections::Info) -> ConnectionEdge {
        let mut typs = BTreeMap::new();
        typs.insert(info.label(), 1);
        ConnectionEdge {
            typs,
            bytes: info.sent(),
        }
    }
    pub fn from_received(bytes: usize) -> ConnectionEdge {
        ConnectionEdge {
            typs: BTreeMap::new(),
            bytes
        }
    }
    pub fn update_with_info(&mut self, info: &connections::Info) {
        let label = info.label();
        if let Some(count) = self.typs.get_mut(&label) {
            *count += 1
        } else {
            self.typs.insert(label, 1);
        }
        self.bytes += info.sent()
    }
    pub fn update_with_received(&mut self, bytes: usize) {
        self.bytes += bytes
    }
    fn label(&self) -> String {
        let tys = self
            .typs
            .iter()
            .map(|(label, count)| format!("{} ({})", label, count))
            .collect::<Vec<String>>()
            .join(", ");
        if self.bytes == 0 {
            tys
        } else if tys == "" {
            format!("{} bytes", self.bytes)
        } else {
            format!("{}; {} bytes", tys, self.bytes)
        }
    }
}

pub struct NodeMeta {
    ips: BTreeSet<std::net::IpAddr>,
    ports: BTreeSet<u16>,
}

impl NodeMeta {
    fn new(ips: BTreeSet<std::net::IpAddr>, port: Option<u16>) -> Self {
        let mut ports = BTreeSet::new();
        if let Some(port) = port {
            ports.insert(port);
        }
        NodeMeta { ips, ports }
    }
}

pub struct ConnectionGraph<'a> {
    pub graph: DiGraphMap<&'a str, ConnectionEdge>,
    meta: BTreeMap<&'a str, NodeMeta>,
}

impl<'a> ConnectionGraph<'a> {
    pub fn new() -> Self {
        ConnectionGraph {
            graph: DiGraphMap::new(),
            meta: BTreeMap::new(),
        }
    }
    pub fn update_endpoint_meta(
        &mut self,
        name: &'a str,
        e: &connections::Endpoint,
        port: Option<u16>,
    ) {
        if let Some(meta) = self.meta.get_mut(name) {
            meta.ips.extend(e.ips.iter());
            if let Some(port) = port {
                meta.ports.insert(port);
            }
        } else {
            let ips = e
                .ips
                .iter()
                .cloned()
                .collect::<BTreeSet<std::net::IpAddr>>();
            self.meta.insert(name, NodeMeta::new(ips, port));
        }
    }
    fn subtitle(&self, name: &str) -> String {
        if let Some(meta) = self.meta.get(name) {
            let ips = meta
                .ips
                .iter()
                .map(|ip| ip.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            let ports = meta
                .ports
                .iter()
                .map(|port| format!(":{}", port))
                .collect::<Vec<String>>()
                .join(", ");
            if ports == "" {
                ips
            } else if ips == "" {
                ports
            } else {
                format!("{}; {}", ips, ports)
            }
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
                    self.subtitle(v),
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
            let label = edge.label();
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
    fn export_dot<P: AsRef<std::ffi::OsStr>>(&self, filename: P) -> std::io::Result<()> {
        let path = std::path::Path::new(&filename).with_extension("dot");
        let mut file = std::fs::File::create(&path)?;
        dot::render(&self.dotgraph(), &mut file)?;
        log::debug!("generated graph: {}", path.display());
        Ok(())
    }
    pub fn export_pdf<P: AsRef<std::ffi::OsStr>>(
        &self,
        filename: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.export_dot(&filename)?;
        // requires graphviz with cairo, e.g.
        // brew install graphviz --with-pango
        if cfg!(target_family = "unix") {
            let path = std::path::Path::new(&filename).with_extension("dot");
            let pdf = std::path::Path::new(&filename).with_extension("pdf");
            let mut command = std::process::Command::new("dot");
            command
                .arg("Tpdf")
                .arg("-Gdpi=80")
                .arg("-Earrowsize=0.6")
                .arg(path.to_str().unwrap())
                .arg("-o")
                .arg(pdf.to_str().unwrap());
            let res = if wait {
                command.output().map(|_| log::info!("PDF exported"))
            } else {
                command.spawn().map(|_| log::info!("exporting PDF"))
            };
            res.map_err(|e| {
                log::warn!("PDF export failed: {}", e);
                e
            })
        } else {
            Ok(())
        }
    }
    pub fn export_svg<P: AsRef<std::ffi::OsStr>>(
        &self,
        filename: P,
        wait: bool,
    ) -> std::io::Result<()> {
        self.export_dot(&filename)?;
        // requires graphviz with cairo, e.g.
        // brew install graphviz --with-pango
        if cfg!(target_family = "unix") {
            let path = std::path::Path::new(&filename).with_extension("dot");
            let svg = std::path::Path::new(&filename).with_extension("svg");
            let mut command = std::process::Command::new("dot");
            command
                .arg("-Tsvg")
                .arg(path.to_str().unwrap())
                .arg("-o")
                .arg(svg.to_str().unwrap());
            let res = if wait {
                command.output().map(|_| log::info!("SVG exported"))
            } else {
                command.spawn().map(|_| log::info!("exporting SVG"))
            };
            res.map_err(|e| {
                log::warn!("SVG export failed: {}", e);
                e
            })
        } else {
            Ok(())
        }
    }
}
