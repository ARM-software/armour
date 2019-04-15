/// policy language interpreter
use super::lang::{Code, Error, Expr};
use super::parser::{Infix, Literal, Prefix};
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
            (Infix::Equal, l1, l2) => Some(Literal::BoolLiteral(l1 == l2)),
            (Infix::NotEqual, l1, l2) => Some(Literal::BoolLiteral(l1 != l2)),
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
            (Infix::Concat, Literal::StringLiteral(i), Literal::StringLiteral(j)) => {
                Some(Literal::StringLiteral(format!("{}{}", i, j)))
            }
            _ => None,
        }
    }
    fn eval_call1(&self, f: &str) -> Option<Self> {
        match (f, self) {
            ("i64::abs", Literal::IntLiteral(i)) => Some(Literal::IntLiteral(i.abs())),
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
            _ => None,
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
    fn eval(self, env: &Code) -> Result<Expr, self::Error> {
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
            Expr::BlockExpr(es) => {
                if es.len() == 0 {
                    Ok(Expr::LitExpr(Literal::Unit))
                } else {
                    let rs: Result<Vec<Expr>, self::Error> =
                        es.into_iter().rev().map(|e| e.eval(env)).collect();
                    let rs = rs?;
                    match rs.iter().find(|r| r.is_return()) {
                        Some(r) => Ok(r.clone()),
                        _ => Ok(rs.last().expect("eval, block").clone()),
                    }
                }
            }
            Expr::ClosureExpr(_, Some(e1), e2) => match e1.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                l @ Expr::LitExpr(_) => e2.subst(0, &l).eval(env),
                _ => Err(Error::new("eval, let-expression")),
            },
            Expr::ClosureExpr(_, None, _) => Err(Error::new("eval, closure")),
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
                    None => match env.get(&function) {
                        Some(e) => {
                            let mut r = e.clone();
                            for a in args {
                                r = r.apply(&a)?
                            }
                            r.evaluate(env)
                        }
                        None => match args.as_slice() {
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
                            x => Err(Error::new(&format!("eval, call: {}: {:?}", function, x))),
                        },
                    },
                }
            }
            Expr::InExpr { val, vals } => match val.eval(env)? {
                r @ Expr::ReturnExpr(_) => Ok(r),
                r @ Expr::LitExpr(_) => {
                    let vals: Result<Vec<Expr>, self::Error> =
                        vals.into_iter().map(|e| e.eval(env)).collect();
                    let vals = vals?;
                    match vals.iter().find(|r| r.is_return()) {
                        Some(r) => Ok(r.clone()),
                        _ => Ok(Expr::bool(vals.iter().any(|v| *v == r))),
                    }
                }
                _ => Err(Error::new("eval, in-expression")),
            },
        }
    }
    pub fn evaluate(self, env: &Code) -> Result<Expr, self::Error> {
        self.eval(env).map(|e| e.strip_return())
    }
}
