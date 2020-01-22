use super::expressions::{Error, Expr};
/// policy language
use super::{externals, headers, lexer, literals, parser, types};
use headers::Headers;
use literals::Literal;
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use types::{Signature, Typ};

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
    fn unreachable(&self, top: &[&String]) -> Vec<String> {
        let indices: Vec<&graph::NodeIndex> =
            top.iter().filter_map(|s| self.nodes.get(*s)).collect();
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
    pub externals: externals::Externals,
    pub code: Code,
    pub headers: Headers,
    pub policies: Policies,
}

impl Program {
    pub fn blake3_hash(&self) -> Option<arrayvec::ArrayString<[u8; 64]>> {
        bincode::serialize(self)
            .map(|bytes| blake3::hash(&bytes).to_hex())
            .ok()
    }
    pub fn to_bincode(&self) -> Result<String, std::io::Error> {
        bincode::serialize(self)
            .map(|a| base64::encode(&a))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    pub fn from_bincode<T: ?Sized + AsRef<[u8]>>(s: &T) -> Result<Self, std::io::Error> {
        let bytes =
            base64::decode(s).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        bincode::deserialize(&bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    fn internal(&self, s: &str) -> Option<&Expr> {
        self.code.0.get(s)
    }
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
        self.policies.cut(set)
    }
    pub fn typ(&self, name: &str) -> Option<types::Signature> {
        self.headers.typ(name)
    }
    pub fn policy(&self, name: &str) -> Policy {
        self.policies.0.get(name).cloned().unwrap_or_default()
    }
    fn is_empty(&self) -> bool {
        self.policies.0.is_empty()
    }
    fn is_allow_all(&self) -> bool {
        // does not capture case when program is empty
        self.policies.0.values().all(|p| p.is_allow())
    }
    fn is_deny_all(&self) -> bool {
        self.policies.0.values().all(|p| p.is_deny())
    }
    pub fn description(&self) -> String {
        if self.is_empty() {
            "empty".to_string()
        } else if self.is_allow_all() {
            "allow all".to_string()
        } else if self.is_deny_all() {
            "deny all".to_string()
        } else if let Some(hash) = self.blake3_hash() {
            hash.to_string()
        } else {
            "hash failed!".to_string()
        }
    }
    pub fn from_file_option<P: AsRef<std::path::Path>>(
        bincode: bool,
        path: Option<P>,
    ) -> Result<Self, std::io::Error> {
        if let Some(path) = path {
            if bincode {
                use std::io::prelude::Read;
                let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
                let mut buf = String::new();
                reader.read_to_string(&mut buf)?;
                Program::from_bincode(buf.as_bytes())
            } else {
                Ok(Module::from_file(path, None)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .program)
            }
        } else {
            Ok(Self::default())
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionInterface {
    signatures: Vec<types::Signature>,
    default: types::Signature,
    allow: Literal,
    deny: Literal,
}

impl FunctionInterface {
    pub fn new(
        signatures: Vec<types::Signature>,
        default: types::Signature,
        allow: Literal,
        deny: Literal,
    ) -> FunctionInterface {
        FunctionInterface {
            signatures,
            default,
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
    pub fn insert_bool(&mut self, name: &str, args: Vec<Vec<Typ>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::Bool))
            .collect();
        self.insert(
            name,
            FunctionInterface::new(
                sigs,
                Signature::any(Typ::Bool),
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

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_CLIENT_PAYLOAD: &str = "allow_client_payload";
pub const ALLOW_SERVER_PAYLOAD: &str = "allow_server_payload";
pub const ALLOW_REST_RESPONSE: &str = "allow_rest_response";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

lazy_static! {
    pub static ref REST_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(ALLOW_REST_REQUEST, vec![vec![Typ::HttpRequest], Vec::new()]);
        policy.insert_bool(ALLOW_CLIENT_PAYLOAD, vec![vec![Typ::Payload]]);
        policy.insert_bool(ALLOW_SERVER_PAYLOAD, vec![vec![Typ::Payload]]);
        policy.insert_bool(
            ALLOW_REST_RESPONSE,
            vec![vec![Typ::HttpResponse], Vec::new()],
        );
        policy
    };
    pub static ref TCP_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(
            ALLOW_TCP_CONNECTION,
            vec![vec![Typ::Connection], Vec::new()],
        );
        policy.insert_unit(
            ON_TCP_DISCONNECT,
            vec![
                vec![Typ::Connection, Typ::I64, Typ::I64],
                vec![Typ::Connection],
                Vec::new(),
            ],
        );
        policy
    };
    pub static ref TCP_REST_POLICY: Interface = {
        let mut policy = TCP_POLICY.clone();
        policy.extend(&REST_POLICY);
        policy
    };
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Policy {
    Allow,
    Deny,
    Unit,
    Args(u8),
}

impl Default for Policy {
    fn default() -> Self {
        Policy::Deny
    }
}

impl Policy {
    pub fn is_allow(&self) -> bool {
        *self == Policy::Allow || *self == Policy::Unit
    }
    fn is_deny(&self) -> bool {
        *self == Policy::Deny || *self == Policy::Unit
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Policies(pub BTreeMap<String, Policy>);

impl Policies {
    fn cut(&mut self, set: &[String]) {
        for s in set.iter() {
            self.0.remove(s);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Module {
    call_graph: CallGraph,
    interface: Interface,
    pub program: Program,
}

impl Module {
    pub fn policy(&self, name: &str) -> Policy {
        if let Some(e) = self.program.internal(name) {
            if let Some(i) = self.interface(name) {
                match e {
                    Expr::LitExpr(Literal::Unit) => Policy::Unit,
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
    fn arg_count(&self, name: &str) -> Option<u8> {
        self.program
            .typ(name)
            .map(|sig| sig.args().unwrap_or_else(Vec::new).len() as u8)
    }
    pub fn interface(&self, s: &str) -> Option<&FunctionInterface> {
        self.interface.0.get(s)
    }
    fn add_decl(&mut self, decl: &parser::FnDecl) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &self.program.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = self
            .call_graph
            .nodes
            .get(name)
            .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls.into_iter().filter(|c| !Headers::is_builtin(&c.name)) {
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
        match self.program.headers.typ(function) {
            Some(f_sig) => {
                if interface
                    .signatures
                    .iter()
                    .any(|sig| Module::type_check(function, &f_sig, sig).is_ok())
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
                self.program
                    .headers
                    .add_function(function, interface.default.clone())?;
                let lit = if allow {
                    interface.allow.clone()
                } else {
                    interface.deny.clone()
                };
                self.program
                    .code
                    .0
                    .insert(function.to_owned(), Expr::LitExpr(lit));
                Ok(())
            }
        }
    }
    pub fn set_interface(&mut self, interface: &Interface, allow: bool) -> Result<(), Error> {
        self.interface = interface.clone();
        for (function, i) in interface.0.iter() {
            self.check_interface(function, i, allow)?;
            self.program
                .policies
                .0
                .insert(function.to_string(), self.policy(function));
        }
        Ok(())
    }
    pub fn allow_all(interface: &Interface) -> Result<Self, Error> {
        let mut module = Module::default();
        module.set_interface(interface, true)?;
        Ok(module)
    }
    pub fn deny_all(interface: &Interface) -> Result<Self, Error> {
        let mut module = Module::default();
        module.set_interface(interface, false)?;
        Ok(module)
    }
    pub fn from_buf(buf: &str, interface: Option<&Interface>) -> Result<Self, Error> {
        let mut module: Module = buf.parse()?;
        module.call_graph.check_for_cycles()?;
        if let Some(interface) = interface {
            // TODO: if no interface functions are declared then default to deny
            module.set_interface(interface, true)?;
            let top: Vec<&String> = interface.0.keys().collect();
            module
                .program
                .cut(module.call_graph.unreachable(top.as_slice()).as_slice());
        }
        Ok(module)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
        interface: Option<&Interface>,
    ) -> Result<Self, Error> {
        use std::io::prelude::Read;
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        Module::from_buf(&buf, interface)
    }
}

impl std::str::FromStr for Module {
    type Err = Error;

    fn from_str(buf: &str) -> Result<Self, Self::Err> {
        let toks = lexer::lex(buf);
        let tokens = lexer::Tokens::new(&toks);
        // println!("{}", tokens);
        match parser::parse_program(tokens) {
            Ok((_rest, prog_parse)) => {
                let mut module = Module::default();
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