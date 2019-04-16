/// policy language
// TODO: tuple return type?
use super::{lexer, parser, types};
use parser::{Infix, Literal, Prefix};
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

lazy_static! {
    static ref BUILTINS: HashMap<String, types::Signature> = {
        let mut m = HashMap::new();
        m.insert("i64::abs".to_string(), (vec![Typ::I64], Typ::I64));
        m.insert("str::len".to_string(), (vec![Typ::Str], Typ::I64));
        m.insert("str::to_lowercase".to_string(), (vec![Typ::Str], Typ::Str));
        m.insert("str::to_uppercase".to_string(), (vec![Typ::Str], Typ::Str));
        m.insert("str::trim_start".to_string(), (vec![Typ::Str], Typ::Str));
        m.insert("str::trim_end".to_string(), (vec![Typ::Str], Typ::Str));
        m.insert("str::as_bytes".to_string(), (vec![Typ::Str], Typ::Data));
        m.insert("str::from_utf8".to_string(), (vec![Typ::Data], Typ::Str));
        m.insert("data::len".to_string(), (vec![Typ::Str], Typ::I64));
        m.insert("i64::pow".to_string(), (vec![Typ::I64, Typ::I64], Typ::I64));
        m.insert("i64::min".to_string(), (vec![Typ::I64, Typ::I64], Typ::I64));
        m.insert("i64::max".to_string(), (vec![Typ::I64, Typ::I64], Typ::I64));
        m.insert(
            "str::starts_with".to_string(),
            (vec![Typ::Str, Typ::Str], Typ::Bool),
        );
        m.insert(
            "str::ends_with".to_string(),
            (vec![Typ::Str, Typ::Str], Typ::Bool),
        );
        m.insert(
            "str::contains".to_string(),
            (vec![Typ::Str, Typ::Str], Typ::Bool),
        );
        m
    };
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

#[derive(Debug, Clone)]
pub struct Headers {
    functions: HashMap<String, types::Signature>,
    current_function: String,
}

impl Headers {
    fn new() -> Headers {
        Headers {
            functions: BUILTINS.clone(),
            current_function: String::new(),
        }
    }
    fn add_function(&mut self, name: &str, args: Vec<Typ>, ret: &Typ) -> Result<(), Error> {
        if self
            .functions
            .insert(name.to_string(), (args, ret.to_owned()))
            .is_some()
        {
            Err(Error::new(&format!("duplicate function \"{}\"", name)))
        } else {
            Ok(())
        }
    }
    fn return_typ(&self) -> Result<Typ, Error> {
        self.get_function(&self.current_function)
            .map(|x| x.1)
            .ok_or(Error::new(&format!(
                "function \"{}\" does not have a return type",
                self.current_function
            )))
    }
    fn set_current_function(&mut self, s: &str) {
        self.current_function = s.to_string()
    }
    fn get_function(&self, name: &str) -> Option<types::Signature> {
        self.functions.get(name).cloned()
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Expr {
    Var(parser::Ident),
    BVar(usize),
    ClosureExpr(String, Option<Box<Expr>>, Box<Expr>), // let-expression or plain closure
    LitExpr(Literal),
    ReturnExpr(Box<Expr>),
    PrefixExpr(Prefix, Box<Expr>),
    InfixExpr(Infix, Box<Expr>, Box<Expr>),
    BlockExpr(Vec<Expr>),
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
    InExpr {
        val: Box<Expr>,
        vals: Vec<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Var(id) => write!(f, r#"var "{}""#, id.0),
            Expr::BVar(i) => write!(f, "bvar {}", i),
            Expr::ClosureExpr(s, None, _) => write!(f, "lambda {}. <..>", s),
            Expr::ClosureExpr(s, Some(_), _) => write!(f, "let {} = <..>; <..>", s),
            Expr::LitExpr(l) => write!(f, "{}", l),
            Expr::ReturnExpr(_) => write!(f, "return <..>"),
            Expr::PrefixExpr(_, _) => write!(f, "prefix <..>"),
            Expr::InfixExpr(_, _, _) => write!(f, "infix <..>"),
            Expr::BlockExpr(_) => write!(f, "{{<..>}}"),
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
            Expr::InExpr { .. } => write!(f, "<..> in [<..>]"),
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
    pub fn data(d: &str) -> Expr {
        Expr::LitExpr(Literal::DataLiteral(d.to_string()))
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
            Expr::ClosureExpr(s, e1, e2) => Expr::ClosureExpr(
                s,
                match e1 {
                    Some(e) => Some(Box::new(e.abs(i, v))),
                    None => None,
                },
                Box::new(e2.abs(i + 1, v)),
            ),
            Expr::ReturnExpr(e) => Expr::return_expr(e.abs(i, v)),
            Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.abs(i, v)),
            Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.abs(i, v), e2.abs(i, v)),
            Expr::BlockExpr(es) => Expr::BlockExpr(es.into_iter().map(|e| e.abs(i, v)).collect()),
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
                consequence: Box::new(consequence.abs(i + 1, v)),
                alternative: alternative.map(|e| Box::new(e.abs(i + 1, v))),
            },
            Expr::CallExpr {
                function,
                arguments,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.abs(i, v)).collect(),
            },
            Expr::InExpr { val, vals } => Expr::InExpr {
                val: Box::new(val.abs(i, v)),
                vals: vals.into_iter().map(|a| a.abs(i, v)).collect(),
            },
            _ => self, // BVar, LitExpr
        }
    }
    fn closure(self, v: &str, e: Option<Expr>) -> Expr {
        Expr::ClosureExpr(
            v.to_string(),
            e.map(|e| Box::new(e)),
            Box::new(self.abs(0, v)),
        )
    }
    pub fn closure_expr(self, v: &str) -> Expr {
        self.closure(v, None)
    }
    pub fn let_expr(self, v: &str, e: Expr) -> Expr {
        self.closure(v, Some(e))
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
                Expr::ClosureExpr(s, e1, e2) => Expr::ClosureExpr(
                    s,
                    match e1 {
                        Some(e) => Some(Box::new(e.shift(i, d))),
                        None => None,
                    },
                    Box::new(e2.shift(i, d + 1)),
                ),
                Expr::ReturnExpr(e) => Expr::return_expr(e.shift(i, d)),
                Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.shift(i, d)),
                Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.shift(i, d), e2.shift(i, d)),
                Expr::BlockExpr(es) => {
                    Expr::BlockExpr(es.into_iter().map(|e| e.shift(i, d)).collect())
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
                    consequence: Box::new(consequence.shift(i, d + 1)),
                    alternative: alternative.map(|a| Box::new(a.shift(i, d + 1))),
                },
                Expr::CallExpr {
                    function,
                    arguments,
                } => Expr::CallExpr {
                    function,
                    arguments: arguments.into_iter().map(|a| a.shift(i, d)).collect(),
                },
                Expr::InExpr { val, vals } => Expr::InExpr {
                    val: Box::new(val.shift(i, d)),
                    vals: vals.into_iter().map(|a| a.shift(i, d)).collect(),
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
            Expr::ClosureExpr(s, e1, e2) => Expr::ClosureExpr(
                s,
                match e1 {
                    Some(e) => Some(Box::new(e.subst(i, u))),
                    None => None,
                },
                Box::new(e2.subst(i + 1, u)),
            ),
            Expr::ReturnExpr(e) => Expr::return_expr(e.subst(i, u)),
            Expr::PrefixExpr(p, e) => Expr::prefix_expr(p, e.subst(i, u)),
            Expr::InfixExpr(op, e1, e2) => Expr::infix_expr(op, e1.subst(i, u), e2.subst(i, u)),
            Expr::BlockExpr(es) => Expr::BlockExpr(es.into_iter().map(|e| e.subst(i, u)).collect()),
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
                consequence: Box::new(consequence.subst(i + 1, u)),
                alternative: alternative.map(|a| Box::new(a.subst(i + 1, u))),
            },
            Expr::CallExpr {
                function,
                arguments,
            } => Expr::CallExpr {
                function,
                arguments: arguments.into_iter().map(|a| a.subst(i, u)).collect(),
            },
            Expr::InExpr { val, vals } => Expr::InExpr {
                val: Box::new(val.subst(i, u)),
                vals: vals.into_iter().map(|a| a.subst(i, u)).collect(),
            },
            _ => self, // Var, LitExpr
        }
    }
    pub fn apply(self, u: &Expr) -> Result<Expr, self::Error> {
        match self {
            Expr::ClosureExpr(_, None, e) => Ok(e.subst(0, u)),
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
                    Typ::type_check(
                        "(in)equality",
                        vec![(Some(e1.loc()), &typ1)],
                        vec![(Some(e2.loc()), &typ2)],
                    )?
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
                        let cap_names: HashSet<(String, bool)> =
                            re.0.capture_names()
                                .filter_map(|x| {
                                    x.map(|y| {
                                        (y.trim_start_matches('_').to_string(), y.starts_with('_'))
                                    })
                                })
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
                let vs: Vec<(String, bool)> = set.into_iter().collect();
                let mut extend_vars = vars.clone();
                for (v, is_i64) in vs.iter() {
                    extend_vars =
                        extend_vars.add_var(&v, &(if *is_i64 { Typ::I64 } else { Typ::Str }))
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
            parser::Expr::CallExpr {
                loc,
                function,
                arguments,
            } => match headers.get_function(function) {
                Some((args, typ)) => {
                    let expressions: Result<Vec<ExprAndMeta>, self::Error> = arguments
                        .iter()
                        .map(|e| Expr::from_loc_expr(e, headers, vars))
                        .collect();
                    let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
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
                }
                None => Err(Error::new(&format!(
                    "undeclared function \"{}\" at {}",
                    function,
                    e.loc()
                ))),
            },
            parser::Expr::InExpr { val, vals } => {
                let (expr1, calls1, typ1) = Expr::from_loc_expr(val, headers, vars)?.split();
                let expressions: Result<Vec<ExprAndMeta>, self::Error> = vals
                    .iter()
                    .map(|e| Expr::from_loc_expr(e, headers, vars))
                    .collect();
                let (expressions, mut calls, types) = ExprAndMeta::split_vec(expressions?);
                let len = types.len();
                let types = vals
                    .iter()
                    .map(|e| Some(e.loc()))
                    .zip(types.iter())
                    .collect();
                Typ::type_check("in-expression", types, vec![(Some(val.loc()), &typ1); len])?;
                calls.push(calls1);
                Ok(ExprAndMeta::new(
                    Expr::InExpr {
                        val: Box::new(expr1),
                        vals: expressions,
                    },
                    Typ::Bool,
                    calls,
                ))
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
                        Typ::type_check(
                            "return",
                            vec![(Some(re.loc()), &typ)],
                            vec![(None, &headers.return_typ()?)],
                        )?;
                        Ok(ExprAndMeta::new(
                            Expr::BlockExpr(vec![Expr::return_expr(expr)]),
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
                            Expr::BlockExpr(mut b) => {
                                b.push(expr1);
                                Ok(ExprAndMeta::new(
                                    Expr::BlockExpr(b),
                                    typ2,
                                    vec![calls1, calls2],
                                ))
                            }
                            _ => Ok(ExprAndMeta::new(
                                Expr::BlockExpr(vec![expr2, expr1]),
                                typ2,
                                vec![calls1, calls2],
                            )),
                        }
                    }
                }
                parser::Stmt::LetStmt(id, le) => {
                    let (expr1, calls1, typ1) = Expr::from_loc_expr(&le, headers, vars)?.split();
                    let (expr2, calls2, typ2) =
                        Expr::from_block_stmt(rest, headers, &vars.add_var(id.id(), &typ1))?
                            .split();
                    Ok(ExprAndMeta::new(
                        expr2.let_expr(id.id(), expr1),
                        typ2,
                        vec![calls1, calls2],
                    ))
                }
            },
            None => Ok(ExprAndMeta::new(
                Expr::BlockExpr(Vec::new()),
                Typ::Unit,
                vec![],
            )),
        }
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
        headers.set_current_function(name);
        let em = Expr::from_block_stmt(decl.body(), headers, &vars)?;
        let mut e = em.expr;
        Typ::type_check(
            name,
            vec![(None, &em.typ)],
            vec![(None, &headers.return_typ()?)],
        )?;
        for a in decl.args().iter().rev() {
            e = e.closure_expr(a.name())
        }
        Ok((name, e, em.calls))
    }
    pub fn from_string(buf: &str, mut headers: &mut Headers) -> Result<Expr, self::Error> {
        let lex = lexer::lex(buf);
        let toks = lexer::Tokens::new(&lex);
        match parser::parse_block_stmt_eof(toks) {
            Ok((_rest, block)) => Ok(Expr::from_block_stmt(&block, headers, &Vars::new())?.expr),
            Err(_) => match parser::parse_expr_eof(toks) {
                Ok((_rest, e)) => {
                    // println!("{:#?}", e);
                    Ok(Expr::from_loc_expr(&e, &mut headers, &Vars::new())?.expr)
                }
                Err(nom::Err::Error(nom::Context::Code(toks, _))) => {
                    Err(self::Error(format!("syntax error: {}", toks.tok[0])))
                }
                Err(err) => Err(self::Error(format!("{:?}", err))),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Code(HashMap<String, Expr>);

impl Code {
    fn new() -> Code {
        Code(HashMap::new())
    }
    pub fn get(&self, s: &str) -> Option<&Expr> {
        self.0.get(s)
    }
}

pub struct Program {
    pub code: Code,
    pub headers: Headers,
    graph: graph::DiGraph<String, lexer::Loc>,
    nodes: HashMap<String, graph::NodeIndex>,
}

impl Program {
    fn new() -> Program {
        Program {
            code: Code::new(),
            headers: Headers::new(),
            graph: graph::Graph::new(),
            nodes: HashMap::new(),
        }
    }
    fn add_decl(&mut self, decl: &parser::FnDecl) -> Result<(), Error> {
        // println!("{:#?}", decl);
        let (name, e, calls) = Expr::from_decl(decl, &mut self.headers)?;
        // println!(r#""{}": {:#?}"#, name, e);
        let own_idx = self
            .nodes
            .get(name)
            .ok_or(Error::new(&format!("cannot find \"{}\" node", name)))?;
        for c in calls
            .into_iter()
            .filter(|c| !BUILTINS.contains_key(&c.name))
        {
            let call_idx = self
                .nodes
                .get(&c.name)
                .ok_or(Error::new(&format!("cannot find \"{}\" node", c.name)))?;
            self.graph.add_edge(*own_idx, *call_idx, c.loc);
        }
        self.code.0.insert(name.to_string(), e);
        Ok(())
    }
    fn check_for_cycles(&self) -> Result<(), Error> {
        if let Err(cycle) = petgraph::algo::toposort(&self.graph, None) {
            if let Some(name) = self.graph.node_weight(cycle.node_id()) {
                Err(Error::new(&format!(
                    "Cycle detected: the function \"{}\" might not termninate",
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
                    let (args, typ) = decl.typ()?;
                    prog.headers.add_function(decl.name(), args, &typ)?;
                    prog.add_node(decl.name());
                }
                // process declarations
                for decl in prog_parse {
                    prog.add_decl(&decl)?
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
