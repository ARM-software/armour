/// policy language
use super::{externals, headers, lexer, literals, parser, types};
use headers::Headers;
use literals::Literal;
use parser::{Infix, Prefix};
use petgraph::graph;
use std::collections::{HashMap, HashSet};
use std::fmt;
use types::Typ;

#[derive(Debug, Clone)]
pub struct Error(String);

impl Error {
    pub fn new(e: &str) -> Error {
        Error(e.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(_: std::num::ParseIntError) -> Error {
        Error::new("failed to parse i64")
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::new(&format!("{}", err))
    }
}

impl<'a> From<types::Error<'a>> for Error {
    fn from(err: types::Error<'a>) -> Error {
        Error::new(&format!("{}", err))
    }
}

impl From<externals::Error> for Error {
    fn from(err: externals::Error) -> Error {
        Error::new(&format!("Externals error: {:?}", err))
    }
}

impl From<headers::Error> for Error {
    fn from(err: headers::Error) -> Error {
        match err {
            headers::Error(s) => Error(s),
        }
    }
}

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Error {
        Error(format!("base 64 error: {}", err.to_string()))
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

#[derive(Clone)]
struct Vars {
    variables: HashMap<String, Typ>,
}

impl Vars {
    fn new() -> Vars {
        Vars {
            variables: HashMap::new(),
        }
    }
    fn add_var(&self, name: &str, typ: &Typ) -> Vars {
        let mut variables = self.variables.clone();
        variables.insert(name.to_string(), typ.to_owned());
        Vars { variables }
    }
    fn get_var(&self, name: &str) -> Option<Typ> {
        self.variables.get(name).cloned()
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Block {
    List,
    Tuple,
    Block,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Expr {
    Var(parser::Ident),
    BVar(usize),
    LitExpr(Literal),
    ReturnExpr(Box<Expr>),
    PrefixExpr(Prefix, Box<Expr>),
    InfixExpr(Infix, Box<Expr>, Box<Expr>),
    BlockExpr(Block, Vec<Expr>),
    Let(Vec<String>, Box<Expr>, Box<Expr>),
    Iter(parser::Iter, Vec<String>, Box<Expr>, Box<Expr>),
    Closure(Box<Expr>),
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
    CallExpr {
        function: String,
        arguments: Vec<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Var(id) => write!(f, r#"var "{}""#, id.0),
            Expr::BVar(i) => write!(f, "bvar {}", i),
            Expr::LitExpr(l) => write!(f, "{}", l),
            Expr::Let(_, _, _) => write!(f, "let <..> = <..>; <..>"),
            Expr::Iter(op, _, _, _) => write!(f, "{} <..> in <..> {{<..>}}", op),
            Expr::Closure(_) => write!(f, "lambda <..>"),
            Expr::ReturnExpr(_) => write!(f, "return <..>"),
            Expr::PrefixExpr(p, _) => write!(f, "{:?} <..>", p),
            Expr::InfixExpr(op, _, _) => write!(f, "{:?} <..>", op),
            Expr::BlockExpr(Block::Block, _) => write!(f, "{{<..>}}"),
            Expr::BlockExpr(Block::List, _) => write!(f, "[<..>]"),
            Expr::BlockExpr(Block::Tuple, _) => write!(f, "(<..>)"),
            Expr::IfExpr {
                alternative: None, ..
            } => write!(f, "if <..> {{<..>}}"),
            Expr::IfExpr {
                alternative: Some(_),
                ..
            } => write!(f, "if <..> {{<..>}} else {{<..>}}"),
            Expr::IfMatchExpr {
                alternative: None, ..
            } => write!(f, "if match <..> {{<..>}}"),
            Expr::IfMatchExpr {
                alternative: Some(_),
                ..
            } => write!(f, "if match <..> {{<..>}} else {{<..>}}"),
            Expr::CallExpr { function, .. } => write!(f, "{}(<..>)", function),
        }
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
        Expr::LitExpr(Literal::IntLiteral(i))
    }
    pub fn f64(f: f64) -> Expr {
        Expr::LitExpr(Literal::FloatLiteral(f))
    }
    pub fn bool(b: bool) -> Expr {
        Expr::LitExpr(Literal::BoolLiteral(b))
    }
    pub fn string(s: &str) -> Expr {
        Expr::LitExpr(Literal::StringLiteral(s.to_string()))
    }
    pub fn data(d: &[u8]) -> Expr {
        Expr::LitExpr(Literal::DataLiteral(d.to_vec()))
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
            Expr::Var(ref id) => {
                if id.0 == v {
                    Expr::BVar(i)
                } else {
                    self
                }
            }
            Expr::Let(l, e1, e2) => Expr::Let(l, Box::new(e1.abs(i, v)), Box::new(e2.abs(i, v))),
            Expr::Iter(op, l, e1, e2) => {
                Expr::Iter(op, l, Box::new(e1.abs(i, v)), Box::new(e2.abs(i, v)))
            }
            Expr::Closure(e) => Expr::Closure(Box::new(e.abs(i + 1, v))),
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
            Expr::CallExpr {
                function,
                arguments,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.abs(i, v)).collect(),
            },
            _ => self, // BVar, LitExpr
        }
    }
    pub fn closure_expr(self, v: &str) -> Expr {
        if v == "_" {
            self
        } else {
            Expr::Closure(Box::new(self.abs(0, v)))
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
                Expr::BVar(j) => {
                    if j >= d {
                        Expr::BVar(j + 1)
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
                Expr::Closure(e) => Expr::Closure(Box::new(e.shift(i, d + 1))),
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
                Expr::CallExpr {
                    function,
                    arguments,
                } => Expr::CallExpr {
                    function,
                    arguments: arguments.into_iter().map(|a| a.shift(i, d)).collect(),
                },
                _ => self, // Var, LitExpr
            }
        }
    }
    pub fn subst(self, i: usize, u: &Expr) -> Expr {
        match self {
            Expr::BVar(j) => {
                if j < i {
                    self
                } else if j == i {
                    u.clone().shift(i, 0)
                } else {
                    Expr::BVar(j - 1)
                }
            }
            Expr::Let(l, e1, e2) => {
                Expr::Let(l, Box::new(e1.subst(i, u)), Box::new(e2.subst(i, u)))
            }
            Expr::Iter(op, l, e1, e2) => {
                Expr::Iter(op, l, Box::new(e1.subst(i, u)), Box::new(e2.subst(i, u)))
            }
            Expr::Closure(e) => Expr::Closure(Box::new(e.subst(i + 1, u))),
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
            Expr::CallExpr {
                function,
                arguments,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.subst(i, u)).collect(),
            },
            _ => self, // Var, LitExpr
        }
    }
    pub fn apply(self, u: &Expr) -> Result<Expr, self::Error> {
        match self {
            Expr::Closure(e) => Ok(e.subst(0, u)),
            _ => Err(Error::new("apply: expression is not a closure")),
        }
    }
    fn block_stmt_loc(v: &parser::BlockStmt, default: lexer::Loc) -> lexer::Loc {
        v.get(0).map_or(default, |s| s.loc().clone())
    }
    fn from_loc_expr(
        e: &parser::LocExpr,
        headers: &mut Headers,
        vars: &Vars,
    ) -> Result<ExprAndMeta, Error> {
        match e.expr() {
            parser::Expr::IdentExpr(id) => match vars.get_var(&id.0) {
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
                    let (expr, call, ty) = Expr::from_loc_expr(&e, headers, vars)?.split();
                    Typ::type_check("list", vec![(Some(e.loc()), &ty)], vec![(None, &typ)])?;
                    exprs.push(expr);
                    calls.push(call);
                    typ = typ.pick(&ty);
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
                    let (expr, call, ty) = Expr::from_loc_expr(&e, headers, vars)?.split();
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
                let (expr, calls, typ) = Expr::from_loc_expr(&e1, headers, vars)?.split();
                let (t1, t2) = p.typ();
                Typ::type_check("prefix", vec![(Some(e1.loc()), &typ)], vec![(None, &t1)])?;
                Ok(ExprAndMeta::new(
                    Expr::prefix_expr(p.clone(), expr),
                    t2,
                    vec![calls],
                ))
            }
            parser::Expr::InfixExpr(op, e1, e2) => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&e1, headers, vars)?.split();
                let (expr2, calls2, typ2) = Expr::from_loc_expr(&e2, headers, vars)?.split();
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
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&cond, headers, vars)?.split();
                let (expr2, calls2, typ2) =
                    Expr::from_block_stmt(consequence, headers, vars)?.split();
                Typ::type_check(
                    "if-expression",
                    vec![
                        (Some(cond.loc()), &typ1),
                        (Some(Expr::block_stmt_loc(consequence, e.loc())), &typ2),
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
                let (expr1, calls1, typ1) = Expr::from_loc_expr(&cond, headers, vars)?.split();
                let (expr2, calls2, typ2) =
                    Expr::from_block_stmt(consequence, headers, vars)?.split();
                let (expr3, calls3, typ3) = Expr::from_block_stmt(alt, headers, vars)?.split();
                Typ::type_check(
                    "if-else-expression",
                    vec![
                        (Some(cond.loc()), &typ1),
                        (Some(Expr::block_stmt_loc(consequence, e.loc())), &typ2),
                    ],
                    vec![
                        (None, &Typ::Bool),
                        (Some(Expr::block_stmt_loc(alt, e.loc())), &typ3),
                    ],
                )?;
                Ok(ExprAndMeta::new(
                    Expr::if_else_expr(expr1, expr2, expr3),
                    typ2.pick(&typ3),
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
                    .map(|(e, _)| Expr::from_loc_expr(e, headers, vars))
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
                            re.0.capture_names()
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
                let mut extend_vars = vars.clone();
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
                    Expr::from_block_stmt(consequence, headers, &extend_vars)?.split();
                let vs: Vec<String> = vs.into_iter().map(|x| x.0).collect();
                for v in vs.iter().rev() {
                    expr1 = expr1.closure_expr(v)
                }
                calls.push(calls1);
                Ok(match alternative {
                    None => {
                        Typ::type_check(
                            "if-match-expression",
                            vec![(Some(Expr::block_stmt_loc(consequence, e.loc())), &typ1)],
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
                            Expr::from_block_stmt(a, headers, vars)?.split();
                        Typ::type_check(
                            "if-match-else-expression",
                            vec![(Some(Expr::block_stmt_loc(consequence, e.loc())), &typ1)],
                            vec![(Some(Expr::block_stmt_loc(a, e.loc())), &typ2)],
                        )?;
                        calls.push(calls2);
                        ExprAndMeta::new(
                            Expr::IfMatchExpr {
                                variables: vs.clone(),
                                matches: expressions.into_iter().zip(matches?).collect(),
                                consequence: { Box::new(expr1) },
                                alternative: Some(Box::new(expr2)),
                            },
                            typ1,
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
                let (expr1, calls1, typ1) = Expr::from_loc_expr(expr, headers, vars)?.split();
                let (vs, iter_vars) = match typ1 {
                    Typ::List(ref lty) => {
                        if idents.len() == 1 {
                            let id = idents[0].id();
                            (vec![id], vars.add_var(id, &lty))
                        } else {
                            match **lty {
                                Typ::Tuple(ref tys) if idents.len() == tys.len() => {
                                    let mut vs = Vec::new();
                                    let mut iter_vars = vars.clone();
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
                    Expr::from_block_stmt(body, headers, &iter_vars)?.split();
                if *op != parser::Iter::Map {
                    Typ::type_check(
                        "all/any/filter-expression",
                        vec![(Some(Expr::block_stmt_loc(body, e.loc())), &typ2)],
                        vec![(None, &Typ::Bool)],
                    )?
                }
                Ok(ExprAndMeta::new(
                    expr2.iter_expr(op, vs, expr1),
                    if *op == parser::Iter::Map {
                        Typ::List(Box::new(typ2))
                    } else if *op == parser::Iter::Filter {
                        typ1
                    } else {
                        Typ::Bool
                    },
                    vec![calls1, calls2],
                ))
            }
            parser::Expr::CallExpr {
                loc,
                function,
                arguments,
            } => {
                let expressions: Result<Vec<ExprAndMeta>, self::Error> = arguments
                    .iter()
                    .map(|e| Expr::from_loc_expr(e, headers, vars))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let function = &Headers::resolve(function, &types);
                if let Some((args, typ)) = headers.typ(function) {
                    let args = args.iter().map(|t| (None, t)).collect();
                    let types = arguments
                        .iter()
                        .map(|e| Some(e.loc()))
                        .zip(types.iter())
                        .collect();
                    Typ::type_check(function, types, args)?;
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
                        },
                        typ,
                        calls,
                    ))
                } else if let Ok(i) = function.parse::<usize>() {
                    match types.as_slice() {
                        &[Typ::Tuple(ref l)] => {
                            if i < l.len() {
                                Ok(ExprAndMeta::new(
                                    Expr::CallExpr {
                                        function: function.to_string(),
                                        arguments: expressions,
                                    },
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
        block: &[parser::LocStmt],
        headers: &mut Headers,
        vars: &Vars,
    ) -> Result<ExprAndMeta, self::Error> {
        match block.split_first() {
            Some((stmt, rest)) => match stmt.stmt() {
                parser::Stmt::ReturnStmt(re) => {
                    if rest.len() == 0 {
                        let (expr, calls, typ) = Expr::from_loc_expr(re, headers, vars)?.split();
                        // need to type check typ against function return type
                        match headers.return_typ() {
                            Some(rtype) => Typ::type_check(
                                "return",
                                vec![(Some(re.loc()), &typ)],
                                vec![(None, &rtype)],
                            )?,
                            None => headers.set_return_typ(typ),
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
                parser::Stmt::ExprStmt(se, has_semi) => {
                    let (expr1, calls1, typ1) =
                        Expr::from_loc_expr(&parser::LocExpr::new(stmt.loc(), se), headers, vars)?
                            .split();
                    if *has_semi && !typ1.is_unit() {
                        println!(
                            "warning: result of expression is being ignored on {}",
                            stmt.loc()
                        )
                    };
                    if rest.len() == 0 {
                        Ok(ExprAndMeta::new(
                            expr1,
                            if *has_semi { Typ::Unit } else { typ1 },
                            vec![calls1],
                        ))
                    } else {
                        if !has_semi {
                            return Err(Error::new(&format!(
                                "missing semi-colon after expression at {}",
                                stmt.loc()
                            )));
                        };
                        let (expr2, calls2, typ2) =
                            Expr::from_block_stmt(rest, headers, vars)?.split();
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
                    let (expr1, calls1, typ1) = Expr::from_loc_expr(&le, headers, vars)?.split();
                    if ids.len() == 1 {
                        let id = ids[0].id();
                        let (expr2, calls2, typ2) =
                            Expr::from_block_stmt(rest, headers, &vars.add_var(id, &typ1))?.split();
                        Ok(ExprAndMeta::new(
                            expr2.let_expr(vec![id], expr1),
                            typ2,
                            vec![calls1, calls2],
                        ))
                    } else {
                        match typ1 {
                            Typ::Tuple(ref tys) if ids.len() == tys.len() => {
                                let mut vs = Vec::new();
                                let mut let_vars = vars.clone();
                                for (id, ty) in ids.iter().zip(tys) {
                                    let v = id.id();
                                    let_vars = let_vars.add_var(v, ty);
                                    vs.push(v)
                                }
                                let (expr2, calls2, typ2) =
                                    Expr::from_block_stmt(rest, headers, &let_vars)?.split();
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
        headers: &mut Headers,
        vars: &Vars,
    ) -> Result<ExprAndMeta, Error> {
        headers.clear_return_typ();
        let em = Expr::from_loc_expr(e, headers, vars)?;
        match headers.return_typ() {
            Some(rtype) => Typ::type_check("REPL", vec![(None, &em.typ)], vec![(None, &rtype)])?,
            None => (),
        }
        Ok(em)
    }
    fn check_from_block_stmt(
        block: &[parser::LocStmt],
        headers: &mut Headers,
        vars: &Vars,
        name: Option<&str>,
    ) -> Result<ExprAndMeta, self::Error> {
        headers.clear_return_typ();
        let em = Expr::from_block_stmt(block, headers, vars)?;
        // check if type of "return" calls is type of statement
        match headers.return_typ() {
            Some(rtype) => Typ::type_check(
                name.unwrap_or("REPL"),
                vec![(None, &em.typ)],
                vec![(None, &rtype)],
            )?,
            None => (),
        }
        // check if declared return type of function is type of statement
        match name {
            Some(name) => Typ::type_check(
                name,
                vec![(None, &em.typ)],
                vec![(None, &headers.return_typ_for_function(name)?)],
            )?,
            None => (),
        }
        Ok(em)
    }
    fn from_decl<'a>(
        decl: &'a parser::FnDecl,
        headers: &'a mut Headers,
    ) -> Result<(&'a str, Expr, Calls), Error> {
        let mut vars = Vars::new();
        for a in decl.args().iter().rev() {
            vars = vars.add_var(a.name(), &a.typ()?)
        }
        let name = decl.name();
        let em = Expr::check_from_block_stmt(decl.body(), headers, &vars, Some(name))?;
        let mut e = em.expr;
        for a in decl.args().iter().rev() {
            e = e.closure_expr(a.name())
        }
        Ok((name, e, em.calls))
    }
    pub fn from_string(buf: &str, mut headers: &mut Headers) -> Result<Expr, self::Error> {
        let lex = lexer::lex(buf);
        let toks = lexer::Tokens::new(&lex);
        // println!("{}", toks);
        match parser::parse_block_stmt_eof(toks) {
            Ok((_rest, block)) => {
                // println!("{:#?}", block);
                Ok(Expr::check_from_block_stmt(&block, headers, &Vars::new(), None)?.expr)
            }
            Err(_) => match parser::parse_expr_eof(toks) {
                Ok((_rest, e)) => {
                    // println!("{:#?}", e);
                    Ok(Expr::check_from_loc_expr(&e, &mut headers, &Vars::new())?.expr)
                }
                Err(nom::Err::Error(nom::Context::Code(toks, _))) => {
                    Err(self::Error(format!("syntax error: {}", toks.tok[0])))
                }
                Err(err) => Err(self::Error(format!("{:?}", err))),
            },
        }
    }
}

// #[derive(Clone)]
pub struct Code {
    internals: HashMap<String, Expr>,
    externals: externals::Externals,
}

impl Code {
    fn new() -> Code {
        Code {
            internals: HashMap::new(),
            externals: externals::Externals::new(),
        }
    }
    pub fn get(&self, s: &str) -> Option<&Expr> {
        self.internals.get(s)
    }
    pub fn externals(&mut self) -> &mut externals::Externals {
        &mut self.externals
    }
}

pub struct Program {
    pub code: Code,
    pub headers: Headers,
    graph: graph::DiGraph<String, lexer::Loc>,
    nodes: HashMap<String, graph::NodeIndex>,
}

impl Program {
    pub fn new() -> Program {
        Program {
            code: Code::new(),
            headers: Headers::new(),
            graph: graph::Graph::new(),
            nodes: HashMap::new(),
        }
    }
    pub fn set_timeout(&mut self, d: std::time::Duration) {
        self.code.externals.set_timeout(d)
    }
    fn add_decl(&mut self, decl: &parser::FnDecl) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &mut self.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = self
            .nodes
            .get(name)
            .ok_or(Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls.into_iter().filter(|c| !Headers::is_builtin(&c.name)) {
            let call_idx = self
                .nodes
                .get(&c.name)
                .ok_or(Error::new(&format!("cannot find \"{}\" node", c.name)))?;
            self.graph.add_edge(*own_idx, *call_idx, c.loc);
        }
        self.code.internals.insert(name.to_string(), e);
        Ok(())
    }
    fn check_for_cycles(&self) -> Result<(), Error> {
        if let Err(cycle) = petgraph::algo::toposort(&self.graph, None) {
            if let Some(name) = self.graph.node_weight(cycle.node_id()) {
                Err(Error::new(&format!(
                    "Cycle detected: the function \"{}\" might not terminate",
                    name
                )))
            } else {
                Err(Error::new("Cycle detected for unknown function"))
            }
        } else {
            Ok(())
        }
    }
    fn add_node(&mut self, name: &str) {
        self.nodes
            .insert(name.to_string(), self.graph.add_node(name.to_string()));
    }
    pub fn from_string(buf: &str) -> Result<Program, self::Error> {
        match parser::parse_program(lexer::Tokens::new(&lexer::lex(buf))) {
            Ok((_rest, prog_parse)) => {
                let mut prog = Program::new();
                // process headers (for type information)
                for decl in prog_parse.iter() {
                    match decl {
                        parser::Decl::FnDecl(decl) => {
                            let name = decl.name();
                            let (args, typ) = decl.typ().map_err(|err| {
                                Error::new(&format!(
                                    "function \"{}\" at {}: {}",
                                    name,
                                    decl.loc(),
                                    err
                                ))
                            })?;
                            prog.headers.add_function(name, args, &typ)?;
                            prog.add_node(name);
                        }
                        parser::Decl::External(e) => {
                            let ename = e.name();
                            for h in e.headers.iter() {
                                let name = &format!("{}::{}", ename, h.name());
                                let (args, typ) = h.typ().map_err(|err| {
                                    Error::new(&format!(
                                        "header \"{}\" at {}: {}",
                                        name,
                                        h.loc(),
                                        err
                                    ))
                                })?;
                                prog.headers.add_function(name, args, &typ)?;
                                prog.add_node(name);
                            }
                            if let Err(err) = prog.code.externals().add_client(ename, e.url()) {
                                println!("failed to add external \"{}\": {:?}", ename, err)
                            }
                        }
                    }
                }
                // process declarations
                for decl in prog_parse {
                    match decl {
                        parser::Decl::FnDecl(decl) => prog.add_decl(&decl)?,
                        _ => (),
                    }
                }
                prog.check_for_cycles()?;
                Ok(prog)
            }
            Err(nom::Err::Error(nom::Context::Code(toks, _))) => {
                match parser::parse_fn_head(toks) {
                    Ok((rest, head)) => {
                        let s = format!(
                            r#"syntax error in body of function "{}" starting at line {:?}"#,
                            head.name(),
                            toks.tok[0].loc.line
                        );
                        match parser::parse_block_stmt(rest) {
                            Ok(_) => unreachable!(),
                            Err(nom::Err::Error(nom::Context::Code(toks, _))) => {
                                Err(self::Error(format!("{}\nsee: {}", s, toks.tok[0])))
                            }
                            Err(e) => Err(self::Error(format!("{}\n{:?}", s, e))),
                        }
                    }
                    Err(nom::Err::Error(nom::Context::Code(toks, _))) => Err(self::Error(format!(
                        "syntax error in function header, starting: {}",
                        toks.tok[0]
                    ))),
                    Err(e) => Err(self::Error(format!("{:?}", e))),
                }
            }
            Err(e) => Err(self::Error(format!("{:?}", e))),
        }
    }
}
