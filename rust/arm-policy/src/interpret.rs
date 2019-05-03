/// policy language interpreter
// NOTE: no optimization
use super::headers::Headers;
use super::lang::{Block, Code, Error, Expr};
use super::literals::Literal;
use super::parser::{Infix, Iter, Prefix};
use std::collections::HashMap;

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
                Some(Literal::DataLiteral(s.to_string()))
            }
            ("str::from_utf8", Literal::DataLiteral(s)) => {
                Some(Literal::StringLiteral(s.to_string()))
            }
            ("data::len", Literal::DataLiteral(d)) => {
                Some(Literal::IntLiteral(d.as_bytes().len() as i64))
            }
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
                            Literal::StringLiteral(v.to_string()),
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
            ("HttpRequest::payload", Literal::HttpRequestLiteral(req)) => {
                Some(Literal::DataLiteral(req.payload()))
            }
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
                "HttpRequest::set_payload",
                Literal::HttpRequestLiteral(req),
                Literal::DataLiteral(q),
            ) => Some(Literal::HttpRequestLiteral(req.set_payload(q))),
            (
                "HttpRequest::header",
                Literal::HttpRequestLiteral(req),
                Literal::StringLiteral(h),
            ) => Some(Literal::List(
                req.header(&h)
                    .into_iter()
                    .map(|v| Literal::StringLiteral(v))
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
                Literal::StringLiteral(v),
            ) => Some(Literal::HttpRequestLiteral(req.set_header(h, v))),
            _ => None,
        }
    }
    fn literal_vector(args: Vec<Expr>) -> Result<Vec<Literal>, Error> {
        let mut v = Vec::new();
        for a in args {
            match a {
                Expr::LitExpr(l) => v.push(l),
                _ => return Err(Error::new("arg is not a literal")),
            }
        }
        Ok(v)
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
    fn eval(self, env: &mut Code) -> Result<Expr, self::Error> {
        match self {
            Expr::Var(_) | Expr::BVar(_) => Err(Error::new("eval variable")),
            Expr::LitExpr(_) => Ok(self),
            Expr::ReturnExpr(e) => Ok(Expr::return_expr(e.eval(env)?)),
            Expr::PrefixExpr(p, e) => match e.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(l) => match l.eval_prefix(&p) {
                    Some(r) => Ok(Expr::LitExpr(r)),
                    None => Err(Error::new("eval prefix: type error")),
                },
                _ => Err(Error::new("eval, prefix")),
            },
            // short circuit for &&
            Expr::InfixExpr(Infix::And, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(false)) => Ok(r),
                Expr::LitExpr(Literal::BoolLiteral(true)) => match e2.eval(env)? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(_)) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            // short circuit for ||
            Expr::InfixExpr(Infix::Or, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(true)) => Ok(r),
                Expr::LitExpr(Literal::BoolLiteral(false)) => match e2.eval(env)? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(Literal::BoolLiteral(_)) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            Expr::InfixExpr(op, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(l1) => match e2.eval(env)? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(l2) => match l1.eval_infix(&op, &l2) {
                        Some(r) => Ok(Expr::LitExpr(r)),
                        None => Err(Error::new("eval, infix: type error")),
                    },
                    _ => Err(Error::new("eval, infix: failed")),
                },
                _ => Err(Error::new("eval, infix: failed")),
            },
            Expr::BlockExpr(b, es) => {
                if es.len() == 0 {
                    Ok(Expr::LitExpr(Literal::Unit))
                } else {
                    let rs: Result<Vec<Expr>, self::Error> =
                        es.into_iter().map(|e| e.eval(env)).collect();
                    let rs = rs?;
                    match rs.iter().find(|r| r.is_return()) {
                        Some(r) => Ok(r.clone()),
                        _ => {
                            if b == Block::Block {
                                Ok(rs.last().expect("eval, block").clone())
                            } else {
                                let lits: Result<Vec<Literal>, self::Error> = rs
                                    .into_iter()
                                    .map(|e| match e {
                                        Expr::LitExpr(l) => Ok(l),
                                        _ => Err(Error::new("failed to evaluate member of a list")),
                                    })
                                    .collect();
                                let lits = lits?;
                                Ok(Expr::LitExpr(if b == Block::List {
                                    Literal::List(lits)
                                } else {
                                    Literal::Tuple(lits)
                                }))
                            }
                        }
                    }
                }
            }
            Expr::Let(vs, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(Literal::Tuple(lits)) => {
                    if vs.len() == lits.len() {
                        let mut e = *e2.clone();
                        for (v, lit) in vs.iter().zip(lits) {
                            if v != "_" {
                                e = e.apply(&Expr::LitExpr(lit))?
                            }
                        }
                        e.eval(env)
                    } else if vs.len() == 1 {
                        e2.apply(&Expr::LitExpr(Literal::Tuple(lits)))?.eval(env)
                    } else {
                        Err(Error::new("eval, let-expression (tuple length mismatch)"))
                    }
                }
                l @ Expr::LitExpr(_) => {
                    if vs.len() == 1 {
                        e2.apply(&l)?.eval(env)
                    } else {
                        Err(Error::new("eval, let-expression (literal not a tuple)"))
                    }
                }
                _ => Err(Error::new("eval, let-expression")),
            },
            Expr::Iter(Iter::Map, vs, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(Literal::List(lits)) => {
                    let mut res = Vec::new();
                    for l in lits.iter() {
                        match l {
                            Literal::Tuple(ts) if vs.len() != 1 => {
                                if vs.len() == ts.len() {
                                    let mut e = *e2.clone();
                                    for (v, lit) in vs.iter().zip(ts) {
                                        if v != "_" {
                                            e = e.apply(&Expr::LitExpr(lit.clone()))?
                                        }
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(l2) => res.push(l2.clone()),
                                        _ => return Err(Error::new("eval, map-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, map-expression (tuple length mismatch)",
                                    ));
                                }
                            }
                            _ => {
                                if vs.len() == 1 {
                                    let mut e = *e2.clone();
                                    if vs[0] != "_" {
                                        e = e.apply(&Expr::LitExpr(l.clone()))?
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(l2) => res.push(l2.clone()),
                                        _ => return Err(Error::new("eval, map-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, map-expression (not a tuple list)",
                                    ));
                                }
                            }
                        }
                    }
                    Ok(Expr::LitExpr(Literal::List(res)))
                }
                _ => Err(Error::new("eval, map-expression")),
            },
            Expr::Iter(Iter::Filter, vs, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(Literal::List(lits)) => {
                    let mut res = Vec::new();
                    for l in lits.iter() {
                        match l {
                            Literal::Tuple(ts) if vs.len() != 1 => {
                                if vs.len() == ts.len() {
                                    let mut e = *e2.clone();
                                    for (v, lit) in vs.iter().zip(ts) {
                                        if v != "_" {
                                            e = e.apply(&Expr::LitExpr(lit.clone()))?
                                        }
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(Literal::BoolLiteral(b)) => {
                                            if b {
                                                res.push(l.clone());
                                            }
                                        }
                                        _ => return Err(Error::new("eval, filter-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, filter-expression (tuple length mismatch)",
                                    ));
                                }
                            }
                            _ => {
                                if vs.len() == 1 {
                                    let mut e = *e2.clone();
                                    if vs[0] != "_" {
                                        e = e.apply(&Expr::LitExpr(l.clone()))?
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(Literal::BoolLiteral(b)) => {
                                            if b {
                                                res.push(l.clone());
                                            }
                                        }
                                        _ => return Err(Error::new("eval, filter-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, filter-expression (not a tuple list)",
                                    ));
                                }
                            }
                        }
                    }
                    Ok(Expr::LitExpr(Literal::List(res)))
                }
                _ => Err(Error::new("eval, filter-expression")),
            },
            // op must by All or Any
            Expr::Iter(op, vs, e1, e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(Literal::List(lits)) => {
                    for l in lits.iter() {
                        match l {
                            Literal::Tuple(ts) if vs.len() != 1 => {
                                if vs.len() == ts.len() {
                                    let mut e = *e2.clone();
                                    for (v, lit) in vs.iter().zip(ts) {
                                        if v != "_" {
                                            e = e.apply(&Expr::LitExpr(lit.clone()))?
                                        }
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(Literal::BoolLiteral(b)) => {
                                            if b == (op == Iter::Any) {
                                                return Ok(Expr::bool(b));
                                            }
                                        }
                                        _ => return Err(Error::new("eval, all/any-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, all/any-expression (tuple length mismatch)",
                                    ));
                                }
                            }
                            _ => {
                                if vs.len() == 1 {
                                    let mut e = *e2.clone();
                                    if vs[0] != "_" {
                                        e = e.apply(&Expr::LitExpr(l.clone()))?
                                    }
                                    match e.eval(env)? {
                                        r @ Expr::ReturnExpr(_) => return Ok(r),
                                        Expr::LitExpr(Literal::BoolLiteral(b)) => {
                                            if b == (op == Iter::Any) {
                                                return Ok(Expr::bool(b));
                                            }
                                        }
                                        _ => return Err(Error::new("eval, all/any-expression")),
                                    }
                                } else {
                                    return Err(Error::new(
                                        "eval, all/any-expression (not a tuple list)",
                                    ));
                                }
                            }
                        }
                    }
                    Ok(Expr::bool(op == Iter::All))
                }
                _ => Err(Error::new("eval, all/any-expression")),
            },
            Expr::Closure(_) => Err(Error::new("eval, closure")),
            Expr::IfExpr {
                cond,
                consequence,
                alternative,
            } => match cond.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                Expr::LitExpr(Literal::BoolLiteral(b)) => {
                    if b {
                        match consequence.eval(env)? {
                            r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                            _ => Err(Error::new("eval, if-expression")),
                        }
                    } else {
                        match alternative {
                            None => Ok(Expr::LitExpr(Literal::Unit)),
                            Some(alt) => match alt.eval(env)? {
                                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                                _ => Err(Error::new("eval, if-expression")),
                            },
                        }
                    }
                }
                _ => Err(Error::new("eval, if-expression")),
            },
            Expr::IfMatchExpr {
                variables,
                matches,
                consequence,
                alternative,
            } => {
                let mut is_match = true;
                let mut caps: HashMap<String, Expr> = HashMap::new();
                for (e, re) in matches {
                    match e.eval(env)? {
                        r @ Expr::ReturnExpr(_) => return Ok(r),
                        Expr::LitExpr(Literal::StringLiteral(ref s)) => {
                            let names: Vec<&str> = re.0.capture_names().filter_map(|s| s).collect();
                            if names.len() == 0 {
                                if !re.0.is_match(s) {
                                    is_match = false;
                                    break;
                                }
                            } else {
                                match re.0.captures(s) {
                                    Some(cap) => {
                                        for name in names {
                                            let match_str = cap.name(name).unwrap().as_str();
                                            caps.insert(
                                                name.trim_start_matches('_').to_string(),
                                                if name.starts_with('_') {
                                                    Expr::i64(match_str.parse()?)
                                                } else {
                                                    Expr::string(match_str)
                                                },
                                            );
                                        }
                                    }
                                    _ => {
                                        is_match = false;
                                        break;
                                    }
                                }
                            }
                        }
                        _ => return Err(Error::new("eval, if-match-expression: type error")),
                    }
                }
                if is_match {
                    let mut c = *consequence;
                    for v in variables {
                        match caps.get(&v) {
                            Some(e) => c = c.apply(e)?,
                            None => {
                                return Err(Error::new("eval, if-match-expression: missing bind"));
                            }
                        }
                    }
                    match c.eval(env)? {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                        _ => Err(Error::new("eval, if-match-expression")),
                    }
                } else {
                    match alternative {
                        None => Ok(Expr::LitExpr(Literal::Unit)),
                        Some(alt) => match alt.eval(env)? {
                            r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                            _ => Err(Error::new("eval, if-match-expression")),
                        },
                    }
                }
            }
            Expr::CallExpr {
                function,
                arguments,
            } => {
                let args: Result<Vec<Expr>, self::Error> =
                    arguments.into_iter().map(|e| e.eval(env)).collect();
                let args = args?;
                match args.iter().find(|r| r.is_return()) {
                    Some(r) => Ok(r.clone()),
                    None => {
                        // user defined function
                        if let Some(e) = env.get(&function) {
                            let mut r = e.clone();
                            for a in args {
                                r = r.apply(&a)?
                            }
                            r.evaluate(env)
                        // builtin function
                        } else if Headers::is_builtin(&function) {
                            match args.as_slice() {
                                &[] => match Literal::eval_call0(&function) {
                                    Some(r) => Ok(Expr::LitExpr(r)),
                                    None => Err(Error::new("eval, call(0): type error")),
                                },
                                &[Expr::LitExpr(ref l)] => match l.eval_call1(&function) {
                                    Some(r) => Ok(Expr::LitExpr(r)),
                                    None => Err(Error::new("eval, call(1): type error")),
                                },
                                &[Expr::LitExpr(ref l1), Expr::LitExpr(ref l2)] => {
                                    match l1.eval_call2(&function, l2) {
                                        Some(r) => Ok(Expr::LitExpr(r)),
                                        None => Err(Error::new("eval, call(2): type error")),
                                    }
                                }
                                &[Expr::LitExpr(ref l1), Expr::LitExpr(ref l2), Expr::LitExpr(ref l3)] => {
                                    match l1.eval_call3(&function, l2, l3) {
                                        Some(r) => Ok(Expr::LitExpr(r)),
                                        None => Err(Error::new("eval, call(3): type error")),
                                    }
                                }
                                x => Err(Error::new(&format!("eval, call: {}: {:?}", function, x))),
                            }
                        } else {
                            // external function (RPC)
                            match function.split("::").collect::<Vec<&str>>().as_slice() {
                                &[external, method] => match env.externals().request(
                                    external,
                                    method,
                                    Literal::literal_vector(args)?,
                                ) {
                                    Ok(result) => Ok(Expr::LitExpr(result)),
                                    Err(err) => Err(Error::from(err)),
                                },
                                _ => Err(Error::new(&format!(
                                    "eval, call: {}: {:?}",
                                    function, args
                                ))),
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn evaluate(self, env: &mut Code) -> Result<Expr, self::Error> {
        self.eval(env).map(|e| e.strip_return())
    }
}
