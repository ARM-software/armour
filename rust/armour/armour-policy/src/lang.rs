use super::expressions::{Error, Expr};
/// policy language
use super::{externals, headers, lexer, literals, parser, types};
use futures::{future, Future};
use headers::Headers;
use literals::Literal;
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use types::{Signature, Typ};

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
    interface: Interface,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionInterface {
    signatures: Vec<types::Signature>,
    default: types::Signature,
    default_to_allow: bool,
    allow: Literal,
    deny: Literal,
}

impl FunctionInterface {
    pub fn new(
        signatures: Vec<types::Signature>,
        default: types::Signature,
        default_to_allow: bool,
        allow: Literal,
        deny: Literal,
    ) -> FunctionInterface {
        FunctionInterface {
            signatures,
            default,
            default_to_allow,
            allow,
            deny,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Interface(BTreeMap<String, FunctionInterface>);

impl Interface {
    pub fn new() -> Self {
        Interface(BTreeMap::new())
    }
    pub fn insert(&mut self, name: &str, policy: FunctionInterface) {
        self.0.insert(name.to_string(), policy);
    }
    pub fn insert_bool(&mut self, name: &str, args: Vec<Vec<Typ>>, default_to_allow: bool) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::Bool))
            .collect();
        self.insert(
            name,
            FunctionInterface::new(
                sigs,
                Signature::any(Typ::Bool),
                default_to_allow,
                Literal::Bool(true),
                Literal::Bool(false),
            ),
        )
    }
    pub fn insert_unit(&mut self, name: &str, args: Vec<Vec<Typ>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::Unit))
            .collect();
        self.insert(
            name,
            FunctionInterface::new(
                sigs,
                Signature::any(Typ::Unit),
                false,
                Literal::Unit,
                Literal::Unit,
            ),
        )
    }
    pub fn extend(&mut self, other: &Interface) {
        self.0
            .extend(other.0.iter().map(|(name, i)| (name.clone(), i.clone())))
    }
}

#[derive(Clone, PartialEq)]
pub enum Policy {
    Allow,
    Deny,
    Args(u8),
}

impl Default for Policy {
    fn default() -> Self {
        Policy::Deny
    }
}

impl Policy {
    pub fn is_allow(&self) -> bool {
        *self == Policy::Allow
    }
    fn is_deny(&self) -> bool {
        *self == Policy::Deny
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
    pub fn policy(&self, name: &str) -> Policy {
        if let Some(e) = self.internal(name) {
            if let Some(i) = self.interface(name) {
                match e {
                    Expr::LitExpr(body) => {
                        if *body == i.allow {
                            Policy::Allow
                        } else if *body == i.deny {
                            Policy::Deny
                        } else {
                            Policy::Args(self.arg_count(name).unwrap_or_default())
                        }
                    }
                    _ => Policy::Args(self.arg_count(name).unwrap_or_default()),
                }
            } else {
                log::warn!("function not in policy interface: {}", name);
                Policy::Deny
            }
        } else {
            log::warn!("missing policy function: {}", name);
            Policy::Deny
        }
    }
    pub fn is_allow_all(&self) -> bool {
        self.interface
            .0
            .iter()
            .all(|(function, i)| i.allow == Literal::Unit || self.policy(&function).is_allow())
    }
    pub fn is_deny_all(&self) -> bool {
        self.interface
            .0
            .iter()
            .all(|(function, i)| i.allow == Literal::Unit || self.policy(&function).is_deny())
    }
    // pub fn has_function(&self, name: &str) -> bool {
    //     self.code.0.contains_key(name)
    // }
    pub fn typ(&self, name: &str) -> Option<types::Signature> {
        self.headers.typ(name)
    }
    fn arg_count(&self, name: &str) -> Option<u8> {
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
    pub fn interface(&self, s: &str) -> Option<&FunctionInterface> {
        self.interface.0.get(s)
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
    fn type_check(
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
    fn check_interface(
        &mut self,
        function: &str,
        interface: &FunctionInterface,
        allow: bool,
    ) -> Result<(), Error> {
        match self.headers.typ(function) {
            Some(f_sig) => {
                if interface
                    .signatures
                    .iter()
                    .any(|sig| Program::type_check(function, &f_sig, sig).is_ok())
                {
                    Ok(())
                } else {
                    let possible = interface
                        .signatures
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
            None => {
                // add default using interface
                self.headers
                    .add_function(function, interface.default.clone())?;
                let lit = if allow {
                    interface.allow.clone()
                } else {
                    interface.deny.clone()
                };
                self.code.0.insert(function.to_owned(), Expr::LitExpr(lit));
                Ok(())
            }
        }
    }
    pub fn set_interface(
        &mut self,
        interface: &Interface,
        allow: Option<bool>,
    ) -> Result<(), Error> {
        for (function, i) in interface.0.iter() {
            self.check_interface(function, i, allow.unwrap_or_else(|| i.default_to_allow))?
        }
        self.interface = interface.clone();
        Ok(())
    }
    pub fn allow_all(interface: &Interface) -> Result<Self, Error> {
        let mut prog = Program::default();
        prog.set_interface(interface, Some(true))?;
        Ok(prog)
    }
    pub fn deny_all(interface: &Interface) -> Result<Self, Error> {
        let mut prog = Program::default();
        prog.set_interface(interface, Some(false))?;
        Ok(prog)
    }
    pub fn from_buf(buf: &str, interface: &Interface) -> Result<Self, Error> {
        let mut prog: Self = buf.parse()?;
        prog.set_interface(interface, None)?;
        Ok(prog)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
        interface: &Interface,
    ) -> Result<Self, Error> {
        use std::io::prelude::Read;
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        Program::from_buf(&buf, interface)
    }
}

impl std::str::FromStr for Program {
    type Err = Error;

    fn from_str(buf: &str) -> Result<Self, Self::Err> {
        let toks = lexer::lex(buf);
        let tokens = lexer::Tokens::new(&toks);
        // println!("{}", tokens);
        match parser::parse_program(tokens) {
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
