/// policy language
use super::{externals, headers, lexer, literals, parser, types};
use futures::{future, Future};
use headers::Headers;
use literals::Literal;
use parser::{Infix, Prefix};
use petgraph::graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use types::Typ;

#[derive(Debug, Clone)]
pub struct Error(String);

impl std::error::Error for Error {}

impl Error {
    pub fn new<D: std::fmt::Display>(e: D) -> Error {
        Error(e.to_string())
    }
    pub fn from_display<D: std::fmt::Display>(e: D) -> Error {
        Error(e.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
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

impl<'a> From<types::Error<'a>> for Error {
    fn from(err: types::Error<'a>) -> Error {
        Error::new(err)
    }
}

impl From<externals::Error> for Error {
    fn from(err: externals::Error) -> Error {
        Error::from_display(err)
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
struct Call {
    loc: lexer::Loc,
    name: String,
}

type Calls = HashSet<Call>;

struct ExprAndMeta {
    expr: Expr,
    calls: Calls,
    typ: Typ,
}

impl ExprAndMeta {
    fn new(expr: Expr, typ: Typ, v: Vec<Calls>) -> ExprAndMeta {
        let mut calls = Calls::new();
        for c in v {
            calls.extend(c)
        }
        ExprAndMeta { expr, typ, calls }
    }
    fn split(self) -> (Expr, Calls, Typ) {
        (self.expr, self.calls, self.typ)
    }
    fn split_vec(v: Vec<ExprAndMeta>) -> (Vec<Expr>, Vec<Calls>, Vec<Typ>) {
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

#[derive(Default)]
pub struct ReturnType(Option<Typ>);

impl ReturnType {
    pub fn get(&self) -> Option<Typ> {
        self.0.clone()
    }
    pub fn clear(&mut self) {
        self.0 = None
    }
    pub fn set(&mut self, typ: Typ) {
        self.0 = Some(typ)
    }
}

#[derive(Clone)]
struct Context {
    variables: HashMap<String, Typ>,
    async_tag: bool,
}

impl Context {
    fn new() -> Context {
        Context {
            variables: HashMap::new(),
            async_tag: false,
        }
    }
    fn add_var(&self, name: &str, typ: &Typ) -> Self {
        let mut ctxt = self.clone();
        ctxt.variables.insert(name.to_string(), typ.to_owned());
        ctxt
    }
    fn update_async_tag(&self, b: bool) -> Self {
        let mut ctxt = self.clone();
        ctxt.async_tag = self.async_tag || b;
        ctxt
    }
    fn var(&self, name: &str) -> Option<Typ> {
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
pub enum Expr {
    Var(parser::Ident),
    BVar(parser::Ident, usize),
    LitExpr(Literal),
    ReturnExpr(Box<Expr>),
    PrefixExpr(Prefix, Box<Expr>),
    InfixExpr(Infix, Box<Expr>, Box<Expr>),
    BlockExpr(Block, Vec<Expr>),
    Let(Vec<String>, Box<Expr>, Box<Expr>),
    Iter(parser::Iter, Vec<String>, Box<Expr>, Box<Expr>),
    Closure(parser::Ident, Box<Expr>),
    IfExpr {
        cond: Box<Expr>,
        consequence: Box<Expr>,
        alternative: Option<Box<Expr>>,
    },
    IfMatchExpr {
        variables: Vec<String>,
        matches: Vec<(Expr, parser::PolicyRegex)>,
        consequence: Box<Expr>,
        alternative: Option<Box<Expr>>,
    },
    IfSomeMatchExpr {
        expr: Box<Expr>,
        consequence: Box<Expr>,
        alternative: Option<Box<Expr>>,
    },
    CallExpr {
        function: String,
        arguments: Vec<Expr>,
        is_async: bool,
    },
}

impl Default for Expr {
    fn default() -> Self {
        Expr::LitExpr(Literal::Unit)
    }
}

impl Expr {
    pub fn var(v: &str) -> Expr {
        Expr::Var(parser::Ident(v.to_string()))
    }
    pub fn unit() -> Expr {
        Expr::LitExpr(Literal::Unit)
    }
    pub fn i64(i: i64) -> Expr {
        Expr::LitExpr(Literal::Int(i))
    }
    pub fn f64(f: f64) -> Expr {
        Expr::LitExpr(Literal::Float(f))
    }
    pub fn bool(b: bool) -> Expr {
        Expr::LitExpr(Literal::Bool(b))
    }
    pub fn string(s: &str) -> Expr {
        Expr::LitExpr(Literal::Str(s.to_string()))
    }
    pub fn data(d: &[u8]) -> Expr {
        Expr::LitExpr(Literal::Data(d.to_vec()))
    }
    pub fn http_request(r: literals::HttpRequest) -> Expr {
        Expr::LitExpr(Literal::HttpRequest(r))
    }
    pub fn id(id: literals::ID) -> Expr {
        Expr::LitExpr(Literal::ID(id))
    }
    pub fn host(&self) -> Option<String> {
        match self {
            Expr::LitExpr(Literal::ID(id)) => id.host(),
            _ => None,
        }
    }
    pub fn none() -> Expr {
        Expr::LitExpr(Literal::none())
    }
    pub fn some(l: Literal) -> Expr {
        Expr::LitExpr(Literal::some(&l))
    }
    pub fn call(f: &str, arguments: Vec<Expr>) -> Expr {
        Expr::CallExpr {
            function: f.to_string(),
            arguments,
            is_async: false,
        }
    }
    pub fn return_expr(e: Expr) -> Expr {
        Expr::ReturnExpr(Box::new(e))
    }
    pub fn prefix_expr(p: Prefix, e: Expr) -> Expr {
        Expr::PrefixExpr(p, Box::new(e))
    }
    pub fn infix_expr(op: Infix, e1: Expr, e2: Expr) -> Expr {
        Expr::InfixExpr(op, Box::new(e1), Box::new(e2))
    }
    pub fn if_else_expr(b: Expr, e1: Expr, e2: Expr) -> Expr {
        Expr::IfExpr {
            cond: Box::new(b),
            consequence: Box::new(e1),
            alternative: Some(Box::new(e2)),
        }
    }
    pub fn if_expr(b: Expr, e: Expr) -> Expr {
        Expr::IfExpr {
            cond: Box::new(b),
            consequence: Box::new(e),
            alternative: None,
        }
    }
    fn abs(self, i: usize, v: &str) -> Expr {
        match self {
            Expr::BVar(_, _) | Expr::LitExpr(_) => self,
            Expr::Var(ref id) => {
                if id.0 == v {
                    Expr::BVar(id.to_owned(), i)
                } else {
                    self
                }
            }
            Expr::Let(l, e1, e2) => Expr::Let(l, Box::new(e1.abs(i, v)), Box::new(e2.abs(i, v))),
            Expr::Iter(op, l, e1, e2) => {
                Expr::Iter(op, l, Box::new(e1.abs(i, v)), Box::new(e2.abs(i, v)))
            }
            Expr::Closure(v2, e) => Expr::Closure(v2, Box::new(e.abs(i + 1, v))),
            Expr::ReturnExpr(e) => Expr::return_expr(e.abs(i, v)),
            Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.abs(i, v)),
            Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.abs(i, v), e2.abs(i, v)),
            Expr::BlockExpr(b, es) => {
                Expr::BlockExpr(b, es.into_iter().map(|e| e.abs(i, v)).collect())
            }
            Expr::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => Expr::if_expr(cond.abs(i, v), consequence.abs(i, v)),
            Expr::IfExpr {
                cond,
                consequence,
                alternative: Some(a),
            } => Expr::if_else_expr(cond.abs(i, v), consequence.abs(i, v), a.abs(i, v)),
            Expr::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => Expr::IfMatchExpr {
                variables,
                matches: matches.into_iter().map(|(e, p)| (e.abs(i, v), p)).collect(),
                consequence: Box::new(consequence.abs(i, v)),
                alternative: alternative.map(|e| Box::new(e.abs(i, v))),
            },
            Expr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => Expr::IfSomeMatchExpr {
                expr: Box::new(expr.abs(i, v)),
                consequence: Box::new(consequence.abs(i, v)),
                alternative: alternative.map(|e| Box::new(e.abs(i, v))),
            },
            Expr::CallExpr {
                function,
                arguments,
                is_async,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.abs(i, v)).collect(),
                is_async,
            },
        }
    }
    pub fn closure_expr(self, v: &str) -> Expr {
        if v == "_" {
            self
        } else {
            Expr::Closure(parser::Ident::from(v), Box::new(self.abs(0, v)))
        }
    }
    pub fn let_expr(self, v: Vec<&str>, e: Expr) -> Expr {
        let mut c = self;
        for s in v.iter().rev() {
            c = c.closure_expr(s)
        }
        Expr::Let(
            v.iter().map(|s| s.to_string()).collect(),
            Box::new(e),
            Box::new(c),
        )
    }
    pub fn iter_expr(self, op: &parser::Iter, v: Vec<&str>, e: Expr) -> Expr {
        let mut c = self;
        for s in v.iter().rev() {
            c = c.closure_expr(s)
        }
        Expr::Iter(
            op.clone(),
            v.iter().map(|s| s.to_string()).collect(),
            Box::new(e),
            Box::new(c),
        )
    }
    fn shift(self, i: usize, d: usize) -> Expr {
        if i == 0 {
            self
        } else {
            match self {
                Expr::Var(_) | Expr::LitExpr(_) => self,
                Expr::BVar(ref id, j) => {
                    if j >= d {
                        Expr::BVar(id.to_owned(), j + 1)
                    } else {
                        self
                    }
                }
                Expr::Let(l, e1, e2) => {
                    Expr::Let(l, Box::new(e1.shift(i, d)), Box::new(e2.shift(i, d)))
                }
                Expr::Iter(op, l, e1, e2) => {
                    Expr::Iter(op, l, Box::new(e1.shift(i, d)), Box::new(e2.shift(i, d)))
                }
                Expr::Closure(v, e) => Expr::Closure(v, Box::new(e.shift(i, d + 1))),
                Expr::ReturnExpr(e) => Expr::return_expr(e.shift(i, d)),
                Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.shift(i, d)),
                Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.shift(i, d), e2.shift(i, d)),
                Expr::BlockExpr(b, es) => {
                    Expr::BlockExpr(b, es.into_iter().map(|e| e.shift(i, d)).collect())
                }
                Expr::IfExpr {
                    cond,
                    consequence,
                    alternative: None,
                } => Expr::if_expr(cond.shift(i, d), consequence.shift(i, d)),
                Expr::IfExpr {
                    cond,
                    consequence,
                    alternative: Some(a),
                } => Expr::if_else_expr(cond.shift(i, d), consequence.shift(i, d), a.shift(i, d)),
                Expr::IfMatchExpr {
                    variables,
                    matches,
                    consequence,
                    alternative,
                } => Expr::IfMatchExpr {
                    variables,
                    matches: matches
                        .into_iter()
                        .map(|(e, p)| (e.shift(i, d), p))
                        .collect(),
                    consequence: Box::new(consequence.shift(i, d)),
                    alternative: alternative.map(|a| Box::new(a.shift(i, d))),
                },
                Expr::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => Expr::IfSomeMatchExpr {
                    expr: Box::new(expr.shift(i, d)),
                    consequence: Box::new(consequence.shift(i, d)),
                    alternative: alternative.map(|e| Box::new(e.shift(i, d))),
                },
                Expr::CallExpr {
                    function,
                    arguments,
                    is_async,
                } => Expr::CallExpr {
                    function,
                    arguments: arguments.into_iter().map(|a| a.shift(i, d)).collect(),
                    is_async,
                },
            }
        }
    }
    pub fn subst(self, i: usize, u: &Expr) -> Expr {
        match self {
            Expr::Var(_) | Expr::LitExpr(_) => self,
            Expr::BVar(ref id, j) => {
                if j < i {
                    self
                } else if j == i {
                    u.clone().shift(i, 0)
                } else {
                    Expr::BVar(id.to_owned(), j - 1)
                }
            }
            Expr::Let(l, e1, e2) => {
                Expr::Let(l, Box::new(e1.subst(i, u)), Box::new(e2.subst(i, u)))
            }
            Expr::Iter(op, l, e1, e2) => {
                Expr::Iter(op, l, Box::new(e1.subst(i, u)), Box::new(e2.subst(i, u)))
            }
            Expr::Closure(v, e) => Expr::Closure(v, Box::new(e.subst(i + 1, u))),
            Expr::ReturnExpr(e) => Expr::return_expr(e.subst(i, u)),
            Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.subst(i, u)),
            Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.subst(i, u), e2.subst(i, u)),
            Expr::BlockExpr(b, es) => {
                Expr::BlockExpr(b, es.into_iter().map(|e| e.subst(i, u)).collect())
            }
            Expr::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => Expr::if_expr(cond.subst(i, u), consequence.subst(i, u)),
            Expr::IfExpr {
                cond,
                consequence,
                alternative: Some(a),
            } => Expr::if_else_expr(cond.subst(i, u), consequence.subst(i, u), a.subst(i, u)),
            Expr::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => Expr::IfMatchExpr {
                variables,
                matches: matches
                    .into_iter()
                    .map(|(e, p)| (e.subst(i, u), p))
                    .collect(),
                consequence: Box::new(consequence.subst(i, u)),
                alternative: alternative.map(|a| Box::new(a.subst(i, u))),
            },
            Expr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => Expr::IfSomeMatchExpr {
                expr: Box::new(expr.subst(i, u)),
                consequence: Box::new(consequence.subst(i, u)),
                alternative: alternative.map(|e| Box::new(e.subst(i, u))),
            },
            Expr::CallExpr {
                function,
                arguments,
                is_async,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.subst(i, u)).collect(),
                is_async,
            },
        }
    }
    pub fn apply(self, u: &Expr) -> Result<Expr, self::Error> {
        match self {
            Expr::Closure(_, e) => Ok(e.subst(0, u)),
            _ => Err(Error::new("apply: expression is not a closure")),
        }
    }
    fn from_loc_expr(
        e: &parser::LocExpr,
        headers: &Headers,
        ret: &mut ReturnType,
        ctxt: &Context,
    ) -> Result<ExprAndMeta, Error> {
        match e.expr() {
            parser::Expr::IdentExpr(id) => match ctxt.var(&id.0) {
                Some(typ) => Ok(ExprAndMeta::new(Expr::var(&id.0), typ, vec![])),
                None => Err(Error::new(&format!(
                    "undeclared variable \"{}\" at {}",
                    id.0,
                    e.loc()
                ))),
            },
            parser::Expr::LitExpr(l) => {
                Ok(ExprAndMeta::new(Expr::LitExpr(l.clone()), l.typ(), vec![]))
            }
            parser::Expr::ListExpr(es) => {
                let mut exprs = Vec::new();
                let mut calls = Vec::new();
                let mut typ = Typ::Return;
                for e in es.iter() {
                    let (expr, call, ty) = Expr::from_loc_expr(&e, headers, ret, ctxt)?.split();
                    Typ::type_check("list", vec![(Some(e.loc()), &ty)], vec![(None, &typ)])?;
                    exprs.push(expr);
                    calls.push(call);
                    typ = typ.unify(&ty);
                }
                Ok(ExprAndMeta::new(
                    Expr::BlockExpr(Block::List, exprs),
                    Typ::List(Box::new(typ)),
                    calls,
                ))
            }
            parser::Expr::TupleExpr(es) => {
                let mut exprs = Vec::new();
                let mut calls = Vec::new();
                let mut typs = Vec::new();
                for e in es.iter() {
                    let (expr, call, ty) = Expr::from_loc_expr(&e, headers, ret, ctxt)?.split();
                    exprs.push(expr);
                    calls.push(call);
                    typs.push(ty);
                }
                Ok(ExprAndMeta::new(
                    Expr::BlockExpr(Block::Tuple, exprs),
                    Typ::Tuple(typs),
                    calls,
                ))
            }
            parser::Expr::PrefixExpr(p, e1) => {
                let (expr, calls, typ) = Expr::from_loc_expr(&e1, headers, ret, ctxt)?.split();
                let (t1, t2) = p.typ();
                Typ::type_check("prefix", vec![(Some(e1.loc()), &typ)], vec![(None, &t1)])?;
                Ok(ExprAndMeta::new(
                    Expr::prefix_expr(p.clone(), expr),
                    t2,
                    vec![calls],
                ))
            }
            parser::Expr::InfixExpr(op, e1, e2) => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&e1, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) = Expr::from_loc_expr(&e2, headers, ret, ctxt)?.split();
                let (t1, t2, typ) = op.typ();
                if t1 == Typ::Return {
                    if t2 == Typ::Return {
                        Typ::type_check(
                            "(in)equality",
                            vec![(Some(e1.loc()), &typ1)],
                            vec![(Some(e2.loc()), &typ2)],
                        )?
                    } else {
                        Typ::type_check(
                            "in",
                            vec![(Some(e1.loc()), &Typ::List(Box::new(typ1)))],
                            vec![(Some(e2.loc()), &typ2)],
                        )?
                    }
                } else {
                    Typ::type_check(
                        "infix",
                        vec![(Some(e1.loc()), &typ1), (Some(e2.loc()), &typ2)],
                        vec![(None, &t1), (None, &t2)],
                    )?
                };
                Ok(ExprAndMeta::new(
                    Expr::infix_expr(op.clone(), expr1, expr2),
                    typ,
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfExpr {
                cond,
                consequence,
                alternative: None,
            } => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&cond, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) =
                    Expr::from_block_stmt(consequence.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-expression",
                    vec![
                        (Some(cond.loc()), &typ1),
                        (Some(consequence.loc(e.loc())), &typ2),
                    ],
                    vec![(None, &Typ::Bool), (None, &Typ::Unit)],
                )?;
                Ok(ExprAndMeta::new(
                    Expr::if_expr(expr1, expr2),
                    Typ::Unit,
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfExpr {
                cond,
                consequence,
                alternative: Some(alt),
            } => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&cond, headers, ret, ctxt)?.split();
                let (expr2, calls2, typ2) =
                    Expr::from_block_stmt(consequence.as_ref(), headers, ret, ctxt)?.split();
                let (expr3, calls3, typ3) =
                    Expr::from_block_stmt(alt.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-else-expression",
                    vec![
                        (Some(cond.loc()), &typ1),
                        (Some(consequence.loc(e.loc())), &typ2),
                    ],
                    vec![(None, &Typ::Bool), (Some(alt.loc(e.loc())), &typ3)],
                )?;
                Ok(ExprAndMeta::new(
                    Expr::if_else_expr(expr1, expr2, expr3),
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
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&expr, headers, ret, ctxt)?.split();
                let typ1 = typ1.dest_option().map_err(|_| {
                    Error::new(format!("expecting option type in if-let at {}", e.loc()))
                })?;
                let id = var.id();
                let (expr2, calls2, typ2) = Expr::from_block_stmt(
                    consequence.as_ref(),
                    headers,
                    ret,
                    &ctxt.add_var(id, &typ1),
                )?
                .split();
                Typ::type_check(
                    "if-let-expression",
                    vec![(Some(consequence.loc(e.loc())), &typ2)],
                    vec![(None, &Typ::Unit)],
                )?;
                Ok(ExprAndMeta::new(
                    Expr::IfSomeMatchExpr {
                        expr: Box::new(expr1),
                        consequence: { Box::new(expr2.closure_expr(id)) },
                        alternative: None,
                    },
                    Typ::Unit,
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::IfSomeMatchExpr {
                var,
                expr,
                consequence,
                alternative: Some(alt),
            } => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&expr, headers, ret, ctxt)?.split();
                let typ1 = typ1.dest_option().map_err(|_| {
                    Error::new(format!("expecting option type in if-let at {}", e.loc()))
                })?;
                let id = var.id();
                let (expr2, calls2, typ2) = Expr::from_block_stmt(
                    consequence.as_ref(),
                    headers,
                    ret,
                    &ctxt.add_var(id, &typ1),
                )?
                .split();
                let (expr3, calls3, typ3) =
                    Expr::from_block_stmt(alt.as_ref(), headers, ret, ctxt)?.split();
                Typ::type_check(
                    "if-let-else-expression",
                    vec![(Some(consequence.loc(e.loc())), &typ2)],
                    vec![(Some(alt.loc(e.loc())), &typ3)],
                )?;
                Ok(ExprAndMeta::new(
                    Expr::IfSomeMatchExpr {
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
                let expressions: Result<Vec<ExprAndMeta>, self::Error> = matches
                    .iter()
                    .map(|(e, _)| Expr::from_loc_expr(e, headers, ret, ctxt))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let len = types.len();
                let types = matches
                    .iter()
                    .map(|(e, _)| Some(e.loc()))
                    .zip(types.iter())
                    .collect();
                Typ::type_check("if-match-expression", types, vec![(None, &Typ::Str); len])?;
                let mut set = HashSet::new();
                let matches: Result<Vec<parser::PolicyRegex>, self::Error> = matches
                    .iter()
                    .map(|(e, p)| {
                        let re = parser::PolicyRegex::from_pat(p)?;
                        let cap_names: HashSet<(String, parser::As)> =
                            re.capture_names()
                                .filter_map(|x| x.map(parser::Pat::strip_as))
                                .collect();
                        if set.is_disjoint(&cap_names) {
                            set.extend(cap_names);
                            Ok(re)
                        } else {
                            Err(Error::new(&format!(
                                "{}: repeated variable(s) in \"if match\"",
                                e.loc()
                            )))
                        }
                    })
                    .collect();
                let vs: Vec<(String, parser::As)> = set.into_iter().collect();
                let mut extend_vars = ctxt.clone();
                for (v, a) in vs.iter() {
                    extend_vars = extend_vars.add_var(
                        &v,
                        &(if *a == parser::As::I64 {
                            Typ::I64
                        } else if *a == parser::As::Base64 {
                            Typ::Data
                        } else {
                            Typ::Str
                        }),
                    )
                }
                let (mut expr1, calls1, typ1) =
                    Expr::from_block_stmt(consequence.as_ref(), headers, ret, &extend_vars)?
                        .split();
                let vs: Vec<String> = vs.into_iter().map(|x| x.0).collect();
                for v in vs.iter().rev() {
                    expr1 = expr1.closure_expr(v)
                }
                calls.push(calls1);
                Ok(match alternative {
                    None => {
                        Typ::type_check(
                            "if-match-expression",
                            vec![(Some(consequence.loc(e.loc())), &typ1)],
                            vec![(None, &Typ::Unit)],
                        )?;
                        ExprAndMeta::new(
                            Expr::IfMatchExpr {
                                variables: vs.clone(),
                                matches: expressions.into_iter().zip(matches?).collect(),
                                consequence: { Box::new(expr1) },
                                alternative: None,
                            },
                            Typ::Unit,
                            calls,
                        )
                    }
                    Some(a) => {
                        let (expr2, calls2, typ2) =
                            Expr::from_block_stmt(a.as_ref(), headers, ret, ctxt)?.split();
                        Typ::type_check(
                            "if-match-else-expression",
                            vec![(Some(consequence.loc(e.loc())), &typ1)],
                            vec![(Some(a.loc(e.loc())), &typ2)],
                        )?;
                        calls.push(calls2);
                        ExprAndMeta::new(
                            Expr::IfMatchExpr {
                                variables: vs.clone(),
                                matches: expressions.into_iter().zip(matches?).collect(),
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
            } => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(expr, headers, ret, ctxt)?.split();
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
                                    return Err(Error::new(&format!(
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
                        return Err(Error::new(&format!(
                            "{} over expression of type {} at {} ",
                            op,
                            typ1,
                            e.loc()
                        )))
                    }
                };
                let (expr2, calls2, typ2) =
                    Expr::from_block_stmt(body.as_ref(), headers, ret, &iter_vars)?.split();
                if *op == parser::Iter::FilterMap {
                    Typ::type_check(
                        "filter_map-expression",
                        vec![(Some(body.loc(e.loc())), &typ2)],
                        vec![(None, &Typ::any_option())],
                    )?
                } else if *op != parser::Iter::Map && *op != parser::Iter::ForEach {
                    Typ::type_check(
                        "all/any/filter-expression",
                        vec![(Some(body.loc(e.loc())), &typ2)],
                        vec![(None, &Typ::Bool)],
                    )?
                }
                Ok(ExprAndMeta::new(
                    expr2.iter_expr(op, vs, expr1),
                    match op {
                        parser::Iter::All | parser::Iter::Any => Typ::Bool,
                        parser::Iter::Filter => typ1,
                        // type check above will ensure unwrap is successful
                        parser::Iter::FilterMap => Typ::List(Box::new(typ2.dest_option().unwrap())),
                        parser::Iter::ForEach => Typ::Unit,
                        parser::Iter::Map => Typ::List(Box::new(typ2)),
                    },
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::CallExpr {
                function,
                arguments,
                ..
            } if function == "option::Some" && arguments.len() == 1 => {
                let (expression, calls, typ) =
                    Expr::from_loc_expr(arguments.get(0).unwrap(), headers, ret, ctxt)?.split();
                Ok(ExprAndMeta::new(
                    Expr::call(function, vec![expression]),
                    Typ::Tuple(vec![typ]),
                    vec![calls],
                ))
            }
            parser::Expr::CallExpr {
                loc,
                function,
                arguments,
            } => {
                let expressions: Result<Vec<ExprAndMeta>, self::Error> = arguments
                    .iter()
                    .map(|e| Expr::from_loc_expr(e, headers, ret, ctxt))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let function = &Headers::resolve(function, &types);
                if let Some((args, typ)) = headers.typ(function).map(types::Signature::split) {
                    // external functions *can* be declared so that they accept any argument,
                    // so only check the arguments when their types are declared
                    if let Some(ref args) = args {
                        let args = args.iter().map(|t| (None, t)).collect();
                        let types = arguments
                            .iter()
                            .map(|e| Some(e.loc()))
                            .zip(types.iter())
                            .collect();
                        Typ::type_check(function, types, args)?
                    };
                    let typ = if function == "list::reduce" {
                        types.iter().next().unwrap().dest_list().unwrap().option()
                    } else {
                        typ
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
                        Expr::CallExpr {
                            function: function.to_string(),
                            arguments: expressions,
                            is_async: typ == Typ::Unit && ctxt.async_tag(),
                        },
                        typ,
                        calls,
                    ))
                } else if let Ok(i) = function.parse::<usize>() {
                    match types.as_slice() {
                        [Typ::Tuple(ref l)] => {
                            if i < l.len() {
                                Ok(ExprAndMeta::new(
                                    Expr::call(function, expressions),
                                    l.get(i).unwrap().clone(),
                                    calls,
                                ))
                            } else {
                                Err(Error::new(&format!(
                                    "tuple index function \"{}\" called on tuple with just {} elements at {}",
                                    function,
                                    l.len(),
                                    e.loc()
                                )))
                            }
                        }
                        _ => Err(Error::new(&format!(
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
                    Err(Error::new(&format!(
                        "undeclared function \"{}\" at {}",
                        function,
                        e.loc()
                    )))
                }
            }
        }
    }
    fn from_block_stmt(
        block: parser::BlockStmtRef,
        headers: &Headers,
        ret: &mut ReturnType,
        ctxt: &Context,
    ) -> Result<ExprAndMeta, self::Error> {
        let ctxt = &ctxt.update_async_tag(block.async_tag());
        // println!("block: {:#?}\nasync is: {}", block, ctxt.async_tag());
        match block.split_first() {
            Some((stmt, rest)) => match stmt.stmt() {
                parser::Stmt::ReturnStmt(re) => {
                    if rest.is_empty() {
                        let (expr, calls, typ) =
                            Expr::from_loc_expr(re, headers, ret, ctxt)?.split();
                        // need to type check typ against function return type
                        match ret.get() {
                            Some(rtype) => Typ::type_check(
                                "return",
                                vec![(Some(re.loc()), &typ)],
                                vec![(None, &rtype)],
                            )?,
                            None => ret.set(typ),
                        };
                        Ok(ExprAndMeta::new(
                            Expr::BlockExpr(Block::Block, vec![Expr::return_expr(expr)]),
                            Typ::Return,
                            vec![calls],
                        ))
                    } else {
                        Err(Error::new(&format!(
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
                    let (expr1, calls1, typ1) = Expr::from_loc_expr(
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
                            if *semi { Typ::Unit } else { typ1 },
                            vec![calls1],
                        ))
                    } else {
                        if !semi {
                            return Err(Error::new(&format!(
                                "missing semi-colon after expression at {}",
                                stmt.loc()
                            )));
                        };
                        let (expr2, calls2, typ2) =
                            Expr::from_block_stmt(rest, headers, ret, ctxt)?.split();
                        match expr2 {
                            Expr::BlockExpr(Block::Block, mut b) => {
                                let mut new_block = vec![expr1];
                                new_block.append(&mut b);
                                Ok(ExprAndMeta::new(
                                    Expr::BlockExpr(Block::Block, new_block),
                                    typ2,
                                    vec![calls1, calls2],
                                ))
                            }
                            _ => Ok(ExprAndMeta::new(
                                Expr::BlockExpr(Block::Block, vec![expr1, expr2]),
                                typ2,
                                vec![calls1, calls2],
                            )),
                        }
                    }
                }
                parser::Stmt::LetStmt(ids, le) => {
                    let (expr1, calls1, typ1) =
                        Expr::from_loc_expr(&le, headers, ret, ctxt)?.split();
                    if ids.len() == 1 {
                        let id = ids[0].id();
                        let (expr2, calls2, typ2) =
                            Expr::from_block_stmt(rest, headers, ret, &ctxt.add_var(id, &typ1))?
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
                                    Expr::from_block_stmt(rest, headers, ret, &let_vars)?.split();
                                Ok(ExprAndMeta::new(
                                    expr2.let_expr(vs, expr1),
                                    typ2,
                                    vec![calls1, calls2],
                                ))
                            }
                            _ => Err(Error::new(&format!(
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
                Expr::BlockExpr(Block::Block, Vec::new()),
                Typ::Unit,
                vec![],
            )),
        }
    }
    fn check_from_loc_expr(
        e: &parser::LocExpr,
        headers: &Headers,
        ctxt: &Context,
    ) -> Result<ExprAndMeta, Error> {
        let mut ret = ReturnType::default();
        let em = Expr::from_loc_expr(e, headers, &mut ret, ctxt)?;
        if let Some(rtype) = ret.get() {
            Typ::type_check("REPL", vec![(None, &em.typ)], vec![(None, &rtype)])?
        }
        Ok(em)
    }
    fn check_from_block_stmt(
        block: parser::BlockStmtRef,
        headers: &Headers,
        ctxt: &Context,
        name: Option<&str>,
    ) -> Result<ExprAndMeta, self::Error> {
        let mut ret = ReturnType::default();
        let em = Expr::from_block_stmt(block, headers, &mut ret, ctxt)?;
        // check if type of "return" calls is type of statement
        if let Some(rtype) = ret.get() {
            Typ::type_check(
                name.unwrap_or("REPL"),
                vec![(None, &em.typ)],
                vec![(None, &rtype)],
            )?
        }
        // check if declared return type of function is type of statement
        if let Some(name) = name {
            Typ::type_check(
                name,
                vec![(None, &em.typ)],
                vec![(None, &headers.return_typ(name)?)],
            )?
        }
        Ok(em)
    }
    fn from_decl<'a>(
        decl: &'a parser::FnDecl,
        headers: &'a Headers,
    ) -> Result<(&'a str, Expr, Calls), Error> {
        let mut ctxt = Context::new();
        for a in decl.args().iter().rev() {
            ctxt = ctxt.add_var(a.name(), &a.typ()?)
        }
        let name = decl.name();
        let em = Expr::check_from_block_stmt(decl.body().as_ref(), headers, &ctxt, Some(name))?;
        let mut e = em.expr;
        for a in decl.args().iter().rev() {
            e = e.closure_expr(a.name())
        }
        Ok((name, e, em.calls))
    }
    pub fn from_string(buf: &str, headers: &Headers) -> Result<Expr, self::Error> {
        let lex = lexer::lex(buf);
        let toks = lexer::Tokens::new(&lex);
        // println!("{}", toks);
        match parser::parse_block_stmt_eof(toks) {
            Ok((_rest, block)) => {
                // println!("{:#?}", block);
                Ok(
                    Expr::check_from_block_stmt(block.as_ref(), headers, &Context::new(), None)?
                        .expr,
                )
            }
            Err(_) => match parser::parse_expr_eof(toks) {
                Ok((_rest, e)) => {
                    // println!("{:#?}", e);
                    Ok(Expr::check_from_loc_expr(&e, headers, &Context::new())?.expr)
                }
                Err(nom::Err::Error((toks, _))) => {
                    Err(self::Error(format!("syntax error: {}", toks.tok[0])))
                }
                Err(err) => Err(self::Error(format!("{:?}", err))),
            },
        }
    }
}

impl std::str::FromStr for Expr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s, &Headers::default())
    }
}

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

impl Program {
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
                            Err(self::Error(format!("{}\nsee: {}", s, toks.tok[0])))
                        }
                        Err(e) => Err(self::Error(format!("{}\n{:?}", s, e))),
                    }
                }
                Err(nom::Err::Error((toks, _))) => Err(self::Error(format!(
                    "syntax error in function header, starting: {}",
                    toks.tok[0]
                ))),
                Err(e) => Err(self::Error(format!("{:?}", e))),
            },
            Err(e) => Err(self::Error(format!("{:?}", e))),
        }
    }
}
