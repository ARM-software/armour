/// policy language
use super::{headers, labels, lexer, literals, parser, types};
use headers::{Headers, THeaders};
use literals::{Literal, TFlatLiteral, DPLiteral, DPFlatLiteral, CPFlatLiteral};
use parser::{Infix, Prefix, TParser};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use types::{CPFlatTyp, Typ, TTyp, FlatTyp, TFlatTyp};

#[derive(Debug, Clone)]
pub struct Error(String);

impl std::error::Error for Error {}

impl std::convert::From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, e.0)
    }
}

impl Error {
    pub fn new<D: std::fmt::Display>(e: D) -> Error {
        Error(e.to_string())
    }
    pub fn from_display<D: std::fmt::Display>(e: D) -> Error {
        Error(e.to_string())
    }
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error(s)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Error {
        Error::new(e)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::new(err)
    }
}

impl<'a, FlatTyp:TFlatTyp> From<types::Error<FlatTyp>> for Error {
    fn from(err: types::Error<FlatTyp>) -> Error {
        Error::new(err)
    }
}

impl From<headers::Error> for Error {
    fn from(err: headers::Error) -> Error {
        Error::new(err)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Error {
        Error::new(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::new(err)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Call {
    pub loc: lexer::Loc,
    pub name: String,
}

type Calls = HashSet<Call>;

pub struct ExprAndMeta<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub expr: Expr<FlatTyp, FlatLiteral>,
    pub calls: Calls,
    pub typ: Typ<FlatTyp>,
    phantom_typ : PhantomData<FlatTyp>,
    phantom_lit : PhantomData<FlatLiteral>,
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> ExprAndMeta<FlatTyp, FlatLiteral> {
    fn new(expr: Expr<FlatTyp, FlatLiteral>, typ: Typ<FlatTyp>, v: Vec<Calls>) -> ExprAndMeta<FlatTyp, FlatLiteral> {
        let mut calls = Calls::new();
        for c in v {
            calls.extend(c)
        }
        ExprAndMeta { expr, typ, calls, phantom_typ: PhantomData, phantom_lit: PhantomData}
    }
    fn split(self) -> (Expr<FlatTyp, FlatLiteral>, Calls, Typ<FlatTyp>) {
        (self.expr, self.calls, self.typ)
    }
    fn split_vec(v: Vec<ExprAndMeta<FlatTyp, FlatLiteral>>) -> (Vec<Expr<FlatTyp, FlatLiteral>>, Vec<Calls>, Vec<Typ<FlatTyp>>) {
        let mut exprs = Vec::new();
        let mut calls = Vec::new();
        let mut typs = Vec::new();
        for (e, c, t) in v.into_iter().map(|em| em.split()) {
            exprs.push(e);
            calls.push(c);
            typs.push(t);
        }
        (exprs, calls, typs)
    }
}

//#[derive(Default)]
#[derive(Clone)]
pub struct ReturnType<FlatTyp:TFlatTyp> (pub Option<Typ<FlatTyp>>);

impl<FlatTyp:TFlatTyp> Default for ReturnType<FlatTyp> {
    fn default() -> Self { ReturnType(None) }
}

impl<FlatTyp:TFlatTyp> ReturnType<FlatTyp> {
    pub fn get(&self) -> Option<Typ<FlatTyp>> {
        self.0.clone()
    }
    pub fn set(&mut self, typ: Typ<FlatTyp>) {
        self.0 = Some(typ)
    }
}

#[derive(Clone)]
pub struct Context<FlatTyp:TFlatTyp> {
    pub variables: HashMap<String, Typ<FlatTyp>>,
    pub async_tag: bool,
}

impl<FlatTyp:TFlatTyp> Context<FlatTyp> {
    pub fn new() -> Context<FlatTyp> {
        Context {
            variables: HashMap::new(),
            async_tag: false,
        }
    }
    pub fn add_var(&self, name: &str, typ: &Typ<FlatTyp>) -> Self {
        let mut ctxt = self.clone();
        ctxt.variables.insert(name.to_string(), typ.to_owned());
        ctxt
    }
    pub fn update_async_tag(&self, b: bool) -> Self {
        let mut ctxt = self.clone();
        ctxt.async_tag = self.async_tag || b;
        ctxt
    }
    fn var(&self, name: &str) -> Option<Typ<FlatTyp>> {
        self.variables.get(name).cloned()
    }
    fn async_tag(&self) -> bool {
        self.async_tag
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    List,
    Tuple,
    Block,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Pattern {
    Regex(parser::PolicyRegex),
    Label(labels::Label),
}

//#[derive(PartialEq, Debug, Clone, Serialize)]
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Expr<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    Var(parser::Ident),
    BVar(parser::Ident, usize),
    LitExpr(Literal<FlatTyp, FlatLiteral>),
    ReturnExpr(Box<Expr<FlatTyp, FlatLiteral>>),
    PrefixExpr(Prefix<FlatTyp>, Box<Expr<FlatTyp, FlatLiteral>>),
    InfixExpr(Infix<FlatTyp>, Box<Expr<FlatTyp, FlatLiteral>>, Box<Expr<FlatTyp, FlatLiteral>>),
    BlockExpr(Block, Vec<Expr<FlatTyp, FlatLiteral>>),
    Let(Vec<String>, Box<Expr<FlatTyp, FlatLiteral>>, Box<Expr<FlatTyp, FlatLiteral>>),
    Iter(parser::Iter, Vec<String>, Box<Expr<FlatTyp, FlatLiteral>>, Box<Expr<FlatTyp, FlatLiteral>>, Option<Box<Expr<FlatTyp, FlatLiteral>>>),
    Closure(parser::Ident, Box<Expr<FlatTyp, FlatLiteral>>),
    IfExpr {
        cond: Box<Expr<FlatTyp, FlatLiteral>>,
        consequence: Box<Expr<FlatTyp, FlatLiteral>>,
        alternative: Option<Box<Expr<FlatTyp, FlatLiteral>>>,
    },
    IfMatchExpr {
        variables: Vec<String>,
        matches: Vec<(Expr<FlatTyp, FlatLiteral>, Pattern)>,
        consequence: Box<Expr<FlatTyp, FlatLiteral>>,
        alternative: Option<Box<Expr<FlatTyp, FlatLiteral>>>,
    },
    IfSomeMatchExpr {
        expr: Box<Expr<FlatTyp, FlatLiteral>>,
        consequence: Box<Expr<FlatTyp, FlatLiteral>>,
        alternative: Option<Box<Expr<FlatTyp, FlatLiteral>>>,
    },
    CallExpr {
        function: String,
        arguments: Vec<Expr<FlatTyp, FlatLiteral>>,
        is_async: bool,
    },
    Phantom(PhantomData<(FlatTyp, FlatLiteral)>),
}
//impl<'de, FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Deserialize<'de> for Box<Expr<FlatTyp, FlatLiteral>> {
//
//    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//    where
//        D: Deserializer<'de>,
//    {
//        /* your implementation here */
//        match Expr::deserialize(deserializer) {
//            Ok(e) => Ok(Box::new(e)),
//            err => err
//        }
//    }
//}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Default for Expr<FlatTyp, FlatLiteral> {
    fn default() -> Self {
        Expr::LitExpr(Literal::unit())
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>, T: Into<Literal<FlatTyp, FlatLiteral>>> From<T> for Expr<FlatTyp, FlatLiteral> {
    fn from(t: T) -> Self {
        Expr::LitExpr(t.into())
    }
}

pub trait TExpr<FlatTyp:TFlatTyp> {
    fn host(&self) -> Option<String> { None }
}

pub type DPExpr = Expr<types::FlatTyp, DPFlatLiteral>;
pub type CPExpr = Expr<types::CPFlatTyp, CPFlatLiteral>;

pub type DPPrefix = Prefix<FlatTyp>;
pub type CPPrefix = Prefix<CPFlatTyp>;

impl From<CPPrefix> for DPPrefix {
    fn from(cpp: CPPrefix) -> Self {
        match cpp {
            CPPrefix::Minus => Prefix::Minus,
            CPPrefix::Not => Prefix::Not,
            CPPrefix::Phantom(_) => Prefix::Phantom(PhantomData),
        }
    }
}
pub type DPInfix = Infix<FlatTyp>;
pub type CPInfix = Infix<CPFlatTyp>;

impl From<CPInfix> for DPInfix {
    fn from(cpp: CPInfix) -> Self {
        match cpp {
            Infix::Equal => Infix::Equal,
            Infix::NotEqual=> Infix::NotEqual,
            Infix::Plus=> Infix::Plus,
            Infix::Minus=> Infix::Minus,
            Infix::Divide=> Infix::Divide,
            Infix::Multiply=> Infix::Multiply,
            Infix::Remainder=> Infix::Remainder,
            Infix::GreaterThanEqual=> Infix::GreaterThanEqual,
            Infix::LessThanEqual=> Infix::LessThanEqual,
            Infix::GreaterThan=> Infix::GreaterThan,
            Infix::LessThan=> Infix::LessThan,
            Infix::And=> Infix::And,
            Infix::Or=> Infix::Or,
            Infix::Concat=> Infix::Concat,
            Infix::ConcatStr=> Infix::ConcatStr,
            Infix::Module=> Infix::Module,
            Infix::In=> Infix::In,
            Infix::Dot=> Infix::Dot,
            Infix::Phantom(_) => Infix::Phantom(PhantomData)
        }
    }
}


impl From<CPExpr> for DPExpr {
    fn from(cpexpr: CPExpr) -> DPExpr {
        match cpexpr {
            CPExpr::Var(v) => DPExpr::Var(v) ,
            CPExpr::BVar(v, u) => DPExpr::BVar(v, u) ,
            CPExpr::LitExpr(lit) => DPExpr::LitExpr(DPLiteral::from(lit)) ,
            CPExpr::ReturnExpr(e) => DPExpr::ReturnExpr(Box::new(DPExpr::from(*e))) ,
            CPExpr::PrefixExpr(pre, e) => DPExpr::PrefixExpr(DPPrefix::from(pre), Box::new(DPExpr::from(*e))) ,
            CPExpr::InfixExpr(inf, e1, e2) => DPExpr::InfixExpr(DPInfix::from(inf), Box::new(DPExpr::from(*e1)), Box::new(DPExpr::from(*e2))),
            CPExpr::BlockExpr(b, es) => DPExpr::BlockExpr(b, es.into_iter().map(|e| DPExpr::from(e)).collect()),
            CPExpr::Let(vs, e1, e2) => DPExpr::Let(vs, Box::new(DPExpr::from(*e1)), Box::new(DPExpr::from(*e2))),
            CPExpr::Iter(it, vs, e1, e2, acc) => DPExpr::Iter(it, vs, Box::new(DPExpr::from(*e1)), Box::new(DPExpr::from(*e2)), acc.map(|x| Box::new(DPExpr::from(*x)))),
            CPExpr::Closure(ident, e) => DPExpr::Closure(ident, Box::new(DPExpr::from(*e))),
            CPExpr::IfExpr {
                cond,
                consequence,
                alternative,
            } => DPExpr::IfExpr { 
                cond: Box::new(DPExpr::from(*cond)),
                consequence: Box::new(DPExpr::from(*consequence)),
                alternative: {match alternative {
                    None => None,
                    Some(e) => Some(Box::new(DPExpr::from(*e))),
                }}
            } ,
            CPExpr::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => DPExpr::IfMatchExpr {
                variables,
                matches: matches.into_iter().map(|x| (DPExpr::from(x.0), x.1)).collect() ,
                consequence: Box::new(DPExpr::from(*consequence)),
                alternative: {match alternative {
                    None => None,
                    Some(e) => Some(Box::new(DPExpr::from(*e))),
                }},

            },
            CPExpr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => DPExpr::IfSomeMatchExpr {
                expr:  Box::new(DPExpr::from(*expr)),
                consequence: Box::new(DPExpr::from(*consequence)),
                alternative: {match alternative {
                    None => None,
                    Some(e) => Some(Box::new(DPExpr::from(*e))),
                }},
            },
            CPExpr::CallExpr {
                function,
                arguments,
                is_async,
            } => DPExpr::CallExpr {
                function: function,
                arguments: arguments.into_iter().map(|e| DPExpr::from(e)).collect(),
                is_async: is_async
            },
            CPExpr::Phantom(_) => DPExpr::Phantom(PhantomData) ,
        }
    }

}


impl TExpr<types::CPFlatTyp> for CPExpr {
    fn host(&self) -> Option<String> {
        match self {
            Self::LitExpr(Literal::FlatLiteral(CPFlatLiteral::ID(id))) => id.host(),
            _ => None,
        }
    }
}
impl TExpr<types::FlatTyp> for DPExpr {
    fn host(&self) -> Option<String> {
        match self {
            Self::LitExpr(Literal::FlatLiteral(DPFlatLiteral::ID(id))) => id.host(),
            _ => None,
        }
    }
}
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Expr<FlatTyp, FlatLiteral> {
    pub fn is_free(&self, u: usize) -> bool {
        match self {
            Expr::Var(_) => true,
            Expr::BVar(_, u1) => u1 == &u,
            Expr::LitExpr(_) => true,
            Expr::Closure(_, e) => e.is_free(u+1),
            Expr::ReturnExpr(e) | Expr::PrefixExpr(_, e) => e.is_free(u),  
            Expr::InfixExpr(_, e1, e2) | Expr::Let(_, e1, e2) => e1.is_free(u) && e2.is_free(u), 
            Expr::Iter(_, _, e1, e2, acc_opt) => e1.is_free(u) && e2.is_free(u) && match acc_opt {Some(acc)=> acc.is_free(u), None => true}, 
            Expr::BlockExpr(_, es) => es.iter().fold(true, |acc, x| acc && x.is_free(u)),
            Expr::IfExpr {
                cond,
                consequence,
                alternative,
            } => cond.is_free(u) && consequence.is_free(u) && alternative.as_ref().map_or_else( || true, |x| x.is_free(u)),
            Expr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => expr.is_free(u) && consequence.is_free(u) && alternative.as_ref().map_or_else(|| true, |x| x.is_free(u)),
            Expr::IfMatchExpr {
                variables:_,
                matches,
                consequence,
                alternative,
            } =>  matches.iter().fold(true, |acc, (x,_)| acc && x.is_free(u)) && consequence.is_free(u) && alternative.as_ref().map_or_else(|| true, |x| x.is_free(u)),

            Expr::CallExpr {
                function:_,
                arguments,
                is_async:_,
            } => arguments.iter().fold(true, |acc, x| acc && x.is_free(u)),
            Expr::Phantom(_) => true
        }
    }
    pub fn var(v: &str) -> Self {
        Self::Var(parser::Ident(v.to_string()))
    }
    pub fn bvar(v: &str, u: usize) -> Self {
        Self::BVar(parser::Ident(v.to_string()), u)
    }
    pub fn none() -> Self {
        Self::LitExpr(Literal::none())
    }
    pub fn some(l: Literal<FlatTyp, FlatLiteral>) -> Self {
        Self::LitExpr(Literal::some(&l))
    }
    pub fn call(f: &str, arguments: Vec<Self>) -> Self {
        Self::CallExpr {
            function: f.to_string(),
            arguments,
            is_async: false,
        }
    }
    pub fn return_expr(e: Self) -> Self {
        Self::ReturnExpr(Box::new(e))
    }
    fn prefix_expr(p: Prefix<FlatTyp>, e: Self) -> Self {
        Self::PrefixExpr(p, Box::new(e))
    }
    fn infix_expr(op: Infix<FlatTyp>, e1: Self, e2: Self) -> Self {
        Self::InfixExpr(op, Box::new(e1), Box::new(e2))
    }
    fn if_else_expr(b: Self, e1: Self, e2: Self) -> Self {
        Self::IfExpr {
            cond: Box::new(b),
            consequence: Box::new(e1),
            alternative: Some(Box::new(e2)),
        }
    }
    fn if_expr(b: Self, e: Self) -> Self {
        Self::IfExpr {
            cond: Box::new(b),
            consequence: Box::new(e),
            alternative: None,
        }
    }
    fn let_expr(self, v: Vec<&str>, e: Self) -> Self {
        if v.as_slice() == ["_"] {
            Self::BlockExpr(Block::Block, vec![e, self])
        } else {
            let mut c = self;
            for s in v.iter().rev() {
                c = c.closure_expr(s)
            }
            Self::Let(
                v.iter().map(|s| (*s).to_string()).collect(),
                Box::new(e),
                Box::new(c),
            )
        }
    }
    fn iter_expr(self, op: &parser::Iter, v: Vec<&str>, e: Self, acc: Option<Self>) -> Self {
        let mut c = self;

        for s in v.iter().rev() {
            c = c.closure_expr(s)
        }

        if acc.is_some() {
            c = c.closure_expr("acc");
        }

        Self::Iter(
            op.clone(),
            v.iter().map(|s| (*s).to_string()).collect(),
            Box::new(e),
            Box::new(c),
            match acc { None => None, Some(x) => Some(Box::new(x)) }
        )
    }
    fn shift(self, i: usize, d: usize) -> Self {
        if i == 0 {
            self
        } else {
            match self {
                Self::Var(_) | Self::LitExpr(_) => self,
                Self::BVar(ref id, j) => {
                    if j >= d {
                        Self::BVar(id.to_owned(), j + 1)
                    } else {
                        self
                    }
                }
                Self::Let(l, e1, e2) => {
                    Self::Let(l, Box::new(e1.shift(i, d)), Box::new(e2.shift(i, d)))
                }
                Self::Iter(op, l, e1, e2, acc_opt) => {
                    Self::Iter(op, l, 
                        Box::new(e1.shift(i, d)), 
                        Box::new(e2.shift(i, d)),
                        match acc_opt {
                            Some(acc) => Some(Box::new(acc.shift(i,d))),
                            None => None
                        } 
                    )
                }
                Self::Closure(v, e) => Self::Closure(v, Box::new(e.shift(i, d + 1))),
                Self::ReturnExpr(e) => Self::return_expr(e.shift(i, d)),
                Self::PrefixExpr(p, e) => Self::prefix_expr(p, e.shift(i, d)),
                Self::InfixExpr(op, e1, e2) => Self::infix_expr(op, e1.shift(i, d), e2.shift(i, d)),
                Self::BlockExpr(b, es) => {
                    Self::BlockExpr(b, es.into_iter().map(|e| e.shift(i, d)).collect())
                }
                Self::IfExpr {
                    cond,
                    consequence,
                    alternative: None,
                } => Self::if_expr(cond.shift(i, d), consequence.shift(i, d)),
                Self::IfExpr {
                    cond,
                    consequence,
                    alternative: Some(a),
                } => Self::if_else_expr(cond.shift(i, d), consequence.shift(i, d), a.shift(i, d)),
                Self::IfMatchExpr {
                    variables,
                    matches,
                    consequence,
                    alternative,
                } => Self::IfMatchExpr {
                    variables,
                    matches: matches
                        .into_iter()
                        .map(|(e, p)| (e.shift(i, d), p))
                        .collect(),
                    consequence: Box::new(consequence.shift(i, d)),
                    alternative: alternative.map(|a| Box::new(a.shift(i, d))),
                },
                Self::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => Self::IfSomeMatchExpr {
                    expr: Box::new(expr.shift(i, d)),
                    consequence: Box::new(consequence.shift(i, d)),
                    alternative: alternative.map(|e| Box::new(e.shift(i, d))),
                },
                Self::CallExpr {
                    function,
                    arguments,
                    is_async,
                } => Self::CallExpr {
                    function,
                    arguments: arguments.into_iter().map(|a| a.shift(i, d)).collect(),
                    is_async,
                },
                Self::Phantom(_) => unreachable!()
            }
        }
    }

    pub fn propagate_subst(self, i: usize, j: usize, u: &Self) -> Self {
        match self {
            Self::Closure(_, e) if j == 0 => e.psubst(i, u),
            Self::Closure(v, e) => Self::Closure(v, Box::new(e.propagate_subst(i, j-1, u))),
            _ => self.psubst(i, u)
        }
    }

    pub fn at_depth(self, i: usize) -> Option<Self> {
        match self {
            Self::Closure(_, e) => e.at_depth(i-1),
            e if i == 0 => Some(e),
            _ => None
        }

    }

    pub fn psubst(self, i: usize, u: &Self) -> Self {
        match self {
            Self::Closure(v, e) => Self::Closure(v, Box::new(e.psubst(i, u))),
            _ => self.subst(i, u)
        }
    }

    pub fn subst(self, i: usize, u: &Self) -> Self {
        match self {
            Self::Var(_) | Self::LitExpr(_) => self,
            Self::BVar(ref id, j) => match j.cmp(&i) {
                Ordering::Less => self,
                Ordering::Equal => u.clone().shift(i, 0),
                _ => Self::BVar(id.to_owned(), j - 1),
            },
            Self::Let(l, e1, e2) => {
                Self::Let(l, Box::new(e1.subst(i, u)), Box::new(e2.subst(i, u)))
            }
            Self::Iter(op, l, e1, e2, acc_opt) => {
                Self::Iter(op, l, 
                    Box::new(e1.subst(i, u)), 
                    Box::new(e2.subst(i, u)),
                    acc_opt.map(|acc| Box::new(acc.subst(i,u)))
                )
            }
            Self::Closure(v, e) => Self::Closure(v, Box::new(e.subst(i + 1, u))),
            Self::ReturnExpr(e) => Self::return_expr(e.subst(i, u)),
            Self::PrefixExpr(p, e) => Self::prefix_expr(p, e.subst(i, u)),
            Self::InfixExpr(op, e1, e2) => Self::infix_expr(op, e1.subst(i, u), e2.subst(i, u)),
            Self::BlockExpr(b, es) => {
                Self::BlockExpr(b, es.into_iter().map(|e| e.subst(i, u)).collect())
            }
            Self::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => Self::if_expr(cond.subst(i, u), consequence.subst(i, u)),
            Self::IfExpr {
                cond,
                consequence,
                alternative: Some(a),
            } => Self::if_else_expr(cond.subst(i, u), consequence.subst(i, u), a.subst(i, u)),
            Self::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => Self::IfMatchExpr {
                variables,
                matches: matches
                    .into_iter()
                    .map(|(e, p)| (e.subst(i, u), p))
                    .collect(),
                consequence: Box::new(consequence.subst(i, u)),
                alternative: alternative.map(|a| Box::new(a.subst(i, u))),
            },
            Self::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => Self::IfSomeMatchExpr {
                expr: Box::new(expr.subst(i, u)),
                consequence: Box::new(consequence.subst(i, u)),
                alternative: alternative.map(|e| Box::new(e.subst(i, u))),
            },
            Self::CallExpr {
                function,
                arguments,
                is_async,
            } => Self::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.subst(i, u)).collect(),
                is_async,
            },
            Self::Phantom(_) => unreachable!()
        }
    }
    pub fn apply(self, u: &Self) -> Result<Self, self::Error> {
        match self {
            Self::Closure(_, e) => Ok(e.subst(0, u)),
            _ => Err(Error::new("apply: expression is not a closure")),
        }
    }

    fn check_from_loc_expr(
        e: &parser::LocExpr<FlatTyp, FlatLiteral>,
        headers: &Headers<FlatTyp>,
        ctxt: &Context<FlatTyp>,
    ) -> Result<ExprAndMeta<FlatTyp, FlatLiteral>, Error> {
        let mut ret = ReturnType::default();
        let em = Self::from_loc_expr(e, headers, &mut ret, ctxt)?;
        if let Some(rtype) = ret.get() {
            Typ::type_check("REPL", vec![(None, em.typ.clone())], vec![(None, rtype.clone())])?
        }
        Ok(em)
    }

    fn abs(self, i: usize, v: &str) -> Self {
        match self {
            Self::BVar(_, _) | Self::LitExpr(_) => self,
            Self::Var(ref id) => {
                if id.0 == v {
                    Self::BVar(id.to_owned(), i)
                } else {
                    self
                }
            }
            Self::Let(l, e1, e2) => Self::Let(l, Box::new(e1.abs(i, v)), Box::new(e2.abs(i, v))),
            Self::Iter(op, l, e1, e2, acc_opt) => {
                Self::Iter(op, l, 
                    Box::new(e1.abs(i, v)), 
                    Box::new(e2.abs(i, v)),
                    acc_opt.map(|acc| Box::new(acc.abs(i, v)))
                )
            }
            Self::Closure(v2, e) => Self::Closure(v2, Box::new(e.abs(i + 1, v))),
            Self::ReturnExpr(e) => Self::return_expr(e.abs(i, v)),
            Self::PrefixExpr(p, e) => Self::prefix_expr(p, e.abs(i, v)),
            Self::InfixExpr(op, e1, e2) => Self::infix_expr(op, e1.abs(i, v), e2.abs(i, v)),
            Self::BlockExpr(b, es) => {
                Self::BlockExpr(b, es.into_iter().map(|e| e.abs(i, v)).collect())
            }
            Self::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => Self::if_expr(cond.abs(i, v), consequence.abs(i, v)),
            Self::IfExpr {
                cond,
                consequence,
                alternative: Some(a),
            } => Self::if_else_expr(cond.abs(i, v), consequence.abs(i, v), a.abs(i, v)),
            Self::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => Self::IfMatchExpr {
                variables,
                matches: matches.into_iter().map(|(e, p)| (e.abs(i, v), p)).collect(),
                consequence: Box::new(consequence.abs(i, v)),
                alternative: alternative.map(|e| Box::new(e.abs(i, v))),
            },
            Self::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => Self::IfSomeMatchExpr {
                expr: Box::new(expr.abs(i, v)),
                consequence: Box::new(consequence.abs(i, v)),
                alternative: alternative.map(|e| Box::new(e.abs(i, v))),
            },
            Self::CallExpr {
                function,
                arguments,
                is_async,
            } => Self::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.abs(i, v)).collect(),
                is_async,
            },
            Self::Phantom(_) => unreachable!()
        }
    }
    fn closure_expr(self, v: &str) -> Self {
        if v == "_" {
            self
        } else {
            Self::Closure(parser::Ident::from(v), Box::new(self.abs(0, v)))
        }
    }
    #[allow(clippy::cognitive_complexity)]
    fn from_loc_expr(
        e: &parser::LocExpr<FlatTyp, FlatLiteral>,
        headers: &Headers<FlatTyp>,
        ret: &mut ReturnType<FlatTyp>,
        ctxt: &Context<FlatTyp>,
    ) -> Result<ExprAndMeta<FlatTyp, FlatLiteral>, Error> {
        match e.expr() {
            parser::Expr::IdentExpr(id) => match ctxt.var(&id.0) {
                Some(typ) => Ok(ExprAndMeta::new(Self::var(&id.0), typ, vec![])),
                None => Err(Error::from(format!(
                    "undeclared variable \"{}\" at {}",
                    id.0,
                    e.loc()
                ))),
            },
            parser::Expr::LitExpr(l) => {
                Ok(ExprAndMeta::new(Self::LitExpr(l.clone()), l.typ(), vec![]))
            }
            parser::Expr::ListExpr(es) => {
                let mut exprs = Vec::new();
                let mut calls = Vec::new();
                let mut typ = Typ::rreturn();
                for e in es.iter() {
                    let (expr, call, ty) = Self::from_loc_expr(&e, headers, ret, ctxt)?.split();
                    Typ::type_check("list", vec![(Some(e.loc()), ty.clone())], vec![(None, typ.clone())])?;
                    exprs.push(expr);
                    calls.push(call);
                    typ = typ.unify(&ty);
                }
                Ok(ExprAndMeta::new(
                    Self::BlockExpr(Block::List, exprs),
                    Typ::List(Box::new(typ)),
                    calls,
                ))
            }
            parser::Expr::TupleExpr(es) => {
                let mut exprs = Vec::new();
                let mut calls = Vec::new();
                let mut typs = Vec::new();
                for e in es.iter() {
                    let (expr, call, ty) = Self::from_loc_expr(&e, headers, ret, ctxt)?.split();
                    exprs.push(expr);
                    calls.push(call);
                    typs.push(ty);
                }
                Ok(ExprAndMeta::new(
                    Self::BlockExpr(Block::Tuple, exprs),
                    Typ::Tuple(typs),
                    calls,
                ))
            }
            parser::Expr::PrefixExpr(p, e1) => {
                let (expr, calls, typ) = Self::from_loc_expr(&e1, headers, ret, ctxt)?.split();
                let (t1, t2) = p.typ();
                Typ::type_check("prefix", vec![(Some(e1.loc()), typ.clone())], vec![(None, t1.clone())])?;
                Ok(ExprAndMeta::new(
                    Self::prefix_expr(p.clone(), expr),
                    t2,
                    vec![calls],
                ))
            }
            parser::Expr::InfixExpr(op, e1, e2) => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(&e1, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) = Self::from_loc_expr(&e2, headers, ret, ctxt)?.split();
                let (t1, t2, typ) = op.typ();
                if t1 == Typ::rreturn() {
                    if t2 == Typ::rreturn() {
                        Typ::type_check(
                            "equality/inequality/concat",
                            vec![(Some(e1.loc()), typ1.clone())],
                            vec![(Some(e2.loc()), typ2.clone())],
                        )?
                    } else {
                        Typ::type_check(
                            "in",
                            vec![(Some(e1.loc()), Typ::List(Box::new(typ1)))],
                            vec![(Some(e2.loc()), typ2.clone())],
                        )?
                    }
                } else {
                    Typ::type_check(
                        "infix",
                        vec![(Some(e1.loc()), typ1.clone()), (Some(e2.loc()), typ2.clone())],
                        vec![(None, t1.clone()), (None, t2.clone())],
                    )?
                };
                Ok(ExprAndMeta::new(
                    Self::infix_expr(op.clone(), expr1, expr2),
                    typ,
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(&cond, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) =
                    Self::from_block_stmt(consequence.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-expression",
                    vec![
                        (Some(cond.loc()), typ1.clone()),
                        (Some(consequence.loc(e.loc())), typ2.clone()),
                    ],
                    vec![(None, Typ::bool()), (None, Typ::unit())],
                )?;
                Ok(ExprAndMeta::new(
                    Self::if_expr(expr1, expr2),
                    Typ::unit(),
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfExpr {
                cond,
                consequence,
                alternative: Some(alt),
            } => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(&cond, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) =
                    Self::from_block_stmt(consequence.as_ref(), headers, ret, ctxt)?.split();
                let (expr3, calls3, typ3) =
                    Self::from_block_stmt(alt.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-else-expression",
                    vec![
                        (Some(cond.loc()), typ1.clone()),
                        (Some(consequence.loc(e.loc())), typ2.clone()),
                    ],
                    vec![(None, Typ::bool()), (Some(alt.loc(e.loc())), typ3.clone())],
                )?;
                Ok(ExprAndMeta::new(
                    Self::if_else_expr(expr1, expr2, expr3),
                    typ2.unify(&typ3),
                    vec![calls1, calls2, calls3],
                ))
            }
            parser::Expr::IfSomeMatchExpr {
                var,
                expr,
                consequence,
                alternative: None,
            } => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(&expr, headers, ret, ctxt)?.split();
                let typ1 = typ1.dest_option().map_err(|_| {
                    Error::from(format!("expecting option type in if-let at {}", e.loc()))
                })?;
                let id = var.id();
                let (expr2, calls2, typ2) = Self::from_block_stmt(
                    consequence.as_ref(),
                    headers,
                    ret,
                    &ctxt.add_var(id, &typ1),
                )?
                .split();
                Typ::type_check(
                    "if-let-expression",
                    vec![(Some(consequence.loc(e.loc())), typ2.clone())],
                    vec![(None, Typ::unit())],
                )?;
                Ok(ExprAndMeta::new(
                    Self::IfSomeMatchExpr {
                        expr: Box::new(expr1),
                        consequence: { Box::new(expr2.closure_expr(id)) },
                        alternative: None,
                    },
                    Typ::unit(),
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfSomeMatchExpr {
                var,
                expr,
                consequence,
                alternative: Some(alt),
            } => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(&expr, headers, ret, ctxt)?.split();
                let typ1 = typ1.dest_option().map_err(|_| {
                    Error::from(format!("expecting option type in if-let at {}", e.loc()))
                })?;
                let id = var.id();
                let (expr2, calls2, typ2) = Self::from_block_stmt(
                    consequence.as_ref(),
                    headers,
                    ret,
                    &ctxt.add_var(id, &typ1),
                )?
                .split();
                let (expr3, calls3, typ3) =
                    Self::from_block_stmt(alt.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-let-else-expression",
                    vec![(Some(consequence.loc(e.loc())), typ2.clone())],
                    vec![(Some(alt.loc(e.loc())), typ3.clone())],
                )?;
                Ok(ExprAndMeta::new(
                    Self::IfSomeMatchExpr {
                        expr: Box::new(expr1),
                        consequence: Box::new(expr2.closure_expr(id)),
                        alternative: Some(Box::new(expr3)),
                    },
                    typ2.unify(&typ3),
                    vec![calls1, calls2, calls3],
                ))
            }
            parser::Expr::IfMatchExpr {
                matches,
                consequence,
                alternative,
            } => {
                let expressions: Result<Vec<ExprAndMeta<FlatTyp, FlatLiteral>>, self::Error> = matches
                    .iter()
                    .map(|(e, _)| Self::from_loc_expr(e, headers, ret, ctxt))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let types = matches
                    .iter()
                    .map(|(e, _)| Some(e.loc()))
                    .zip(types.iter())
                    .map(|(loc, p)| (loc, p.clone()))
                    .collect();
                let expected: Vec<(_, Typ<FlatTyp>)> =
                    //matches.iter().map(|(_, p)| (None, p.typ())).collect();
                     matches.iter().map(|(_, p)| (None, p.typ().clone())).collect();
                Typ::type_check("if-match-expression", types, expected)?;
                let mut map = HashMap::new();
                let matches: Result<Vec<Pattern>, self::Error> = matches
                    .iter()
                    .map(|(e, p)| match p {
                        parser::Pattern::Regex(r) => {
                            let re = parser::PolicyRegex::from_pat(r)?;
                            for x in re.capture_names() {
                                if let Some(x) = x {
                                    let (k, v) = parser::Pat::strip_as(x);
                                    if map.insert(k.clone(), v).is_some() {
                                        return Err(Error::from(format!(
                                            r#"{}: repeated variable "{}"" in "if match""#,
                                            e.loc(),
                                            k
                                        )));
                                    }
                                }
                            }
                            Ok(Pattern::Regex(re))
                        }
                        parser::Pattern::Label(l) => {
                            for x in l.vars() {
                                if map.insert(x.clone(), parser::As::Str).is_some() {
                                    return Err(Error::from(format!(
                                        r#"{}: repeated variable "{}" in "if match""#,
                                        e.loc(),
                                        x
                                    )));
                                }
                            }
                            Ok(Pattern::Label(l.clone()))
                        },
                        parser::Pattern::Phantom(_) => unreachable!()
                    })
                    .collect();
                let matches = matches?;
                let mut extend_vars = ctxt.clone();
                for (v, a) in map.iter() {
                    extend_vars = extend_vars.add_var(
                        &v,
                        &(if *a == parser::As::I64 {
                            Typ::i64()
                        } else if *a == parser::As::Base64 {
                            Typ::data()
                        } else {
                            Typ::str()
                        }),
                    )
                }
                let (mut expr1, calls1, typ1) =
                    Self::from_block_stmt(consequence.as_ref(), headers, ret, &extend_vars)?
                        .split();
                let variables: Vec<String> = map.into_iter().map(|x| x.0).collect();
                for v in variables.iter().rev() {
                    expr1 = expr1.closure_expr(v)
                }
                calls.push(calls1);
                Ok(match alternative {
                    None => {
                        Typ::type_check(
                            "if-match-expression",
                            vec![(Some(consequence.loc(e.loc())), typ1.clone())],
                            vec![(None, Typ::unit())],
                        )?;
                        ExprAndMeta::new(
                            Self::IfMatchExpr {
                                variables,
                                matches: expressions.into_iter().zip(matches).collect(),
                                consequence: { Box::new(expr1) },
                                alternative: None,
                            },
                            Typ::unit(),
                            calls,
                        )
                    }
                    Some(a) => {
                        let (expr2, calls2, typ2) =
                            Self::from_block_stmt(a.as_ref(), headers, ret, ctxt)?.split();
                        Typ::type_check(
                            "if-match-else-expression",
                            vec![(Some(consequence.loc(e.loc())), typ1.clone())],
                            vec![(Some(a.loc(e.loc())), typ2.clone())],
                        )?;
                        calls.push(calls2);
                        ExprAndMeta::new(
                            Self::IfMatchExpr {
                                variables,
                                matches: expressions.into_iter().zip(matches).collect(),
                                consequence: { Box::new(expr1) },
                                alternative: Some(Box::new(expr2)),
                            },
                            typ1.unify(&typ2),
                            calls,
                        )
                    }
                })
            }
            parser::Expr::IterExpr {
                op,
                idents,
                expr,
                body,
                accumulator
            } => {
                let (expr1, calls1, typ1) = Self::from_loc_expr(expr, headers, ret, ctxt)?.split();
                let (vs, iter_vars) = match typ1 {
                    Typ::List(ref lty) => {
                        if idents.len() == 1 {
                            let id = idents[0].id();
                            (vec![id], ctxt.add_var(id, &lty))
                        } else {
                            match **lty {
                                Typ::Tuple(ref tys) if idents.len() == tys.len() => {
                                    let mut vs = Vec::new();
                                    let mut iter_vars = ctxt.clone();
                                    for (id, ty) in idents.iter().zip(tys) {
                                        let v = id.id();
                                        iter_vars = iter_vars.add_var(v, ty);
                                        vs.push(v)
                                    }
                                    (vs, iter_vars)
                                }
                                _ => {
                                    return Err(Error::from(format!(
                                        "{} over expression of type {} at {} ",
                                        op,
                                        typ1,
                                        e.loc()
                                    )))
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(Error::from(format!(
                            "{} over expression of type {} at {} ",
                            op,
                            typ1,
                            e.loc()
                        )))
                    }
                };
                
                //Adding a acc var inside the body block
                let iter_vars = &match accumulator { 
                    Some(acc) if *op == parser::Iter::Fold => {
                        let (_, _, acc_typ) = Self::from_loc_expr(acc, headers, ret, ctxt)?.split();
                        iter_vars.add_var("acc", &acc_typ.clone())
                    }
                    _ => iter_vars.clone(),
                };

                let (expr2, calls2, typ2) =
                    Self::from_block_stmt(body.as_ref(), headers, ret, &iter_vars)?.split();
                if *op == parser::Iter::FilterMap {
                    Typ::type_check(
                        "filter_map-expression",
                        vec![(Some(body.loc(e.loc())), typ2.clone())],
                        vec![(None, Typ::any_option())],
                    )?
                } 
                else if *op != parser::Iter::Map && *op != parser::Iter::ForEach && *op != parser::Iter::Fold{
                    Typ::type_check(
                        "all/any/filter-expression",
                        vec![(Some(body.loc(e.loc())), typ2.clone())],
                        vec![(None, Typ::bool())],
                    )?
                }

                match accumulator {
                    None => {
                        Ok(ExprAndMeta::new(
                            expr2.iter_expr(op, vs, expr1, None),
                            match op {
                                parser::Iter::All | parser::Iter::Any => Typ::bool(),
                                parser::Iter::Filter => typ1,
                                // type check above will ensure unwrap is successful
                                parser::Iter::FilterMap => Typ::List(Box::new(typ2.dest_option().unwrap())),
                                parser::Iter::ForEach => Typ::unit(),
                                parser::Iter::Fold =>{
                                    return Err(Error::from(format!(
                                        "{} can not be defined without an accumulator at {} ",
                                        op,
                                        e.loc()
                                    )))
                                },
                                parser::Iter::Map => Typ::List(Box::new(typ2)),
                            },
                            vec![calls1, calls2],
                        ))
                    },
                    Some(acc) if *op == parser::Iter::Fold => {
                        let (acc, acc_calls, acc_typ) = Self::from_loc_expr(acc, headers, ret, ctxt)?.split();
                        Typ::type_check(
                            "fold-expression",
                            vec![(Some(body.loc(e.loc())), typ2.clone())],
                            vec![(None, acc_typ.clone())],
                        )?;
                        Ok(ExprAndMeta::new(
                            expr2.iter_expr(op, vs, expr1, Some(acc)),
                            acc_typ,
                            vec![calls1, calls2, acc_calls],
                        ))
                    },
                    _ => return Err(Error::from(format!(
                        "{} can have an accumulator at {} ",
                        op,
                        e.loc()
                    )))
                }
            }
            parser::Expr::CallExpr {
                function,
                arguments,
                ..
            } if function == "option::Some" && arguments.len() == 1 => {
                let (expression, calls, typ) =
                    Self::from_loc_expr(arguments.get(0).unwrap(), headers, ret, ctxt)?.split();
                Ok(ExprAndMeta::new(
                    Self::call(function, vec![expression]),
                    Typ::Tuple(vec![typ]),
                    vec![calls],
                ))
            }
            parser::Expr::CallExpr {
                loc,
                function,
                arguments,
            } => {
                let expressions: Result<Vec<ExprAndMeta<FlatTyp, FlatLiteral>>, self::Error> = arguments
                    .iter()
                    .map(|e| Self::from_loc_expr(e, headers, ret, ctxt))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let function = &Headers::resolve(function, &types);
                if let Some((args, typ)) = headers.typ(function).map(types::Signature::split) {
                    // external functions *can* be declared so that they accept any argument,
                    // so only check the arguments when their types are declared
                    if let Some(ref args) = args {
                        let args = args.iter().map(|t| (None, t.clone())).collect();
                        let types = arguments
                            .iter()
                            .map(|e| Some(e.loc()))
                            .zip(types.iter())
                            .map(|(loc, t)| (loc, t.clone()))
                            .collect();
                        Typ::type_check(function, types, args)?
                    };
                    let typ = match function.as_str() {
                        "list::reduce" => {
                            types.iter().next().unwrap().dest_list().unwrap().option()
                        }
                        "list::difference" | "list::intersection" => {
                            types.iter().next().unwrap().to_owned()
                        }
                        _ => typ,
                    };
                    calls.push(
                        vec![Call {
                            name: function.to_string(),
                            loc: loc.clone(),
                        }]
                        .into_iter()
                        .collect(),
                    );
                    Ok(ExprAndMeta::new(
                        Self::CallExpr {
                            function: function.to_string(),
                            arguments: expressions,
                            is_async: typ == Typ::unit() && ctxt.async_tag(),
                        },
                        typ,
                        calls,
                    ))
                } else if let Ok(i) = function.parse::<usize>() {
                    match types.as_slice() {
                        [Typ::Tuple(ref l)] => {
                            if i < l.len() {
                                Ok(ExprAndMeta::new(
                                    Self::call(function, expressions),
                                    l.get(i).unwrap().clone(),
                                    calls,
                                ))
                            } else {
                                Err(Error::from(format!(
                                    "tuple index function \"{}\" called on tuple with just {} elements at {}",
                                    function,
                                    l.len(),
                                    e.loc()
                                )))
                            }
                        }
                        _ => Err(Error::from(format!(
                            "tuple index function \"{}\" called on non-tuple ({}) at {}",
                            function,
                            types
                                .iter()
                                .map(|t| t.to_string())
                                .collect::<Vec<String>>()
                                .join(","),
                            e.loc()
                        ))),
                    }
                } else {
                    Err(Error::from(format!(
                        "undeclared function \"{}\" at {}",
                        function,
                        e.loc()
                    )))
                }
            }
        }
    }

    fn from_block_stmt(
        block: parser::BlockStmtRef<FlatTyp, FlatLiteral>,
        headers: &Headers<FlatTyp>,
        ret: &mut ReturnType<FlatTyp>,
        ctxt: &Context<FlatTyp>,
    ) -> Result<ExprAndMeta<FlatTyp, FlatLiteral>, self::Error> {
        let ctxt = &ctxt.update_async_tag(block.async_tag());
        // println!("block: {:#?}\nasync is: {}", block, ctxt.async_tag());
        match block.split_first() {
            Some((stmt, rest)) => match stmt.stmt() {
                parser::Stmt::ReturnStmt(re) => {
                    if rest.is_empty() {
                        let (expr, calls, typ) =
                            Self::from_loc_expr(re, headers, ret, ctxt)?.split();
                        // need to type check typ against function return type
                        match ret.get() {
                            Some(rtype) => Typ::type_check(
                                "return",
                                vec![(Some(re.loc()), typ.clone())],
                                vec![(None, rtype.clone())],
                            )?,
                            None => ret.set(typ),
                        };
                        Ok(ExprAndMeta::new(
                            Self::BlockExpr(Block::Block, vec![Self::return_expr(expr)]),
                            Typ::rreturn(),
                            vec![calls],
                        ))
                    } else {
                        Err(Error::from(format!(
                            "unreachable code after return at {}",
                            stmt.loc()
                        )))
                    }
                }
                parser::Stmt::ExprStmt {
                    exp,
                    async_tag,
                    semi,
                } => {
                    let (expr1, calls1, typ1) = Self::from_loc_expr(
                        &parser::LocExpr::new(&stmt.loc(), exp),
                        headers,
                        ret,
                        &ctxt.update_async_tag(*async_tag),
                    )?
                    .split();
                    if *semi && !typ1.is_unit() {
                        println!(
                            "warning: result of expression is being ignored on {}",
                            stmt.loc()
                        )
                    };
                    if rest.is_empty() {
                        Ok(ExprAndMeta::new(
                            expr1,
                            if *semi { Typ::unit() } else { typ1 },
                            vec![calls1],
                        ))
                    } else {
                        if !semi {
                            return Err(Error::from(format!(
                                "missing semi-colon after expression at {}",
                                stmt.loc()
                            )));
                        };
                        let (expr2, calls2, typ2) =
                            Self::from_block_stmt(rest, headers, ret, ctxt)?.split();
                        match expr2 {
                            Self::BlockExpr(Block::Block, mut b) => {
                                let mut new_block = vec![expr1];
                                new_block.append(&mut b);
                                Ok(ExprAndMeta::new(
                                    Self::BlockExpr(Block::Block, new_block),
                                    typ2,
                                    vec![calls1, calls2],
                                ))
                            }
                            _ => Ok(ExprAndMeta::new(
                                Self::BlockExpr(Block::Block, vec![expr1, expr2]),
                                typ2,
                                vec![calls1, calls2],
                            )),
                        }
                    }
                }
                parser::Stmt::LetStmt(ids, le) => {
                    let (expr1, calls1, typ1) =
                        Self::from_loc_expr(&le, headers, ret, ctxt)?.split();
                    if ids.len() == 1 {
                        let id = ids[0].id();
                        let (expr2, calls2, typ2) =
                            Self::from_block_stmt(rest, headers, ret, &ctxt.add_var(id, &typ1))?
                                .split();
                        Ok(ExprAndMeta::new(
                            expr2.let_expr(vec![id], expr1),
                            typ2,
                            vec![calls1, calls2],
                        ))
                    } else {
                        match typ1 {
                            Typ::Tuple(ref tys) if ids.len() == tys.len() => {
                                let mut vs = Vec::new();
                                let mut let_vars = ctxt.clone();
                                for (id, ty) in ids.iter().zip(tys) {
                                    let v = id.id();
                                    let_vars = let_vars.add_var(v, ty);
                                    vs.push(v)
                                }
                                let (expr2, calls2, typ2) =
                                    Self::from_block_stmt(rest, headers, ret, &let_vars)?.split();
                                Ok(ExprAndMeta::new(
                                    expr2.let_expr(vs, expr1),
                                    typ2,
                                    vec![calls1, calls2],
                                ))
                            }
                            _ => Err(Error::from(format!(
                                "{} variables in let expression of type {} at {} ",
                                ids.len(),
                                typ1,
                                stmt.loc()
                            ))),
                        }
                    }
                }
            },
            None => Ok(ExprAndMeta::new(
                Self::BlockExpr(Block::Block, Vec::new()),
                Typ::unit(),
                vec![],
            )),
        }
    }

    pub fn from_string(buf: &str, headers: &Headers<FlatTyp>) -> Result<Self, self::Error> {
        let lex = lexer::lex(buf);
        let toks = lexer::Tokens::new(&lex);
        // println!("{}", toks);
        match parser::Parser::parse_block_stmt_eof(toks) {
            Ok((_rest, block)) => {
                // println!("{:#?}", block);
                Ok(
                    Self::check_from_block_stmt(block.as_ref(), headers, &Context::new(), None)?
                        .expr,
                )
            }
            Err(_) => match parser::Parser::parse_expr_eof(toks) {
                Ok((_rest, e)) => {
                    // println!("{:#?}", e);
                    Ok(Self::check_from_loc_expr(&e, headers, &Context::new())?.expr)
                }
                Err(nom::Err::Error((toks, _))) => {
                    Err(Error::from(format!("syntax error: {}", toks.tok[0])))
                }
                Err(err) => Err(Error::from(format!("{:?}", err))),
            },
        }
    }

    fn check_from_block_stmt(
        block: parser::BlockStmtRef<FlatTyp, FlatLiteral>,
        headers: &Headers<FlatTyp>,
        ctxt: &Context<FlatTyp>,
        name: Option<&str>,
    ) -> Result<ExprAndMeta<FlatTyp, FlatLiteral>, self::Error> {
        let mut ret : ReturnType<FlatTyp> = ReturnType::default();
        let em = Self::from_block_stmt(block, headers, &mut ret, ctxt)?;
        // check if type of "return" calls is type of statement
        if let Some(rtype) = ret.get() {
            Typ::type_check(
                name.unwrap_or("REPL"),
                vec![(None, em.typ.clone())],
                vec![(None, rtype.clone())],
            )?
        }
        // check if declared return type of function is type of statement
        if let Some(name) = name {
            Typ::type_check(
                name,
                vec![(None, em.typ.clone())],
                vec![(None, headers.return_typ(name)?)],
            )?
        }
        Ok(em)
    }

    pub fn from_decl<'a>(
        decl: &'a parser::FnDecl<FlatTyp, FlatLiteral>,
        headers: &'a Headers<FlatTyp>,
    ) -> Result<(&'a str, Self, Calls), Error> {
        let mut ctxt = Context::new();
        for a in decl.args().iter().rev() {
            ctxt = ctxt.add_var(a.name(), &Typ::from_parse(&a.typ)?)
        }
        let name = decl.name();
        let em = Self::check_from_block_stmt(decl.body().as_ref(), headers, &ctxt, Some(name))?;
        let mut e = em.expr;
        for a in decl.args().iter().rev() {
            e = e.closure_expr(a.name())
        }
        Ok((name, e, em.calls))
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> std::str::FromStr for Expr<FlatTyp, FlatLiteral> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s, &Headers::default())
    }
}
