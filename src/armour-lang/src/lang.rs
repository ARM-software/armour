/// policy language
use super::types::{Typ, TTyp};
use super::expressions::{Error, Expr, TExpr};
use super::{externals, headers, lexer, parser, types};
use headers::{THeaders};
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

//FIXME duplicated with interpreter
type Headers = headers::Headers<parser::Typ, types::Typ>;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Code(pub BTreeMap<String, Expr>);

impl Code {
    fn cut(&mut self, set: &[String]) {
        for s in set.iter() {
            self.0.remove(s);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct CallGraph {
    graph: graph::DiGraph<String, lexer::Loc>,
    nodes: HashMap<String, graph::NodeIndex>,
}

impl CallGraph {
    fn add_node(&mut self, name: &str) {
        self.nodes
            .insert(name.to_string(), self.graph.add_node(name.to_string()));
    }
    fn check_for_cycles(&self) -> Result<(), Error> {
        if let Err(cycle) = petgraph::algo::toposort(&self.graph, None) {
            if let Some(name) = self.graph.node_weight(cycle.node_id()) {
                Err(Error::new(&format!(
                    "cycle detected: the function \"{}\" might not terminate",
                    name
                )))
            } else {
                Err(Error::new("cycle detected for unknown function"))
            }
        } else {
            Ok(())
        }
    }
    fn unreachable(&self, top: &[String]) -> Vec<String> {
        let indices: Vec<&graph::NodeIndex> =
            top.iter().filter_map(|s| self.nodes.get(s)).collect();
        let mut unreachable = Vec::new();
        for (node, index) in self.nodes.iter() {
            let is_reachable = indices.iter().any(|top_node| {
                petgraph::algo::has_path_connecting(&self.graph, **top_node, *index, None)
            });
            if !is_reachable {
                unreachable.push(node.to_string())
            }
        }
        unreachable
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Program {
    pub code: Code,
    pub externals: externals::Externals,
    pub headers: Headers,
}

impl Program {
    pub fn set_timeout(&mut self, t: std::time::Duration) {
        self.externals.set_timeout(t)
    }
    pub fn timeout(&self) -> std::time::Duration {
        self.externals.timeout()
    }
    fn cut(&mut self, set: &[String]) {
        if !set.is_empty() {
            log::warn!("removing unreachable functions: {:?}", set)
        };
        self.headers.cut(set);
        self.code.cut(set);
    }
    pub fn typ(&self, name: &str) -> Option<types::Signature<parser::Typ, types::Typ>> {
        self.headers.typ(name)
    }
    pub fn is_empty(&self) -> bool {
        self.code.0.is_empty()
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Ok(PreProgram::from_file(path)?.program(&[]))
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PreProgram {
    call_graph: CallGraph,
    pub program: Program,
}

impl PreProgram {
    fn add_decl(&mut self, decl: &parser::FnDecl<parser::Typ, parser::Expr>) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &self.program.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = self
            .call_graph
            .nodes
            .get(name)
            .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls.into_iter().filter(|c| !Headers::is_internal(&c.name)) {
            let call_idx = self
                .call_graph
                .nodes
                .get(&c.name)
                .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", c.name)))?;
            self.call_graph.graph.add_edge(*own_idx, *call_idx, c.loc);
        }
        self.program.code.0.insert(name.to_string(), e);
        Ok(())
    }
    pub fn program(&self, functions: &[String]) -> Program {
        let mut prog = self.program.clone();
        if !functions.is_empty() {
            prog.cut(self.call_graph.unreachable(functions).as_slice())
        };
        prog
    }
    pub fn from_buf(buf: &str) -> Result<Self, Error> {
        let pre_prog: PreProgram = buf.parse()?;
        pre_prog.call_graph.check_for_cycles()?;
        Ok(pre_prog)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        use std::io::prelude::Read;
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        PreProgram::from_buf(&buf)
    }
}

impl std::str::FromStr for PreProgram {
    type Err = Error;

    fn from_str(buf: &str) -> Result<Self, Self::Err> {
        let toks = lexer::lex(buf);
        let tokens = lexer::Tokens::new(&toks);
        // println!("{}", tokens);
        match parser::parse_program(tokens) {
            Ok((_rest, prog_parse)) => {
                let mut module = PreProgram::default();
                // process headers (for type information)
                for decl in prog_parse.iter() {
                    match decl {
                        parser::Decl::FnDecl(decl) => {
                            let name = decl.name();
                            let sig = decl.typ().map_err(|err| {
                                Error::new(&format!(
                                    "function \"{}\" at {}: {}",
                                    name,
                                    decl.loc(),
                                    err
                                ))
                            })?;
                            module.program.headers.add_function(name, sig)?;
                            module.call_graph.add_node(name);
                        }
                        parser::Decl::External(e) => {
                            let ename = e.name();
                            for h in e.headers.iter() {
                                let name = &format!("{}::{}", ename, h.name());
                                let sig = h.typ().map_err(|err| {
                                    Error::new(&format!(
                                        "header \"{}\" at {}: {}",
                                        name,
                                        h.loc(),
                                        err
                                    ))
                                })?;
                                module.program.headers.add_function(name, sig)?;
                                module.call_graph.add_node(name);
                            }
                            if module.program.externals.add_external(ename, e.url()) {
                                println!("WARNING: external \"{}\" already existed", ename)
                            }
                        }
                    }
                }
                // process declarations
                for decl in prog_parse {
                    if let parser::Decl::FnDecl(decl) = decl {
                        module.add_decl(&decl)?
                    }
                }
                Ok(module)
            }
            Err(nom::Err::Error((toks, _))) => match parser::parse_fn_head(toks) {
                Ok((rest, head)) => {
                    let s = format!(
                        r#"syntax error in body of function "{}" starting at line {:?}"#,
                        head.name(),
                        toks.tok[0].loc.line
                    );
                    match parser::parse_block_stmt(rest) {
                        Ok(_) => unreachable!(),
                        Err(nom::Err::Error((toks, _))) => {
                            Err(Error::from(format!("{}\nsee: {}", s, toks.tok[0])))
                        }
                        Err(e) => Err(Error::from(format!("{}\n{:?}", s, e))),
                    }
                }
                Err(nom::Err::Error((toks, _))) => Err(Error::from(format!(
                    "syntax error in function header, starting: {}",
                    toks.tok[0]
                ))),
                Err(e) => Err(Error::from(format!("{:?}", e))),
            },
            Err(e) => Err(Error::from(format!("{:?}", e))),
        }
    }
}
