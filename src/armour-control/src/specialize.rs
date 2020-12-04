// Specialize global policy
use armour_lang::{cpdplit};
use armour_lang::expressions::{Block, CPExpr, Error, Expr};
use armour_lang::headers::{CPHeaders, THeaders};
use armour_lang::interpret::{CPEnv, TExprInterpreter, TInterpret};
use armour_lang::literals::{
    Literal,
    CPID,
    CPFlatLiteral, DPFlatLiteral,
    TFlatLiteral 
};
use armour_lang::parser::{Ident, Infix, Iter};
use armour_lang::policies;
use armour_lang::types::{CPFlatTyp, Signature, Typ, TTyp};
use async_trait::async_trait;
use futures::future::{BoxFuture, FutureExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use super::interpret::{CPExprWrapper};
use super::State;

#[async_trait]
pub trait TSExprPEval : Sized{
    fn peval(
        self, 
        state: Arc<State>, 
        env: CPEnv,
        simplification_only: bool
    ) -> BoxFuture<'static, Result<(bool, Self), self::Error>>; 

    async fn pevaluate(
        self, 
        state: Arc<State>, 
        env: CPEnv,
        simplification_only: bool
    ) -> Result<(bool, Self), self::Error>; 
}

macro_rules! ring_simplification (
    ($ring: ident, $plus1: ident, $multiply1: ident,  $plus2: ident, $multiply2: ident, $zero: expr, $one: expr) => (
        |
            op: Infix<CPFlatTyp>,
            b1: bool,
            n_e1: CPExpr, 
            b2: bool,
            n_e2: CPExpr
        | -> Result<(bool, bool, CPExpr), self::Error> {
            if op == Infix::$plus1 || op == Infix::$plus2 {
                match n_e1 {
                    r @ Expr::ReturnExpr(_) => Ok((true, b1, r)),
                    Expr::LitExpr(x) if x == cpdplit!($ring($zero)) => Ok((true, b2, n_e2)),
                    _ => match n_e2 {
                        r2 @ Expr::ReturnExpr(_) => Ok((true, b2, r2)),
                        Expr::LitExpr(x) if x == cpdplit!($ring($zero)) => Ok((true, b1, n_e1)),
                        _ => Ok((false, b1 && b2, Expr::InfixExpr(op, Box::new(n_e1), Box::new(n_e2)))), 
                    },
                }
            }
            else if op == Infix::$multiply1 || op == Infix::$multiply2 {
                match n_e1 {
                    r @ Expr::ReturnExpr(_) => Ok((true, b1, r)),
                    Expr::LitExpr(x) if x == cpdplit!($ring($zero)) => Ok((true, b1, Expr::LitExpr(x))),
                    Expr::LitExpr(x) if x == cpdplit!($ring($one)) => Ok((true, b2, n_e2)),
                    _ => match n_e2 {
                        r2 @ Expr::ReturnExpr(_) => Ok((true, b2, r2)),
                        Expr::LitExpr(x) if x == cpdplit!($ring($zero)) => Ok((true, b2, Expr::LitExpr(x))),
                        Expr::LitExpr(x) if x == cpdplit!($ring($one)) => Ok((true, b1, n_e1)),
                        _ => Ok((false, b1 && b2, Expr::InfixExpr(op, Box::new(n_e1), Box::new(n_e2)))), 
                    },
                }
            } else {
                Ok((false, b1 && b2, Expr::InfixExpr(op, Box::new(n_e1), Box::new(n_e2)))) 
            }
        }
    );
);

fn combine_simplification(
    simpl1: impl Fn(Infix<CPFlatTyp>, bool, CPExpr, bool, CPExpr) -> Result<(bool, bool, CPExpr), self::Error>,
    simpl2: impl Fn(Infix<CPFlatTyp>, bool, CPExpr, bool, CPExpr) -> Result<(bool, bool, CPExpr), self::Error>
) -> impl Fn(Infix<CPFlatTyp>, bool, CPExpr, bool, CPExpr) -> Result<(bool, bool, CPExpr), self::Error>
{
    move |op: Infix<CPFlatTyp>,
    b1: bool,
    n_e1: CPExpr,
    b2: bool,
    n_e2: CPExpr| 
    -> Result<(bool, bool, CPExpr), self::Error> { 
        match simpl1(op.clone(), b1.clone(), n_e1.clone(), b2.clone(), n_e2.clone()) {
            Ok((false, _, _)) => simpl2(op, b1, n_e1, b2, n_e2),
            r => r
        }
    }
}

fn simplify(
    op: Infix<CPFlatTyp>,
    b1: bool,
    n_e1: CPExpr, 
    b2: bool,
    n_e2: CPExpr
) -> Result<(bool, CPExpr), self::Error> {
    match (op, n_e2) {
        (Infix::Divide, Expr::LitExpr(cpdplit!(Int(0)))) => 
            return Err(Error::new("peval, can not divide by zero")),
        (Infix::Divide, Expr::LitExpr(x)) if x == cpdplit!(Float(0.)) => 
            return Err(Error::new("peval, can not divide by zero")),
        //Syntaxic check for equality    
        (Infix::Equal, n_e2) if n_e1 == n_e2 => Ok((b1 && b2, Expr::LitExpr(cpdplit!(Bool(true))))), 
        (Infix::NotEqual, n_e2) if n_e1 != n_e2 => Ok((b1 && b2, Expr::LitExpr(cpdplit!(Bool(true))))),
        (op, n_e2) => {
            let t = combine_simplification(
                ring_simplification!(Bool, Or, And, Or, And, false, true),
                combine_simplification(
                    ring_simplification!(Bool, And, Or, And, Or, true, false),
                    combine_simplification(
                        ring_simplification!(Int, Plus, Multiply, Minus, Divide, 0, 1),
                        ring_simplification!(Float, Plus, Multiply, Minus, Divide, 0., 1.)
                    )
                )
            );

            let tmp = t(op, b1, n_e1, b2, n_e2)?;
            Ok((tmp.1, tmp.2))
        }
    }
}


#[async_trait]
impl TSExprPEval for CPExpr {

    fn peval(self, 
        state: Arc<State>, 
        env: CPEnv,
        simplification_only: bool//if true, prevent call evaluation FIXME allow all non side effect call
    ) -> BoxFuture<'static, Result<(bool, Self), self::Error>> {
        async move { 
            let simplification_only = simplification_only.clone();
            match self {
                Expr::Var(_) | Expr::BVar(_, _) => Ok((false, self)),
                Expr::LitExpr(_) => Ok((true, self)),
                Expr::Closure(x, e) => {
                    let (_, e) = e.peval(state, env, simplification_only).await?;
                    if e.is_free(0) {
                        Ok((true, e))    
                    } else {
                        Ok((false, Expr::Closure(x, Box::new(e))))    
                    }
                },

                Expr::ReturnExpr(e) =>{
                    let (b, expr) = e.peval(state, env, simplification_only).await?;
                    Ok((b, Expr::return_expr(expr)))
                },
                Expr::PrefixExpr(p, e) => match e.peval(state, env, simplification_only).await? {
                    (true, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                    (true, Expr::LitExpr(l)) => match l.eval_prefix(&p) {
                        Some(r) => Ok((true, r.into())),
                        None => Err(Error::new("peval prefix: type error")),
                    },
                    (false, n_e) => Ok((false, Expr::PrefixExpr(p, Box::new(n_e)))),//evaluation delayed
                    _ => Err(Error::new("peval, prefix")),
                },
                Expr::InfixExpr(op, e1, e2) => {
                    let r1 = e1.peval(state.clone(), env.clone(), simplification_only).await?;
                    let r2 = e2.peval(state, env, simplification_only).await?;
                    match (r1, r2) {
                        ((b1 @ false, x), (b2 , y)) | ((b1, x), (b2 @ false, y)) => Ok(
                            simplify(op, b1, x, b2, y)?
                        ),
                        ((true, x), (true, y)) => {
                            match (x, y) {
                                (r @ Expr::ReturnExpr(_), _) => Ok((true, r)),
                                (_, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                                (Expr::LitExpr(l1), Expr::LitExpr(l2)) => match l1.eval_infix(&op, &l2) {
                                    Some(r) => Ok((true, r.into())),
                                    None => Err(Error::new("peval, infix: type error")),
                                },
                                _ => Err(Error::new("peval, infix: failed")),
                            }
                        }
                    }
                }
                Expr::BlockExpr(b, mut es) => {
                    if es.is_empty() {
                        Ok((true, Expr::LitExpr(if b == Block::List {
                            Literal::List(Vec::new())
                        } else {
                            Literal::unit()
                        })))
                    } else if b == Block::Block {
                        let e = es.remove(0);
                        match e.peval(state.clone(), env.clone(), simplification_only).await? {
                            (false, res) => {
                                if res.is_return() || es.is_empty() {
                                    Ok((false, res))
                                } else {
                                    match Expr::BlockExpr(b.clone(), es).peval(state.clone(), env.clone(), true).await?.1 {
                                        Expr::BlockExpr(_, mut es) => {
                                            es.insert(0, res);
                                            Ok((false, Expr::BlockExpr(b, es)))
                                        }
                                        e => Ok((false, Expr::BlockExpr(b, vec![res, e]))),
                                    }
                                }
                            },
                            (true, res) => {
                                if res.is_return() || es.is_empty() {
                                    Ok((true, res))
                                } else {
                                    Expr::BlockExpr(b, es).peval(state, env, simplification_only).await
                                }
                            },
                        }
                    } else {
                        // list or tuple
                        let mut rs = Vec::new();
                        let mut flag = true;
                        for e in es.into_iter() {
                            let (f, res) = e.peval(state.clone(), env.clone(), simplification_only).await?;
                            flag = flag && f;
                            rs.push(res);
                        }

                        match rs.iter().find(|r| r.is_return()) {
                            Some(r) => Ok((flag, r.clone())),
                            _ => match Self::literal_vector(rs) {
                                Ok(lits) => Ok((flag, (if b == Block::List {
                                    Literal::List(lits)
                                } else {
                                    Literal::Tuple(lits)
                                })
                                .into())),
                                Err(err) => Err(err),
                            },
                        }
                    }
                }
                Expr::Let(vs, e1, e2) =>{
                    let mut flag = true;
                    let body = e2.clone().at_depth(vs.len()).unwrap();
                    for u in 0..vs.len(){
                        flag = flag && body.is_free(u);
                        if !flag { break };
                    }

                    if flag { //e2 is independant of the let-bindings
                        //Getting ride of the closures
                        let mut e2a = *e2;
                        for _ in 0..vs.len() {
                            e2a = e2a.apply(&Expr::LitExpr(Literal::unit()))?;
                        }

                        e2a.peval(state, env, simplification_only).await
                    } else {
                        match e1.peval(state.clone(), env.clone(), simplification_only).await? {
                            (flag, r @ Expr::ReturnExpr(_)) => Ok((flag, r)),
                            (true, Expr::LitExpr(Literal::Tuple(lits))) => {
                                let lits_len = lits.len();
                                if 1 < lits_len && vs.len() == lits_len {
                                    let mut e2a = *e2.clone();
                                    for (v, lit) in vs.iter().zip(lits) {
                                        if v != "_" {
                                            e2a = e2a.apply(&Expr::LitExpr(lit))?
                                        }
                                    }
                                    e2a.peval(state, env, simplification_only).await
                                } else if vs.len() == 1 {
                                    e2.apply(&Expr::LitExpr(Literal::Tuple(lits)))?
                                        .peval(state, env, simplification_only)
                                        .await
                                } else {
                                    Err(Error::new("peval, let-expression (tuple length mismatch)"))
                                }
                            }
                            (true, l @ Expr::LitExpr(_)) => {
                                if vs.len() == 1 {
                                    e2.apply(&l)?.peval(state, env, simplification_only).await
                                } else {
                                    Err(Error::new("peval, let-expression (literal not a tuple)"))
                                }
                            },
                            (false, ee1) =>  Ok((
                                false, 
                                Expr::Let(vs, Box::new(ee1), Box::new(e2.peval(state, env, simplification_only).await?.1))
                            )),
                            _ => Err(Error::new("peval, let-expression")),
                        }
                    }
                },
                Expr::Iter(op, vs, e1, e2, acc_opt) => match e1.peval(state.clone(), env.clone(), simplification_only).await? {
                    (_, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                    (false, e1) => {
                        let (_, e2) = e2.peval(state.clone(), env.clone(), simplification_only).await?;
                        let acc_opt = match acc_opt{
                            Some((acc_name, acc)) => Some((acc_name, Box::new(acc.peval(state.clone(), env.clone(), simplification_only).await?.1))),
                            None => None
                        }; 

                        Ok((false, Expr::Iter(op, vs, Box::new(e1), Box::new(e2), acc_opt)))
                    }
                    (true, Expr::LitExpr(Literal::List(lits))) => {
                        let mut res = Vec::new();
                        let acc_name_opt = acc_opt.clone().map(|x| x.0); 
                        let mut acc_opt = match acc_opt {
                            Some((acc_name,e)) =>{
                                match e.peval(state.clone(), env.clone(), simplification_only).await? {
                                    (true, acc) => Some((true, acc)), 
                                    (false, acc) => 
                                        //Fold can not be applied if acc is not a value
                                        return Ok((
                                            false, 
                                            Expr::Iter(
                                                op, 
                                                vs, 
                                                Box::new(Expr::LitExpr(Literal::List(lits))), 
                                                e2, 
                                                Some((acc_name, Box::new(acc)))
                                            )
                                        )
                                    )
                                }
                            },
                            _=> None
                        };
                        for l in lits.iter() {
                            let mut e = *e2.clone();
                            
                            //Apply the accumulator if any
                            if acc_opt.is_some() {
                                let acc = acc_opt.clone().unwrap().1;
                                e = e.apply(&acc)?;
                            }

                            //Apply l to e
                            match l {
                                Literal::Tuple(ref ts) if vs.len() != 1 => {
                                    if vs.len() == ts.len() {
                                        for (v, lit) in vs.iter().zip(ts) {
                                            if v != "_" {
                                                e = e.apply(&Expr::LitExpr(lit.clone()))?
                                            }
                                        }
                                    } else {
                                        return Err(Error::new(
                                            "peval, iter-expression (tuple length mismatch)",
                                        ));
                                    }
                                }
                                _ => {
                                    if vs.len() == 1 {
                                        if vs[0] != "_" {
                                            e = e.clone().apply(&Expr::LitExpr(l.clone()))?
                                        }
                                    } else {
                                        return Err(Error::new(
                                            "peval, iter-expression (not a tuple list)",
                                        ));
                                    }
                                }
                            }

                            //Update the acc if any
                            if acc_opt.is_some() {
                                let tmp = e.peval(state.clone(), env.clone(), simplification_only).await?;
                                acc_opt = Some(tmp.clone());
                                res.push(tmp)    
                            } else {
                                res.push(e.peval(state.clone(), env.clone(), simplification_only).await?)
                            }
                        }
                        
                        let flag = res.iter().fold(true, |f, e| f && e.0 );
                        let mut res = res.into_iter().map(|e| e.1);

                        match res.find(|r| r.is_return()) {
                            Some(r) => Ok((flag, r.clone())),
                            None if op == Iter::Fold => {
                                Ok(acc_opt.unwrap())
                            }
                            None if flag => match Self::literal_vector(res.collect()) {
                                Ok(iter_lits) => Ok((flag, match op {
                                    Iter::Map => Literal::List(iter_lits).into(),
                                    Iter::ForEach => Expr::from(()),
                                    Iter::Filter => {
                                        let filtered_lits = lits
                                            .into_iter()
                                            .zip(iter_lits.into_iter())
                                            .filter_map(
                                                |(l, b)| if b.get_bool() { Some(l) } else { None },
                                            )
                                            .collect();
                                        Literal::List(filtered_lits).into()
                                    }
                                    Iter::FilterMap => {
                                        let filtered_lits = iter_lits
                                            .iter()
                                            .filter_map(Literal::dest_some)
                                            .collect();
                                        Literal::List(filtered_lits).into()
                                    }
                                    Iter::Fold => unreachable!(),
                                    Iter::All => iter_lits.iter().all(|l| l.get_bool()).into(),
                                    Iter::Any => iter_lits.iter().any(|l| l.get_bool()).into(),
                                })),
                                Err(err) => Err(err),
                            },
                            None if !flag => Ok((
                                false, 
                                Expr::Iter(
                                    op,
                                    vs, 
                                    Box::new(Expr::LitExpr(Literal::List(lits))), 
                                    e2, 
                                    acc_opt.map(|x| (acc_name_opt.unwrap(), Box::new(x.1)))
                                )
                            )),
                            _ => unreachable!("Could not happen in classical logic")
                        }
                    }
                    _ => Err(Error::new("peval, map-expression")),
                },
                Expr::IfExpr {
                    cond,
                    consequence,
                    alternative,
                } => match cond.peval(state.clone(), env.clone(), simplification_only).await? {
                    (flag, r @ Expr::ReturnExpr(_)) => Ok((flag,r)),
                    (true, conda) => match conda {
                        Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && fl.get_bool() => consequence.peval(state, env, simplification_only).await,
                        Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && !fl.get_bool() => match alternative {
                            Some(alt) => alt.peval(state, env, simplification_only).await,
                            None => Ok((true, Expr::from(()))),
                        },
                        _ => Err(Error::new("peval, if-expression")),
                    },
                    (false, cond1) => {                                                        
                        let (bc, consequence1) = consequence.peval(state.clone(), env.clone(), simplification_only).await?;
                        match alternative {
                            Some(alt) =>{
                                let (_, tmp) = alt.peval(state.clone(), env.clone(), simplification_only).await?;
                                //Syntaxic if elimination
                                if tmp == consequence1 {
                                    return Ok((bc, consequence1))
                                } else {
                                    Ok((false, Expr::IfExpr {
                                        cond: Box::new(cond1),
                                        consequence: Box::new(consequence1),
                                        alternative: Some(Box::new(tmp)),
                                    } ))
                                }                           
                            },
                            None => Ok((false, Expr::IfExpr {
                                cond: Box::new(cond1),
                                consequence: Box::new(consequence1),
                                alternative: None,
                            } ))
                        }
                    } 
                },
                Expr::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => match expr.peval(state.clone(), env.clone(), simplification_only).await? {
                    (flag, r @ Expr::ReturnExpr(_)) => Ok((flag, r)),
                    (true, Expr::LitExpr(Literal::Tuple(t))) => {
                        if t.len() == 1 {
                            match consequence.apply(&Expr::LitExpr(t[0].clone())) {
                                Ok(consequence_apply) => consequence_apply.peval(state, env, simplification_only).await,
                                Err(e) => Err(e),
                            }
                        } else {
                            match alternative {
                                Some(alt) => alt.peval(state, env, simplification_only).await,
                                None => Ok((true, Expr::from(()))),
                            }
                        }
                    },
                    (false, expr1) => {                            
                        let (c_var, c_body)= match *consequence {
                            Expr::Closure(c_var, c_body) => (c_var, c_body),
                            _ => unreachable!()
                        };

                        let (_, c_body_1) = c_body.peval(state.clone(), env.clone(), simplification_only).await?;

                        match alternative {
                            Some(alt) =>{
                                let (b0, tmp) = alt.peval(state.clone(), env.clone(), simplification_only).await?;
                                let consequence1 = Expr::Closure(c_var, Box::new(c_body_1.clone()));
                                //Syntaxic IfSomeMatch elimination
                                if c_body_1.is_free(0) && consequence1.clone().apply(&Expr::LitExpr(Literal::unit()))? == tmp {//Dummy apply                                     
                                    Ok((b0, tmp))
                                } else {
                                    Ok((false, Expr::IfSomeMatchExpr {
                                        expr: Box::new(expr1),
                                        consequence: Box::new(consequence1),
                                        alternative: Some(Box::new(tmp)),
                                    }))
                                }
                            },
                            None => {
                                let consequence1 = Expr::Closure(c_var, Box::new(c_body_1));
                                Ok((false, Expr::IfSomeMatchExpr {
                                    expr: Box::new(expr1),
                                    consequence: Box::new(consequence1),
                                    alternative: None,
                                }))
                            }
                        }                            
                    }
                    (_, r) => Err(Error::from(format!("peval, if-let-expression: {:#?}", r))),
                },
                Expr::IfMatchExpr {
                    variables,
                    matches,
                    consequence,
                    alternative,
                } => {
                    let mut rs = Vec::new();
                    let mut flag = true;

                    for (e, re) in matches.clone().into_iter() {
                        let (f, tmp) = e.peval(state.clone(), env.clone(), simplification_only).await?; 
                        flag = flag && f;
                        if let Some(r) = tmp.perform_match(re) {
                            rs.push(r)
                        } else {
                            return Err(Error::new("peval, if-match-expression: type error"));
                        }
                    }

                    if !flag {
                        return Ok((flag, Expr::IfMatchExpr {
                            variables,
                            matches,
                            consequence,
                            alternative,
                        }))
                    };

                    if flag {
                        match rs.iter().find(|(r, _captures)| r.is_return()) {
                            // early exit
                            Some((r, _captures)) => Ok((true, r.clone())),
                            None => {
                                if rs.iter().any(|(_r, captures)| captures.is_none()) {
                                    // failed match
                                    match alternative {
                                        None => Ok((true, Expr::from(()))),
                                        Some(alt) => match alt.peval(state, env, simplification_only).await? {
                                            (f, r @ Expr::ReturnExpr(_)) | (f, r @ Expr::LitExpr(_)) => Ok((f, r)),
                                            _ => Err(Error::new("peval, if-match-expression")),
                                        },
                                    }
                                } else {
                                    // match
                                    let mut all_captures: BTreeMap<String, Self> = BTreeMap::new();
                                    for (_r, captures) in rs {
                                        if let Some(caps) = captures {
                                            all_captures.extend(caps)
                                        }
                                    }
                                    let mut c = *consequence;
                                    for v in variables {
                                        if let Some(e) = all_captures.get(&v) {
                                            c = c.apply(e)?
                                        } else {
                                            return Err(Error::new(
                                                "peval, if-match-expression: missing bind",
                                            ));
                                        }
                                    }
                                    match c.peval(state, env, simplification_only).await? {
                                        (f, r @ Expr::ReturnExpr(_)) | (f, r @ Expr::LitExpr(_)) => Ok((f,r)),
                                        _ => Err(Error::new("peval, if-match-expression")),
                                    }
                                }
                            }
                        }
                    } else {
                        let (_, consequence1) = consequence.peval(state.clone(), env.clone(), simplification_only).await?;
                        let alternative = match alternative {
                            Some(alt) =>{
                                let (_, tmp) = alt.peval(state.clone(), env.clone(), simplification_only).await?;
                                Some(Box::new(tmp))
                            },
                            None => None
                        };
                        Ok((false, Expr::IfMatchExpr {
                            variables,
                            matches,
                            consequence: Box::new(consequence1),
                            alternative,
                        }))
                    } 
                }
                Expr::CallExpr {
                    function,
                    arguments,
                    is_async,
                } => {
                    let mut args = Vec::new();
                    for e in arguments.clone().into_iter() {
                        args.push(e.peval(state.clone(), env.clone(), simplification_only).await?)
                    }                        
                    let flag = args.iter().fold(true, |f, e| f && e.0);
                    match args.iter().find(|r| r.1.is_return()) {
                        Some(r) => Ok((flag, r.1.clone())),
                        None if flag && !simplification_only => {
                            if let Some(mut r) = env.get(&function) {
                                // user defined function
                                for a in args.into_iter().map(|x| x.1) {
                                    r = r.apply(&a)?
                                }
                                r.pevaluate(state, env, simplification_only).await
                            } else if CPHeaders::is_builtin(&function) {
                                    Ok((flag, CPExprWrapper::eval_call(
                                        state,
                                        function.as_str(),
                                        args.into_iter().map(|(_, e)| e).collect()
                                    ).await?))
                            } else if let Some((_, _)) = CPHeaders::split(&function) {
                                // external function (RPC) or "Ingress/Egress" metadata
                                // should be evaluated dynamically 
                                Ok((false, Expr::CallExpr {
                                    function,
                                    arguments,
                                    is_async,
                                }))
                            } else {
                                Err(Error::from(format!("peval, call: {}: {:?}", function, args)))
                            }
                        },
                        None if !flag || simplification_only => {
                            if let Some(mut r) = env.get(&function) {
                                //user defined function
                                //partial evaluation + inlining
                                for a in args.into_iter().map(|x| x.1) {
                                    r = r.apply(&a)?
                                }
                                match r.pevaluate(state, env, simplification_only).await {
                                    Ok((_, r)) => Ok((false, r)),
                                    err => err
                                }
                            } else { //can not be partially evaluated                                    
                                Ok((false, Expr::CallExpr {
                                    function: function,
                                    arguments: args.into_iter().map(|e| e.1).collect(),
                                    is_async: is_async,
                                })) 
                            }
                        },
                        _ => unreachable!("Could not happen in classical logic")
                    }
                },
                Expr::Phantom(_) => unreachable!()
            }
        }.boxed()
    }

    async fn pevaluate(
        self, 
        state: Arc<State>, 
        env: CPEnv,
        simplification_only: bool
    ) -> Result<(bool, Self), self::Error> {
        let (b, e) = self.peval(state.clone(), env, simplification_only).await?;
        Ok((b,e.strip_return()))
    }
}

fn check_header(
    pol: &policies::GlobalPolicy, 
    function: &str, 
) -> Result<usize, self::Error> {
        let expected_sig = match function {
            policies::ALLOW_REST_REQUEST => Signature::new(
                vec![ Typ::id(), Typ::id(), Typ::http_request(), Typ::data() ], 
                Typ::bool()
            ),
            policies::ALLOW_REST_RESPONSE => Signature::new(
                vec![ Typ::id(), Typ::id(), Typ::http_response(), Typ::data() ], 
                Typ::bool()
            ),
            policies::ALLOW_TCP_CONNECTION => Signature::new(
                vec![Typ::id(), Typ::id(), Typ::connection()],
                Typ::bool()
            ),
            policies::ON_TCP_DISCONNECT => Signature::new(
                vec![ Typ::id(), Typ::id(), Typ::connection(), Typ::i64(), Typ::i64() ],
                Typ::bool()
            ),
            _ => return Err(Error::from(format!(
                "unknown main function to specialize: {}", 
                function
            )))
        };
        match pol.program.headers.get(function) {
            None => 
                Err(Error::from(format!(
                    "specialization  checking headers, {} is undefined in global policy", 
                    function
                ))), 
            Some(sig) if *sig != expected_sig => 
                Err(Error::from(format!(
                    "specialization  checking headers, {} has a wrong signature\n{}\nexpected\n{}",
                    function,
                    sig,
                    expected_sig
                ))),
            _ => Ok(expected_sig.args().unwrap().len())
        }
}

fn compile_helper(
    pol: &policies::GlobalPolicy, 
    function: &str, 
    mut n_pol: policies::GlobalPolicy
) -> Result<policies::GlobalPolicy, self::Error>  {
    //Update headers
    //unwrap is safe since we are actually working on existing fct
    let ret_typ = pol.program.headers.return_typ(&function.to_string()).unwrap(); 
    let sig = match function {
        policies::ALLOW_REST_REQUEST =>
            Signature::new(vec![Typ::http_request(), Typ::data()], ret_typ),
        policies::ALLOW_REST_RESPONSE =>
            Signature::new(vec![Typ::http_response(), Typ::data()], ret_typ),
        policies::ALLOW_TCP_CONNECTION =>
            Signature::new(vec![Typ::connection()], ret_typ),
        policies::ON_TCP_DISCONNECT =>
            Signature::new(vec![Typ::connection(), Typ::i64(), Typ::i64()], ret_typ),
        _ => return Err(Error::from(format!(
            "unknown main function to specialize: {}", 
            function
        )))
    };
    n_pol.program.headers.insert(function.to_string(), sig.clone());

    //Update fn_policies
    n_pol.fn_policies.set_args(function.to_string(), sig.args().unwrap().len() as u8);

    //Deadcode elimination
    n_pol.program = n_pol.program.deadcode_elim(&vec![function.to_string()][..])?;
    Ok(n_pol)
}
async fn compile_egress_ingress(
    f_egress: bool, //true => compile_egress, false => compile_ingress
    state: Arc<State>, 
    global_pol: policies::GlobalPolicies, 
    function: &str, 
    to: &CPID
) -> Result<policies::DPPolicies, self::Error> {
    let mut new_gpol = policies::GlobalPolicies::default();
    for (proto, pol)  in (&global_pol).policies() {
        let env = CPEnv::new(&pol.program);        
        match pol.program.code.get(function.to_string()) {
            None =>  {
                log::warn!(
                    "compile_{}, {} is undefined in global policy", 
                    if f_egress {"egress"} else {"ingress"},
                    function
                );
            } 
            Some(ref fexpr) => {    
                //Checking header type
                let n_args = check_header(pol, function)?;

                //Replacing the "from" (resp "to") variable by the ID of the µservice
                let fexpr = fexpr.clone().propagate_subst(
                    if f_egress {n_args-1} else {n_args-2}, 
                    if f_egress {0} else {1}, 
                    &Expr::LitExpr(Literal::id(to.clone()))
                ); 


                let body = match fexpr.at_depth(n_args-1) {
                   Some(e) => e,
                    _ => unreachable!("check_header prevent this from happening"),
                };

                match body.pevaluate(state.clone(), env, false).await {                        
                    Ok((_, e)) =>{ 
                        let mut n_pol = policies::GlobalPolicy::default();                            

                        //FIXME the following can be shared
                        let e = match function {
                            policies::ALLOW_REST_REQUEST | policies::ALLOW_REST_RESPONSE => {
                                let get_from_name = if function == policies::ALLOW_REST_REQUEST {
                                    format!("HttpRequest::{}", if f_egress {"to"} else {"from"})
                                } else {
                                    format!("HttpResponse::{}", if f_egress {"to"} else {"from"})
                                };

                                let e = e.subst(2, &Expr::call(&get_from_name[..], vec![Expr::bvar("req", 1)]), false);

                                Expr::Closure(
                                    Ident("req".to_string()), 
                                    Box::new(Expr::Closure(
                                        Ident("payload".to_string()), 
                                        Box::new(e)
                                    ))
                                )
                            },
                            policies::ALLOW_TCP_CONNECTION => {                                    
                                let e = e.subst(1,
                                    &Expr::call(
                                        &format!("Connection::{}", if f_egress {"to"} else {"from"} )[..],
                                        vec![Expr::bvar("conn", 0)]
                                    ),
                                    false
                                );

                                Expr::Closure(
                                    Ident("con".to_string()), 
                                    Box::new(e)
                                )
                            }, 
                            policies::ON_TCP_DISCONNECT => {                                    
                                let e = Expr::Closure(
                                    Ident((if f_egress {"to"} else {"from"}).to_string()),
                                    Box::new(Expr::Closure(
                                        Ident("req".to_string()),
                                        Box::new(Expr::Closure(
                                            Ident("i".to_string()),
                                            Box::new(Expr::Closure(
                                                Ident("j".to_string()),
                                                Box::new(e)
                                            ))
                                        ))
                                    ))
                                );

                                let e = e.subst(3, 
                                    &Expr::call(
                                        &format!("Connection::{}", if f_egress {"to"} else {"from"} )[..],
                                        vec![Expr::bvar("conn", 2)],
                                    ),
                                    false
                                );

                                Expr::Closure(
                                    Ident("conn".to_string()), 
                                    Box::new(Expr::Closure(
                                        Ident("i".to_string()), 
                                        Box::new(Expr::Closure(
                                            Ident("j".to_string()),
                                            Box::new(e)
                                        ))
                                    ))
                                )
                            } 
                            _ => return Err(Error::from(format!(
                                "unknown main function to specialize: {}", 
                                function
                            )))
                        };

                        n_pol.program.code.insert(function.to_string(), e.clone()); 
                        
                        new_gpol.insert(
                            proto.clone(), 
                            compile_helper(pol, function, n_pol)?
                        );
                    },
                    Err(err) => return Err(err)
                }
            }
        }
    }
    Ok(policies::DPPolicies::from(new_gpol))
}

pub async fn compile_egress(
    state: Arc<State>, 
    global_pol: policies::GlobalPolicies, 
    function: &str, 
    from: &CPID
) -> Result<policies::DPPolicies, self::Error> {
    compile_egress_ingress(true, state, global_pol, function, from).await
}

pub async fn compile_ingress(
    state: Arc<State>, 
    global_pol: policies::GlobalPolicies, 
    function: &str, 
    to: &CPID
) -> Result<policies::DPPolicies, self::Error> {
    compile_egress_ingress(false, state, global_pol, function, to).await
}