/// policy language
use super::{
    externals,
    expressions::{DPExpr, Error, Expr},
    headers::{Headers, DPHeaders, THeaders},
    lexer,
    literals::{self, TFlatLiteral, CPFlatLiteral},
    parser::{self, TParser },
    types_cp::{CPFlatTyp},
    types::{self, TFlatTyp}
};
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::iter::FromIterator;

//FIXME duplicated with interpreter
//type Headers = headers::Headers<parser::Typ, types::Typ>;
//type DPExpr = expressions::Expr<types::FlatTyp, literals::DPFlatLiteral>;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct Code<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(pub BTreeMap<String, Expr<FlatTyp, FlatLiteral>>);
pub type DPCode = Code<types::FlatTyp, literals::FlatLiteral>;
pub type CPCode = Code<CPFlatTyp, literals::CPFlatLiteral>;

impl From<CPCode> for DPCode {
    fn from(cpcode: CPCode) -> DPCode {
        Code( BTreeMap::from_iter( cpcode.0.into_iter().map(|(s, e)| (s, DPExpr::from(e)) )))
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Code<FlatTyp, FlatLiteral> {
    fn cut(&mut self, set: &[String]) {
        for s in set.iter() {
            self.0.remove(s);
        }
    }
    pub fn get(&self, s: String) -> Option<Expr<FlatTyp, FlatLiteral>> {
        match self.0.get(&s) {
            None => None,
            Some(e) => Some(e.clone())
        }
    }
    pub fn insert(&mut self, s: String, e: Expr<FlatTyp, FlatLiteral>) -> Option<Expr<FlatTyp, FlatLiteral>> {
        self.0.insert(s, e)
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct Program<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub code: Code<FlatTyp, FlatLiteral>,
    pub externals: externals::Externals,
    pub headers: Headers<FlatTyp>,
    phantom: PhantomData<FlatLiteral>
}

pub type DPProgram = Program<types::FlatTyp, literals::FlatLiteral>;
pub type CPProgram = Program<CPFlatTyp, literals::CPFlatLiteral>;

impl From<CPProgram> for DPProgram {
    fn from(cp: CPProgram) -> Self {
        DPProgram {
            code: DPCode::from(cp.code), 
            externals: cp.externals,
            headers: DPHeaders::from(cp.headers),
            phantom: PhantomData
        }
    }
}

impl< FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Program<FlatTyp, FlatLiteral> {
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
    pub fn typ(&self, name: &str) -> Option<types::Signature<FlatTyp>> {
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
pub struct PreProgram<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    call_graph: CallGraph,
    pub program: Program<FlatTyp, FlatLiteral>,
}
pub type CPPreProgram = PreProgram<CPFlatTyp, CPFlatLiteral>;

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> PreProgram<FlatTyp, FlatLiteral> {
    fn add_decl(&mut self, decl: &parser::FnDecl<FlatTyp, FlatLiteral>) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &self.program.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = self
            .call_graph
            .nodes
            .get(name)
            .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls.into_iter().filter(|c| !<Headers<FlatTyp>>::is_internal(&c.name)) {
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
    pub fn program(&self, functions: &[String]) -> Program<FlatTyp, FlatLiteral> {
        let mut prog = self.program.clone();
        if !functions.is_empty() {
            prog.cut(self.call_graph.unreachable(functions).as_slice())
        };
        prog
    }
    pub fn from_buf(buf: &str) -> Result<Self, Error> {
        println!("lang::PreProgram::from_bu, building preprogrm");
        let pre_prog: PreProgram<FlatTyp, FlatLiteral> = buf.parse()?;
        pre_prog.call_graph.check_for_cycles()?;
        println!("lang::PreProgram::from_bu, preprogram built");
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

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>  std::str::FromStr for PreProgram<FlatTyp, FlatLiteral> {
    type Err = Error;

    fn from_str(buf: &str) -> Result<Self, Self::Err> {
        let toks = lexer::lex(buf);
        let tokens = lexer::Tokens::new(&toks);
        // println!("{}", tokens);
        match parser::Parser::parse_program(tokens) {
            Ok((_rest, prog_parse)) => {
                let mut module : PreProgram<FlatTyp, FlatLiteral> = PreProgram::default();
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
                        },
                        parser::Decl::Phantom(_) => unimplemented!()
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
            Err(nom::Err::Error((toks, _))) => match <parser::Parser<FlatTyp, FlatLiteral>>::parse_fn_head(toks) {
                Ok((rest, head)) => {
                    let s = format!(
                        r#"syntax error in body of function "{}" starting at line {:?}"#,
                        head.name(),
                        toks.tok[0].loc.line
                    );
                    match <parser::Parser<FlatTyp, FlatLiteral>>::parse_block_stmt(rest) {
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
