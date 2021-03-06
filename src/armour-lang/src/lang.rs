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

/// policy language
use super::{
    externals,
    expressions::{DPExpr, Error, Expr},
    headers::{Headers, DPHeaders, THeaders},
    lexer,
    literals::{self, TFlatLiteral, CPFlatLiteral},
    parser::{self, TParser},
    types::{self, CPFlatTyp, TFlatTyp}
};
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::iter::FromIterator;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct Code<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(pub BTreeMap<String, Expr<FlatTyp, FlatLiteral>>);
pub type DPCode = Code<types::FlatTyp, literals::FlatLiteral>;
pub type CPCode = Code<CPFlatTyp, literals::CPFlatLiteral>;

impl From<CPCode> for DPCode {
    fn from(cpcode: CPCode) -> DPCode {
        Code( BTreeMap::from_iter( cpcode.0.into_iter().map(|(s, e)| (s, DPExpr::from(e)) )))
    }
}

impl<FlatTyp, FlatLiteral> Code<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
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

    pub fn merge(&self, other: &Self) -> Self{
        Code(self.0.clone().into_iter().chain(other.0.clone().into_iter()).collect())
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

impl< FlatTyp, FlatLiteral> Program<FlatTyp, FlatLiteral>
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
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

    pub fn merge(&self, other: &Self) -> Self{
        Program{
            code: self.code.merge(&other.code),
            externals: self.externals.merge(&other.externals),
            headers: self.headers.merge(&other.headers),
            phantom: PhantomData
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PreProgram<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    call_graph: CallGraph,
    pub program: Program<FlatTyp, FlatLiteral>,
}
pub type CPPreProgram = PreProgram<CPFlatTyp, CPFlatLiteral>;

impl<FlatTyp, FlatLiteral> PreProgram<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
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
        let pre_prog: PreProgram<FlatTyp, FlatLiteral> = buf.parse()?;
        pre_prog.call_graph.check_for_cycles()?;
        Ok(pre_prog)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        use std::io::prelude::Read;
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        //TODO clean the buf to remove special char
        PreProgram::from_buf(&buf)
    }
}

impl<FlatTyp, FlatLiteral>  std::str::FromStr for PreProgram<FlatTyp, FlatLiteral>
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
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
                        parser::Decl::Phantom(_) => unreachable!()
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


impl<FlatTyp, FlatLiteral> Program<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn aux_deadcode_elim(
        module: &mut PreProgram<FlatTyp, FlatLiteral>,
        e: &Expr<FlatTyp, FlatLiteral>, 
        own_idx: &graph::NodeIndex
    ) -> Result<(), Error> { 
        match e {
            Expr::Var(_) | Expr::BVar(_, _) | Expr::LitExpr(_) => Ok(()),
            Expr::ReturnExpr(e) | Expr::PrefixExpr(_, e) |Expr::Closure(_, e) => Self::aux_deadcode_elim(module, e, own_idx),
            Expr::InfixExpr(_, e1, e2) | Expr::Let(_, e1, e2) => {
                Self::aux_deadcode_elim(module, e1, own_idx)?; 
                Self::aux_deadcode_elim(module, e2, own_idx)},
            Expr::Iter(_, _, e1, e2, acc_opt)=> {
                Self::aux_deadcode_elim(module, e1, own_idx)?;
                Self::aux_deadcode_elim(module, e2, own_idx)?;
                match acc_opt {
                    Some((_, acc)) => Self::aux_deadcode_elim(module, acc, own_idx),
                    _ => Ok(())
                }
            },
            Expr::BlockExpr(_, xs) => {xs.iter().map(|e| Self::aux_deadcode_elim(module, e, own_idx)).for_each(drop); Ok(())},
            Expr::IfExpr { cond, consequence, alternative} => {
                Self::aux_deadcode_elim(module, cond, own_idx)?;
                Self::aux_deadcode_elim(module, consequence, own_idx)?; 
                match alternative {
                    Some(e3) => Self::aux_deadcode_elim(module, e3, own_idx),
                    _ => Ok(())
                }
            },
            Expr::IfSomeMatchExpr  { expr, consequence, alternative} => {
                Self::aux_deadcode_elim(module, expr, own_idx)?; 
                Self::aux_deadcode_elim(module, consequence, own_idx)?; 
                match alternative {
                    Some(e3) => Self::aux_deadcode_elim(module, e3, own_idx),
                    _ => Ok(())
                }
            },
            Expr::IfMatchExpr { variables:_, matches, consequence, alternative} => {
                matches.iter().map(|(e,_)| Self::aux_deadcode_elim(module, e, own_idx)).for_each(drop);
                Self::aux_deadcode_elim(module, consequence, own_idx)?;
                match alternative {
                    Some(e3) => Self::aux_deadcode_elim(module, e3, own_idx),
                    _ => Ok(())
                }
            },
            Expr::CallExpr { function, arguments, is_async:_} => {
                arguments.iter().map(|e| Self::aux_deadcode_elim(module, e, own_idx)).for_each(drop);
                if  !<Headers<FlatTyp>>::is_internal(&function) {
                    let call_idx = match module.call_graph.nodes.get(function) {
                        Some(x) => x,
                        None => {                                
                            //adding tmp node
                            module.call_graph.add_node(&function);
                            module.call_graph.nodes.get(function).unwrap()
                        }
                    };

                    module.call_graph.graph.add_edge(*own_idx, *call_idx, lexer::Loc::dummy()); 
                }
                Ok(())
            },
            Expr::Phantom(_) => Ok(()) 
        }
    }

    pub fn deadcode_elim(&self, mains: &[String]) -> Result<Program<FlatTyp, FlatLiteral>, Error> {
        let mut module : PreProgram<FlatTyp, FlatLiteral> = PreProgram::default();
        module.program = self.clone();

        //Fn call

        //Fn declaration
        for (name, _) in &self.headers.0 {
            module.call_graph.add_node(&name);
            //module.call_graph.add_node(&name);
            let own_idx = module 
                .call_graph
                .nodes
                .get(name)
                .ok_or_else(|| Error::new(&format!("cannot find \"{}\" node", name)))?.clone();

            Self::aux_deadcode_elim(&mut module, &self.code.get(name.clone()).unwrap(), &own_idx)?;
        }

        Ok(module.program(mains))
    }
}
