/// policy language
use super::{externals, headers, lexer, literals, parser, types};
use super::expressions::{Error, Expr};
use futures::{future, Future};
use headers::Headers;
use literals::Literal;
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use types::Typ;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Code(pub BTreeMap<String, Expr>);

struct CallGraph {
    graph: graph::DiGraph<String, lexer::Loc>,
    nodes: HashMap<String, graph::NodeIndex>,
}

impl CallGraph {
    fn new() -> CallGraph {
        CallGraph {
            graph: graph::Graph::new(),
            nodes: HashMap::new(),
        }
    }
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
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Program {
    pub code: Code,
    pub externals: externals::Externals,
    pub headers: Headers,
}

struct Hash<'a>(&'a [u8]);

impl<'a> std::fmt::Display for Hash<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            std::fmt::LowerHex::fmt(byte, f)?
        }
        write!(f, "")
    }
}

impl Program {
    pub fn blake2_hash(&self) -> Option<String> {
        bincode::serialize(self)
            .map(|bytes| {
                Hash(blake2_rfc::blake2b::blake2b(24, b"armour", &bytes).as_bytes()).to_string()
            })
            .ok()
    }
    pub fn has_function(&self, name: &str) -> bool {
        self.code.0.contains_key(name)
    }
    pub fn typ(&self, name: &str) -> Option<types::Signature> {
        self.headers.typ(name)
    }
    pub fn arg_count(&self, name: &str) -> Option<u8> {
        self.typ(name)
            .map(|sig| sig.args().unwrap_or_else(Vec::new).len() as u8)
    }
    pub fn set_timeout(&mut self, t: std::time::Duration) {
        self.externals.set_timeout(t)
    }
    pub fn timeout(&self) -> std::time::Duration {
        self.externals.timeout()
    }
    pub fn internal(&self, s: &str) -> Option<&Expr> {
        self.code.0.get(s)
    }
    pub fn external(
        &self,
        external: &str,
        method: &str,
        args: Vec<Expr>,
    ) -> Box<dyn Future<Item = Expr, Error = Error>> {
        if let Some(socket) = self.externals.get_socket(external) {
            match Literal::literal_vector(args) {
                Ok(lits) => Box::new(
                    externals::Externals::request(
                        external.to_string(),
                        method.to_string(),
                        lits,
                        socket,
                        self.externals.timeout(),
                    )
                    .and_then(|r| future::ok(Expr::LitExpr(r)))
                    .from_err(),
                ),
                Err(err) => Box::new(future::err(err)),
            }
        } else {
            Box::new(future::err(Error::new(format!(
                "missing exteral: {}",
                external
            ))))
        }
    }
    fn add_decl(&mut self, call_graph: &mut CallGraph, decl: &parser::FnDecl) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &self.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = call_graph
            .nodes
            .get(name)
            .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls.into_iter().filter(|c| !Headers::is_builtin(&c.name)) {
            let call_idx = call_graph
                .nodes
                .get(&c.name)
                .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", c.name)))?;
            call_graph.graph.add_edge(*own_idx, *call_idx, c.loc);
        }
        self.code.0.insert(name.to_string(), e);
        Ok(())
    }
    fn type_check1(
        function: &str,
        sig1: &types::Signature,
        sig2: &types::Signature,
    ) -> Result<(), Error> {
        let (args1, ty1) = sig1.split_as_ref();
        let (args2, ty2) = sig2.split_as_ref();
        Typ::type_check(function, vec![(None, ty1)], vec![(None, ty2)]).map_err(Error::from)?;
        match (args1, args2) {
            (Some(a1), Some(a2)) => {
                let a1 = a1.iter().map(|t| (None, t)).collect();
                let a2 = a2.iter().map(|t| (None, t)).collect();
                Typ::type_check(function, a1, a2).map_err(Error::from)
            }
            (Some(_), None) => Err(Error::new(format!(
                "type of function not general enough: {}",
                function
            ))),
            (None, None) | (None, Some(_)) => Ok(()),
        }
    }
    fn type_check(&self, function: &str, sigs: &[types::Signature]) -> Result<(), Error> {
        match self.headers.typ(function) {
            Some(f_sig) => {
                if sigs
                    .iter()
                    .any(|sig| Program::type_check1(function, &f_sig, sig).is_ok())
                {
                    Ok(())
                } else {
                    let possible = sigs
                        .iter()
                        .map(|sig| sig.to_string())
                        .collect::<Vec<String>>()
                        .join("; ");
                    Err(Error::new(format!(
                        r#"unable to find suitable instance of function "{}". possible types are: {}"#,
                        function, possible
                    )))
                }
            }
            None => Ok(()), // ok if not present
        }
    }
    pub fn check_from_file<P: AsRef<std::path::Path>>(
        path: P,
        check: &[(&'static str, Vec<types::Signature>)],
    ) -> Result<Self, Error> {
        use std::io::prelude::Read;
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        let prog: Self = buf.parse()?;
        for (f, sigs) in check {
            prog.type_check(f, sigs)?
        }
        Ok(prog)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Program::check_from_file(path, &Vec::new())
    }
}

impl std::str::FromStr for Program {
    type Err = Error;

    fn from_str(buf: &str) -> Result<Self, Self::Err> {
        match parser::parse_program(lexer::Tokens::new(&lexer::lex(buf))) {
            Ok((_rest, prog_parse)) => {
                let mut call_graph = CallGraph::new();
                let mut prog = Program::default();
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
                            prog.headers.add_function(name, sig)?;
                            call_graph.add_node(name);
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
                                prog.headers.add_function(name, sig)?;
                                call_graph.add_node(name);
                            }
                            if prog.externals.add_external(ename, e.url()) {
                                println!("WARNING: external \"{}\" already existed", ename)
                            }
                        }
                    }
                }
                // process declarations
                for decl in prog_parse {
                    if let parser::Decl::FnDecl(decl) = decl {
                        prog.add_decl(&mut call_graph, &decl)?
                    }
                }
                call_graph.check_for_cycles()?;
                Ok(prog)
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
