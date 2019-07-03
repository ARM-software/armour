/// policy language interpreter
// NOTE: no optimization
use super::headers::Headers;
use super::lang::{Block, Error, Expr, Program};
use super::literals::Literal;
use super::parser::{As, Infix, Iter, Pat, Prefix};
use futures::{
    future,
    stream::{self, Stream},
    Future,
};
use std::collections::HashMap;
use std::sync::Arc;

impl Literal {
    fn eval_prefix(&self, p: &Prefix) -> Option<Self> {
        match (p, self) {
            (Prefix::Not, Literal::BoolLiteral(b)) => Some(Literal::BoolLiteral(!b)),
            (Prefix::PrefixMinus, Literal::IntLiteral(i)) => Some(Literal::IntLiteral(-i)),
            _ => None,
        }
    }
    fn eval_infix(&self, op: &Infix, other: &Literal) -> Option<Self> {
        match (op, self, other) {
            (Infix::Equal, _, _) => Some(Literal::BoolLiteral(self == other)),
            (Infix::NotEqual, _, _) => Some(Literal::BoolLiteral(self != other)),
            (Infix::Plus, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i + j))
            }
            (Infix::Minus, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i - j))
            }
            (Infix::Divide, Literal::IntLiteral(_), Literal::IntLiteral(0)) => None,
            (Infix::Divide, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i / j))
            }
            (Infix::Multiply, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i * j))
            }
            (Infix::Remainder, Literal::IntLiteral(_), Literal::IntLiteral(0)) => None,
            (Infix::Remainder, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i % j))
            }
            (Infix::LessThan, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::BoolLiteral(i < j))
            }
            (Infix::LessThanEqual, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::BoolLiteral(i <= j))
            }
            (Infix::GreaterThan, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::BoolLiteral(i > j))
            }
            (Infix::GreaterThanEqual, Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::BoolLiteral(i >= j))
            }
            (Infix::And, Literal::BoolLiteral(i), Literal::BoolLiteral(j)) => {
                Some(Literal::BoolLiteral(*i && *j))
            }
            (Infix::Or, Literal::BoolLiteral(i), Literal::BoolLiteral(j)) => {
                Some(Literal::BoolLiteral(*i || *j))
            }
            (Infix::Concat, Literal::List(i), Literal::List(j)) => Some(Literal::List({
                let mut k = i.clone();
                k.append(&mut j.clone());
                k
            })),
            (Infix::ConcatStr, Literal::StringLiteral(i), Literal::StringLiteral(j)) => {
                Some(Literal::StringLiteral(format!("{}{}", i, j)))
            }
            (Infix::In, _, Literal::List(l)) => {
                Some(Literal::BoolLiteral(l.iter().any(|o| o == self)))
            }
            _ => None,
        }
    }
    fn eval_call0(f: &str) -> Option<Self> {
        match f {
            "HttpRequest::default" => Some(Literal::HttpRequestLiteral(Default::default())),
            _ => None,
        }
    }
    fn eval_call1(&self, f: &str) -> Option<Self> {
        match (f, self) {
            ("Some", _) => Some(Literal::Tuple(vec![self.clone()])),
            ("option::is_none", Literal::Tuple(t)) => Some(Literal::BoolLiteral(t.len() == 0)),
            ("option::is_some", Literal::Tuple(t)) => Some(Literal::BoolLiteral(t.len() == 1)),
            ("i64::abs", Literal::IntLiteral(i)) => Some(Literal::IntLiteral(i.abs())),
            ("i64::to_str", Literal::IntLiteral(i)) => Some(Literal::StringLiteral(i.to_string())),
            ("str::len", Literal::StringLiteral(s)) => Some(Literal::IntLiteral(s.len() as i64)),
            ("str::to_lowercase", Literal::StringLiteral(s)) => {
                Some(Literal::StringLiteral(s.to_lowercase()))
            }
            ("str::to_uppercase", Literal::StringLiteral(s)) => {
                Some(Literal::StringLiteral(s.to_uppercase()))
            }
            ("str::trim_start", Literal::StringLiteral(s)) => {
                Some(Literal::StringLiteral(s.trim_start().to_string()))
            }
            ("str::trim_end", Literal::StringLiteral(s)) => {
                Some(Literal::StringLiteral(s.trim_end().to_string()))
            }
            ("str::as_bytes", Literal::StringLiteral(s)) => {
                Some(Literal::DataLiteral(s.as_bytes().to_vec()))
            }
            ("str::from_utf8", Literal::DataLiteral(s)) => Some(Literal::StringLiteral(
                std::string::String::from_utf8_lossy(s).to_string(),
            )),
            ("str::to_base64", Literal::StringLiteral(s)) => {
                Some(Literal::StringLiteral(base64::encode(s)))
            }
            ("data::to_base64", Literal::DataLiteral(d)) => {
                Some(Literal::StringLiteral(base64::encode(d)))
            }
            ("data::len", Literal::DataLiteral(d)) => Some(Literal::IntLiteral(d.len() as i64)),
            ("HttpRequest::method", Literal::HttpRequestLiteral(req)) => {
                Some(Literal::StringLiteral(req.method()))
            }
            ("HttpRequest::version", Literal::HttpRequestLiteral(req)) => {
                Some(Literal::StringLiteral(req.version()))
            }
            ("HttpRequest::path", Literal::HttpRequestLiteral(req)) => {
                Some(Literal::StringLiteral(req.path()))
            }
            ("HttpRequest::route", Literal::HttpRequestLiteral(req)) => Some(Literal::List(
                req.split_path()
                    .into_iter()
                    .map(|h| Literal::StringLiteral(h))
                    .collect(),
            )),
            ("HttpRequest::query", Literal::HttpRequestLiteral(req)) => {
                Some(Literal::StringLiteral(req.query()))
            }
            ("HttpRequest::query_pairs", Literal::HttpRequestLiteral(req)) => Some(Literal::List(
                req.query_pairs()
                    .iter()
                    .map(|(k, v)| {
                        Literal::Tuple(vec![
                            Literal::StringLiteral(k.to_string()),
                            Literal::StringLiteral(v.to_string()),
                        ])
                    })
                    .collect(),
            )),
            ("HttpRequest::header_pairs", Literal::HttpRequestLiteral(req)) => Some(Literal::List(
                req.header_pairs()
                    .iter()
                    .map(|(k, v)| {
                        Literal::Tuple(vec![
                            Literal::StringLiteral(k.to_string()),
                            Literal::StringLiteral(String::from_utf8_lossy(&v).into_owned()),
                        ])
                    })
                    .collect(),
            )),
            ("HttpRequest::headers", Literal::HttpRequestLiteral(req)) => Some(Literal::List(
                req.headers()
                    .into_iter()
                    .map(|h| Literal::StringLiteral(h))
                    .collect(),
            )),
            ("list::len", Literal::List(l)) => Some(Literal::IntLiteral(l.len() as i64)),
            (_, Literal::Tuple(l)) => {
                if let Ok(i) = f.parse::<usize>() {
                    l.get(i).cloned()
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn eval_call2(&self, f: &str, other: &Literal) -> Option<Self> {
        match (f, self, other) {
            ("i64::pow", Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(i.pow(*j as u32)))
            }
            ("i64::min", Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(std::cmp::min(*i, *j)))
            }
            ("i64::max", Literal::IntLiteral(i), Literal::IntLiteral(j)) => {
                Some(Literal::IntLiteral(std::cmp::max(*i, *j)))
            }
            ("str::starts_with", Literal::StringLiteral(i), Literal::StringLiteral(j)) => {
                Some(Literal::BoolLiteral(i.starts_with(j)))
            }
            ("str::ends_with", Literal::StringLiteral(i), Literal::StringLiteral(j)) => {
                Some(Literal::BoolLiteral(i.ends_with(j)))
            }
            ("str::contains", Literal::StringLiteral(i), Literal::StringLiteral(j)) => {
                Some(Literal::BoolLiteral(i.contains(j)))
            }
            (
                "HttpRequest::set_path",
                Literal::HttpRequestLiteral(req),
                Literal::StringLiteral(q),
            ) => Some(Literal::HttpRequestLiteral(req.set_path(q))),
            (
                "HttpRequest::set_query",
                Literal::HttpRequestLiteral(req),
                Literal::StringLiteral(q),
            ) => Some(Literal::HttpRequestLiteral(req.set_query(q))),
            (
                "HttpRequest::header",
                Literal::HttpRequestLiteral(req),
                Literal::StringLiteral(h),
            ) => Some(Literal::List(
                req.header(&h)
                    .into_iter()
                    .map(|v| Literal::DataLiteral(v))
                    .collect(),
            )),
            _ => None,
        }
    }
    fn eval_call3(&self, f: &str, l1: &Literal, l2: &Literal) -> Option<Self> {
        match (f, self, l1, l2) {
            (
                "HttpRequest::set_header",
                Literal::HttpRequestLiteral(req),
                Literal::StringLiteral(h),
                Literal::DataLiteral(v),
            ) => Some(Literal::HttpRequestLiteral(req.set_header(h, v))),
            _ => None,
        }
    }
    pub fn literal_vector(args: Vec<Expr>) -> Result<Vec<Literal>, Error> {
        let mut v = Vec::new();
        for a in args {
            match a {
                Expr::LitExpr(l) => v.push(l),
                _ => return Err(Error::new("arg is not a literal")),
            }
        }
        Ok(v)
    }
    fn is_true(&self) -> bool {
        match self {
            Literal::BoolLiteral(true) => true,
            _ => false,
        }
    }
}

impl Expr {
    fn is_return(&self) -> bool {
        match self {
            Expr::ReturnExpr(_) => true,
            _ => false,
        }
    }
    fn strip_return(self) -> Expr {
        match self {
            Expr::ReturnExpr(r) => *r,
            _ => self,
        }
    }
    fn eval(self, env: Arc<Program>) -> Box<dyn Future<Item = Expr, Error = self::Error>> {
        match self {
            Expr::Var(_) | Expr::BVar(_) => Box::new(future::err(Error::new("eval variable"))),
            Expr::LitExpr(_) => Box::new(future::ok(self)),
            Expr::Closure(_) => Box::new(future::err(Error::new("eval, closure"))),
            Expr::ReturnExpr(e) => Box::new(
                e.eval(env)
                    .and_then(|res| future::ok(Expr::return_expr(res))),
            ),
            Expr::PrefixExpr(p, e) => Box::new(e.eval(env).and_then(move |res| match res {
                r @ Expr::ReturnExpr(_) => future::ok(r),
                Expr::LitExpr(l) => match l.eval_prefix(&p) {
                    Some(r) => future::ok(Expr::LitExpr(r)),
                    None => future::err(Error::new("eval prefix: type error")),
                },
                _ => future::err(Error::new("eval, prefix")),
            })),
            // short circuit for &&
            Expr::InfixExpr(Infix::And, e1, e2) => {
                Box::new(e1.eval(env.clone()).and_then(move |res1| match res1 {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(false)) => {
                        future::Either::A(future::ok(r))
                    }
                    Expr::LitExpr(Literal::BoolLiteral(true)) => {
                        future::Either::B(e2.eval(env).and_then(move |res2| match res2 {
                            r @ Expr::ReturnExpr(_)
                            | r @ Expr::LitExpr(Literal::BoolLiteral(_)) => future::ok(r),
                            _ => future::err(Error::new("eval, infix")),
                        }))
                    }
                    _ => future::Either::A(future::err(Error::new("eval, infix"))),
                }))
            }
            // short circuit for ||
            Expr::InfixExpr(Infix::Or, e1, e2) => {
                Box::new(e1.eval(env.clone()).and_then(|res1| match res1 {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(true)) => {
                        future::Either::A(future::ok(r))
                    }
                    Expr::LitExpr(Literal::BoolLiteral(false)) => {
                        future::Either::B(e2.eval(env).and_then(|res2| match res2 {
                            r @ Expr::ReturnExpr(_)
                            | r @ Expr::LitExpr(Literal::BoolLiteral(_)) => future::ok(r),
                            _ => future::err(Error::new("eval, infix")),
                        }))
                    }
                    _ => future::Either::A(future::err(Error::new("eval, infix"))),
                }))
            }
            Expr::InfixExpr(op, e1, e2) => {
                Box::new(e1.eval(env.clone()).and_then(move |res1| match res1 {
                    r @ Expr::ReturnExpr(_) => future::Either::A(future::ok(r)),
                    Expr::LitExpr(l1) => {
                        future::Either::B(e2.eval(env).and_then(move |res2| match res2 {
                            r @ Expr::ReturnExpr(_) => future::ok(r),
                            Expr::LitExpr(l2) => match l1.eval_infix(&op, &l2) {
                                Some(r) => future::ok(Expr::LitExpr(r)),
                                None => future::err(Error::new("eval, infix: type error")),
                            },
                            _ => future::err(Error::new("eval, infix: failed")),
                        }))
                    }
                    _ => future::Either::A(future::err(Error::new("eval, infix: failed"))),
                }))
            }
            Expr::BlockExpr(b, mut es) => {
                if es.len() == 0 {
                    Box::new(future::ok(Expr::LitExpr(Literal::Unit)))
                } else if b == Block::Block {
                    let e = es.remove(0);
                    Box::new(e.eval(env.clone()).and_then(move |res| {
                        if res.is_return() || es.len() == 0 {
                            future::Either::A(future::ok(res))
                        } else {
                            future::Either::B(Expr::BlockExpr(b, es).eval(env))
                        }
                    }))
                } else {
                    Box::new(
                        stream::futures_ordered(es.into_iter().map(|e| e.eval(env.clone())))
                            .collect()
                            .and_then(move |rs| match rs.iter().find(|r| r.is_return()) {
                                Some(r) => future::ok(r.clone()),
                                _ => match Literal::literal_vector(rs) {
                                    Ok(lits) => future::ok(Expr::LitExpr(if b == Block::List {
                                        Literal::List(lits)
                                    } else {
                                        Literal::Tuple(lits)
                                    })),
                                    Err(err) => future::err(err),
                                },
                            }),
                    )
                }
            }
            Expr::Let(vs, e1, e2) => {
                Box::new(e1.eval(env.clone()).and_then(move |res1| match res1 {
                    r @ Expr::ReturnExpr(_) => Box::new(future::ok(r)),
                    Expr::LitExpr(Literal::Tuple(lits)) => {
                        let lits_len = lits.len();
                        if 1 < lits_len && vs.len() == lits_len {
                            let mut e2a = *e2.clone();
                            let mut apply_err = None;
                            for (v, lit) in vs.iter().zip(lits) {
                                if v != "_" {
                                    match e2a.clone().apply(&Expr::LitExpr(lit)) {
                                        Ok(ea) => e2a = ea,
                                        Err(err) => {
                                            apply_err = Some(err);
                                            break;
                                        }
                                    }
                                }
                            }
                            match apply_err {
                                Some(err) => Box::new(future::err(err)),
                                None => e2a.eval(env),
                            }
                        } else if vs.len() == 1 {
                            match e2.apply(&Expr::LitExpr(Literal::Tuple(lits))) {
                                Ok(e2a) => e2a.eval(env),
                                Err(err) => Box::new(future::err(err)),
                            }
                        } else {
                            Box::new(future::err(Error::new(
                                "eval, let-expression (tuple length mismatch)",
                            )))
                        }
                    }
                    l @ Expr::LitExpr(_) => {
                        if vs.len() == 1 {
                            match e2.apply(&l) {
                                Ok(e2a) => e2a.eval(env),
                                Err(err) => Box::new(future::err(err)),
                            }
                        } else {
                            Box::new(future::err(Error::new(
                                "eval, let-expression (literal not a tuple)",
                            )))
                        }
                    }
                    _ => Box::new(future::err(Error::new("eval, let-expression"))),
                }))
            }
            Expr::Iter(op, vs, e1, e2) => Box::new(e1.eval(env.clone()).and_then(move |res1| {
                match res1 {
                    r @ Expr::ReturnExpr(_) => future::Either::A(future::ok(r)),
                    Expr::LitExpr(Literal::List(lits)) => future::Either::B(
                        stream::futures_ordered(lits.clone().into_iter().map(move |l| match l {
                            Literal::Tuple(ref ts) if vs.len() != 1 => {
                                if vs.len() == ts.len() {
                                    let mut e = *e2.clone();
                                    for (v, lit) in vs.iter().zip(ts) {
                                        if v != "_" {
                                            match e.clone().apply(&Expr::LitExpr(lit.clone())) {
                                                Ok(ea) => e = ea,
                                                Err(_) => {
                                                    return future::Either::A(future::err(
                                                        Error::new("eval, iter-expression"),
                                                    ))
                                                }
                                            }

                                        }

                                    }
                                    future::Either::B(e.eval(env.clone()))
                                } else {
                                    future::Either::A(future::err(Error::new(
                                        "eval, iter-expression (tuple length mismatch)",
                                    )))
                                }
                            }
                            _ => {
                                if vs.len() == 1 {
                                    let mut e = *e2.clone();
                                    if vs[0] != "_" {
                                        match e.clone().apply(&Expr::LitExpr(l.clone())) {
                                            Ok(ea) => e = ea,
                                            Err(_) => {
                                                return future::Either::A(future::err(Error::new(
                                                    "eval, iter-expression",
                                                )))
                                            }
                                        }
                                    }
                                    future::Either::B(e.eval(env.clone()))
                                } else {
                                    future::Either::A(future::err(Error::new(
                                        "eval, iter-expression (not a tuple list)",
                                    )))
                                }
                            }
                        }))
                        .collect()
                        .and_then(move |res| {
                            match res.iter().find(|r| r.is_return()) {
                                Some(r) => future::ok(r.clone()),
                                None => match Literal::literal_vector(res) {
                                    Ok(iter_lits) => match op {
                                        Iter::Map => {
                                            future::ok(Expr::LitExpr(Literal::List(iter_lits)))
                                        }
                                        Iter::Filter => {
                                            let filtered_lits = lits
                                                .into_iter()
                                                .zip(iter_lits.into_iter())
                                                .filter_map(|(l, b)| {
                                                    if b.is_true() {
                                                        Some(l)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();
                                            future::ok(Expr::LitExpr(Literal::List(filtered_lits)))
                                        }
                                        Iter::All => future::ok(Expr::bool(
                                            iter_lits.iter().all(|l| l.is_true()),
                                        )),
                                        Iter::Any => future::ok(Expr::bool(
                                            iter_lits.iter().any(|l| l.is_true()),
                                        )),
                                    },
                                    Err(err) => future::err(err),
                                },
                            }
                        }),
                    ),
                    _ => future::Either::A(future::err(Error::new("eval, map-expression"))),
                }
            })),
            Expr::IfExpr {
                cond,
                consequence,
                alternative,
            } => Box::new(cond.eval(env.clone()).and_then(|res1| match res1 {
                r @ Expr::ReturnExpr(_) => future::Either::A(future::ok(r)),
                Expr::LitExpr(Literal::BoolLiteral(b)) => {
                    if b {
                        future::Either::B(future::Either::B(consequence.eval(env).and_then(
                            |res2| match res2 {
                                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => future::ok(r),
                                _ => future::err(Error::new("eval, if-expression")),
                            },
                        )))
                    } else {
                        future::Either::B(match alternative {
                            None => future::Either::A(future::Either::A(future::ok(
                                Expr::LitExpr(Literal::Unit),
                            ))),
                            Some(alt) => future::Either::A(future::Either::B(
                                alt.eval(env).and_then(|res2| match res2 {
                                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => future::ok(r),
                                    _ => future::err(Error::new("eval, if-expression")),
                                }),
                            )),
                        })
                    }
                }
                _ => future::Either::A(future::err(Error::new("eval, if-expression"))),
            })),
            Expr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => Box::new(expr.eval(env.clone()).and_then(|res1| match res1 {
                r @ Expr::ReturnExpr(_) => future::Either::A(future::ok(r)),
                Expr::LitExpr(Literal::Tuple(t)) => {
                    if t.len() == 1 {
                        match consequence.apply(&Expr::LitExpr(t[0].clone())) {
                            Ok(consequence_apply) => future::Either::B(future::Either::B(
                                consequence_apply.eval(env).and_then(|res2| match res2 {
                                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => future::ok(r),
                                    _ => future::err(Error::new("eval, if-let-expression")),
                                }),
                            )),
                            Err(e) => future::Either::A(future::err(e)),
                        }
                    } else {
                        future::Either::B(match alternative {
                            None => future::Either::A(future::Either::A(future::ok(
                                Expr::LitExpr(Literal::Unit),
                            ))),
                            Some(alt) => future::Either::A(future::Either::B(
                                alt.eval(env).and_then(|res2| match res2 {
                                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => future::ok(r),
                                    _ => future::err(Error::new("eval, if-let-expression")),
                                }),
                            )),
                        })
                    }
                }
                _ => future::Either::A(future::err(Error::new(format!(
                    "eval, if-let-expression: {:#?}",
                    res1
                )))),
            })),
            Expr::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => {
                Box::new(
                    stream::futures_ordered(matches.into_iter().map(|(e, re)| {
                        e.eval(env.clone()).and_then(move |f| match f {
                            Expr::ReturnExpr(_) => future::ok((f, None)),
                            Expr::LitExpr(Literal::StringLiteral(ref s)) => {
                                let names: Vec<&str> =
                                    re.0.capture_names().filter_map(|s| s).collect();
                                // if there are no bindings then do a simple "is_match", otherwise collect
                                // variable captures
                                if names.len() == 0 {
                                    if re.0.is_match(s) {
                                        future::ok((f, Some(HashMap::new())))
                                    } else {
                                        future::ok((f, None))
                                    }
                                } else {
                                    match re.0.captures(s) {
                                        // matches
                                        Some(cap) => {
                                            let mut is_match = true;
                                            let mut captures: HashMap<String, Expr> =
                                                HashMap::new();
                                            for name in names {
                                                let match_str = cap.name(name).unwrap().as_str();
                                                let (s, a) = Pat::strip_as(name);
                                                captures.insert(
                                                    s,
                                                    match a {
                                                        As::I64 => match match_str.parse() {
                                                            Ok(i) => Expr::i64(i),
                                                            _ => {
                                                                is_match = false;
                                                                break;
                                                            }
                                                        },
                                                        As::Base64 => {
                                                            match base64::decode(match_str) {
                                                                Ok(bytes) => {
                                                                    Expr::data(bytes.as_slice())
                                                                }
                                                                _ => {
                                                                    is_match = false;
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        _ => Expr::string(match_str),
                                                    },
                                                );
                                            }
                                            if is_match {
                                                future::ok((f, Some(captures)))
                                            } else {
                                                future::ok((f, None))
                                            }
                                        }
                                        // not a match
                                        None => future::ok((f, None)),
                                    }
                                }
                            }
                            _ => future::err(Error::new("eval, if-match-expression: type error")),
                        })
                    }))
                    .collect()
                    .and_then(move |rs| {
                        match rs.iter().find(|(r, _captures)| r.is_return()) {
                            // early exit
                            Some((r, _captures)) => {
                                future::Either::A(future::Either::A(future::ok(r.clone())))
                            }
                            None => match rs.iter().find(|(_r, captures)| captures.is_none()) {
                                // failed match
                                Some(_) => match alternative {
                                    None => future::Either::A(future::Either::A(future::ok(
                                        Expr::LitExpr(Literal::Unit),
                                    ))),
                                    Some(alt) => future::Either::A(future::Either::B(
                                        alt.eval(env).and_then(|res1| match res1 {
                                            r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => {
                                                future::ok(r)
                                            }
                                            _ => {
                                                future::err(Error::new("eval, if-match-expression"))
                                            }
                                        }),
                                    )),
                                },
                                // match
                                _ => {
                                    let mut all_captures: HashMap<String, Expr> = HashMap::new();
                                    for (_r, captures) in rs {
                                        if let Some(caps) = captures {
                                            all_captures.extend(caps)
                                        }
                                    }
                                    let mut c = *consequence;
                                    let mut error_occured = false;
                                    for v in variables {
                                        match all_captures.get(&v) {
                                            Some(e) => match c.clone().apply(e) {
                                                Ok(ce) => c = ce,
                                                Err(_) => {
                                                    error_occured = true;
                                                    break;
                                                }
                                            },
                                            None => {
                                                error_occured = true;
                                                break;
                                            }
                                        }
                                    }
                                    future::Either::B(if error_occured {
                                        future::Either::A(future::err(Error::new(
                                            "eval, if-match-expression: missing bind",
                                        )))
                                    } else {
                                        future::Either::B(c.eval(env).and_then(move |res1| {
                                            match res1 {
                                                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => {
                                                    future::ok(r)
                                                }
                                                _ => future::err(Error::new(
                                                    "eval, if-match-expression",
                                                )),
                                            }
                                        }))
                                    })
                                }
                            },
                        }
                    }),
                )
            }
            Expr::CallExpr {
                function,
                arguments,
            } => {
                Box::new(stream::futures_ordered(arguments.into_iter().map(|e| e.eval(env.clone()))).collect()
                    .and_then(move |args|
                        match args.iter().find(|r| r.is_return()) {
                            Some(r) => future::Either::A(future::ok(r.clone())),
                            None => {
                                // user defined function
                                if let Some(e) = env.internal(&function) {
                                    let mut r = e.clone();
                                    let mut error = None;
                                    for a in args {
                                        match r.clone().apply(&a) {
                                            Ok(ra) =>r = ra,
                                            Err(err) => {error = Some(err); break}
                                        }
                                    }
                                    match error {
                                        Some(err) => future::Either::A(future::err(err)),
                                        None => future::Either::B(r.evaluate(env.clone())),
                                    }
                                // builtin function
                                } else if Headers::is_builtin(&function) {
                                    match args.as_slice() {
                                        &[] => match Literal::eval_call0(&function) {
                                            Some(r) => future::Either::A(future::ok(Expr::LitExpr(r))),
                                            None => future::Either::A(future::err(Error::new("eval, call(0): type error"))),
                                        },
                                        &[Expr::LitExpr(ref l)] => match l.eval_call1(&function) {
                                            Some(r) => future::Either::A(future::ok(Expr::LitExpr(r))),
                                            None => future::Either::A(future::err(Error::new("eval, call(1): type error"))),
                                        },
                                        &[Expr::LitExpr(ref l1), Expr::LitExpr(ref l2)] => {
                                            match l1.eval_call2(&function, l2) {
                                                Some(r) => future::Either::A(future::ok(Expr::LitExpr(r))),
                                                None => future::Either::A(future::err(Error::new("eval, call(2): type error"))),
                                            }
                                        }
                                        &[Expr::LitExpr(ref l1), Expr::LitExpr(ref l2), Expr::LitExpr(ref l3)] => {
                                            match l1.eval_call3(&function, l2, l3) {
                                                Some(r) => future::Either::A(future::ok(Expr::LitExpr(r))),
                                                None => future::Either::A(future::err(Error::new("eval, call(3): type error"))),
                                            }
                                        }
                                        x => future::Either::A(future::err(Error::new(&format!("eval, call: {}: {:?}", function, x)))),
                                    }
                                } else {
                                    // external function (RPC)
                                    match function.split("::").collect::<Vec<&str>>().as_slice() {
                                        &[external, method] =>
                                        future::Either::B(env.external(external, method, args)),
                                        _ => future::Either::A(future::err(Error::new(&format!(
                                            "eval, call: {}: {:?}",
                                            function, args
                                        )))),
                                    }
                                }
                            }
                        }))
            }
        }
    }
    pub fn evaluate(self, env: Arc<Program>) -> Box<dyn Future<Item = Expr, Error = self::Error>> {
        Box::new(
            self.eval(env)
                .and_then(|e| Box::new(future::ok(e.strip_return()))),
        )
    }
}
