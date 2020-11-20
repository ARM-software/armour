// Specialize global policy
use actix::prelude::*;
use armour_lang::{cpdplit};
use armour_lang::expressions::{Block, CPExpr, Error, Expr};
use armour_lang::externals::{Call};
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
use armour_lang::types::{Signature, Typ, TTyp};
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
        env: CPEnv
    ) -> BoxFuture<'static, Result<(bool, Self), self::Error>>; 

    async fn pevaluate(
        self, 
        state: Arc<State>, 
        env: CPEnv
    ) -> Result<(bool, Self), self::Error>; 
}

#[async_trait]
impl TSExprPEval for CPExpr {

    fn peval(self, 
        state: Arc<State>, 
        env: CPEnv
    ) -> BoxFuture<'static, Result<(bool, Self), self::Error>> {
        async { 
            match self {
                Expr::Var(_) | Expr::BVar(_, _) => Ok((false, self)),
                Expr::LitExpr(_) => Ok((true, self)),
                Expr::Closure(x, e) => {
                    let (_, e) = e.peval(state, env).await?;
                    if e.is_free(0) {
                        Ok((true, e))    
                    } else {
                        Ok((false, Expr::Closure(x, Box::new(e))))    
                    }
                },

                Expr::ReturnExpr(e) =>{
                    let (b, expr) = e.peval(state, env).await?;
                    Ok((b, Expr::return_expr(expr)))
                },
                Expr::PrefixExpr(p, e) => match e.peval(state, env).await? {
                    (true, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                    (true, Expr::LitExpr(l)) => match l.eval_prefix(&p) {
                        Some(r) => Ok((true, r.into())),
                        None => Err(Error::new("peval prefix: type error")),
                    },
                    (false, n_e) => Ok((false, Expr::PrefixExpr(p, Box::new(n_e)))),//evaluation delayed
                    _ => Err(Error::new("peval, prefix")),
                },
                // short circuit for &&
                Expr::InfixExpr(Infix::And, e1, e2) =>{ 
                    let (b1, n_e1) =  e1.peval(state.clone(), env.clone()).await?;
                    let (b2, n_e2) = e2.peval(state, env).await?;

                    match n_e1 {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(false))) => Ok((b1, r)),
                        Expr::LitExpr(cpdplit!(Bool(true))) => Ok((b2, n_e2)),
                        _ if !b1 =>  match n_e2 {
                            r2 @ Expr::ReturnExpr(_) | r2 @ Expr::LitExpr(cpdplit!(Bool(false))) => Ok((b2, r2)),
                            Expr::LitExpr(cpdplit!(Bool(true))) => Ok((b1, n_e1)),
                            _ => Ok((b1 || b2, Expr::InfixExpr(Infix::And, Box::new(n_e1), Box::new(n_e2)))),
                        },
                        _ => Err(Error::new("peval, && infix")),
                    }
                },
                // short circuit for ||
                Expr::InfixExpr(Infix::Or, e1, e2) => {
                    let (b1, n_e1) =  e1.peval(state.clone(), env.clone()).await?;
                    let (b2, n_e2) = e2.peval(state, env).await?;

                    match n_e1 {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cpdplit!(Bool(true))) => Ok((b1, r)),
                        Expr::LitExpr(cpdplit!(Bool(false))) => Ok((b2, n_e2)),
                        _ if !b1 => match n_e2 {
                            Expr::LitExpr(cpdplit!(Bool(false))) => Ok((b1, n_e1)),
                            r2 @ Expr::ReturnExpr(_) | r2 @ Expr::LitExpr(cpdplit!(Bool(true))) => Ok((b2, r2)),
                            _ => Ok((b1 || b2, Expr::InfixExpr(Infix::Or, Box::new(n_e1), Box::new(n_e2)))),
                        },
                        _ => Err(Error::new("peval, infix")),
                }
                },
                Expr::InfixExpr(op, e1, e2) => {
                    let r1 = e1.peval(state.clone(), env.clone()).await?;
                    let r2 = e2.peval(state, env).await?;
                    match (r1, r2) {
                        ((false, x), (_, y)) | ((_,x), (false, y)) => Ok(
                            (false, Expr::InfixExpr(op, Box::new(x), Box::new(y)))
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
                Expr::BlockExpr(b, es) => {
                    if es.is_empty() {
                        Ok((true, Expr::LitExpr(if b == Block::List {
                            Literal::List(Vec::new())
                        } else {
                            Literal::unit()
                        })))
                    } else if b == Block::Block {
                        let e = es.clone().remove(0);
                        match e.peval(state.clone(), env.clone()).await? {
                            (false, res) => {
                                if res.is_return() || es.is_empty() {
                                    Ok((false, res))
                                } else {
                                    Expr::BlockExpr(b, es).peval(state, env).await
                                }
                            },
                            (true, res) => {
                                if res.is_return() || es.is_empty() {
                                    Ok((true, res))
                                } else {
                                    Expr::BlockExpr(b, es).peval(state, env).await
                                }
                            },
                        }
                    } else {
                        // list or tuple
                        let mut rs = Vec::new();
                        let mut flag = true;
                        for e in es.into_iter() {
                            let (f, res) = e.peval(state.clone(), env.clone()).await?;
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
                    for u in 0..vs.len(){
                        flag = flag && e2.is_free(u);
                        if !flag { break };
                    }

                    if flag { //e2 is independant of the let-bindings
                        e2.peval(state, env).await
                    } else {
                        match e1.peval(state.clone(), env.clone()).await? {
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
                                    e2a.peval(state, env).await
                                } else if vs.len() == 1 {
                                    e2.apply(&Expr::LitExpr(Literal::Tuple(lits)))?
                                        .peval(state, env)
                                        .await
                                } else {
                                    Err(Error::new("peval, let-expression (tuple length mismatch)"))
                                }
                            }
                            (true, l @ Expr::LitExpr(_)) => {
                                if vs.len() == 1 {
                                    e2.apply(&l)?.peval(state, env).await
                                } else {
                                    Err(Error::new("peval, let-expression (literal not a tuple)"))
                                }
                            },
                            (false, ee1) =>  Ok((
                                false, 
                                Expr::Let(vs, Box::new(ee1), Box::new(e2.peval(state, env).await?.1))
                            )),
                            _ => Err(Error::new("peval, let-expression")),
                        }
                    }
                },
                Expr::Iter(op, vs, e1, e2, acc_opt) => match e1.peval(state.clone(), env.clone()).await? {
                    (_, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                    (false, e1) => {
                        let (_, e2) = e2.peval(state.clone(), env.clone()).await?;
                        let acc_opt = match acc_opt{
                            Some(acc) => Some(Box::new(acc.peval(state.clone(), env.clone()).await?.1)),
                            None => None
                        }; 

                        Ok((false, Expr::Iter(op, vs, Box::new(e1), Box::new(e2), acc_opt)))
                    }
                    (true, Expr::LitExpr(Literal::List(lits))) => {
                        let mut res = Vec::new();
                        let mut acc_opt = match acc_opt {
                            Some(e) =>{
                                match e.peval(state.clone(), env.clone()).await? {
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
                                                Some(Box::new(acc))
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
                                let tmp = e.peval(state.clone(), env.clone()).await?;
                                acc_opt = Some(tmp.clone());
                                res.push(tmp)    
                            } else {
                                res.push(e.peval(state.clone(), env.clone()).await?)
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
                                    acc_opt.map(|x| Box::new(x.1))
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
                } => match cond.peval(state.clone(), env.clone()).await? {
                    (flag, r @ Expr::ReturnExpr(_)) => Ok((flag,r)),
                    (true, conda) => match conda {
                        Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && fl.get_bool() => consequence.peval(state, env).await,
                        Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && !fl.get_bool() => match alternative {
                            Some(alt) => alt.peval(state, env).await,
                            None => Ok((true, Expr::from(()))),
                        },
                        _ => Err(Error::new("peval, if-expression")),
                    },
                    (false, cond1) => {                                                        
                        let (_, consequence1) = consequence.peval(state.clone(), env.clone()).await?;
                        let alternative = match alternative {
                            Some(alt) =>{
                                let (_, tmp) = alt.peval(state.clone(), env.clone()).await?;
                                Some(Box::new(tmp))
                            },
                            None => None
                        };
                        Ok((false, Expr::IfExpr {
                            cond: Box::new(cond1),
                            consequence: Box::new(consequence1),
                            alternative,
                        } ))
                    } 
                },
                Expr::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => match expr.peval(state.clone(), env.clone()).await? {
                    (flag, r @ Expr::ReturnExpr(_)) => Ok((flag, r)),
                    (true, Expr::LitExpr(Literal::Tuple(t))) => {
                        if t.len() == 1 {
                            match consequence.apply(&Expr::LitExpr(t[0].clone())) {
                                Ok(consequence_apply) => consequence_apply.peval(state, env).await,
                                Err(e) => Err(e),
                            }
                        } else {
                            match alternative {
                                Some(alt) => alt.peval(state, env).await,
                                None => Ok((true, Expr::from(()))),
                            }
                        }
                    },
                    (false, expr1) => {                            
                        let (_, consequence1) = consequence.peval(state.clone(), env.clone()).await?;
                        let alternative = match alternative {
                            Some(alt) =>{
                                let (_, tmp) = alt.peval(state.clone(), env.clone()).await?;
                                Some(Box::new(tmp))
                            },
                            None => None
                        };
                        Ok((false, Expr::IfSomeMatchExpr {
                            expr: Box::new(expr1),
                            consequence: Box::new(consequence1),
                            alternative,
                        }))
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
                        let (f, tmp) = e.peval(state.clone(), env.clone()).await?; 
                        flag = flag && f;
                        if let Some(r) = tmp.perform_match(re) {
                            rs.push(r)
                        } else {
                            return Err(Error::new("peval, if-match-expression: type error"));
                        }
                    }
                    if flag {
                        match rs.iter().find(|(r, _captures)| r.is_return()) {
                            // early exit
                            Some((r, _captures)) => Ok((true, r.clone())),
                            None => {
                                if rs.iter().any(|(_r, captures)| captures.is_none()) {
                                    // failed match
                                    match alternative {
                                        None => Ok((true, Expr::from(()))),
                                        Some(alt) => match alt.peval(state, env).await? {
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
                                    match c.peval(state, env).await? {
                                        (f, r @ Expr::ReturnExpr(_)) | (f, r @ Expr::LitExpr(_)) => Ok((f,r)),
                                        _ => Err(Error::new("peval, if-match-expression")),
                                    }
                                }
                            }
                        }
                    } else {
                        let (_, consequence1) = consequence.peval(state.clone(), env.clone()).await?;
                        let alternative = match alternative {
                            Some(alt) =>{
                                let (_, tmp) = alt.peval(state.clone(), env.clone()).await?;
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
                    for e in arguments.into_iter() {
                        args.push(e.peval(state.clone(), env.clone()).await?)
                    }                        
                    let flag = args.iter().fold(true, |f, e| f && e.0);
                    match args.iter().find(|r| r.1.is_return()) {
                        Some(r) => Ok((flag, r.1.clone())),
                        None if flag => {
                            if let Some(mut r) = env.get(&function) {
                                // user defined function
                                for a in args.into_iter().map(|x| x.1) {
                                    r = r.apply(&a)?
                                }
                                r.pevaluate(state, env).await
                            } else if CPHeaders::is_builtin(&function) {
                                    Ok((flag, CPExprWrapper::eval_call(
                                        state,
                                        function.as_str(),
                                        args.into_iter().map(|(_, e)| e).collect()
                                    ).await?))
                            } else if let Some((external, method)) = CPHeaders::split(&function) {
                                // external function (RPC) or "Ingress/Egress" metadata
                                let args = Self::literal_vector(args.into_iter().map(|x| x.1).collect())?;
                                let call = Call::new(external, method, args);
                                if external == "Ingress" || external == "Egress" {
                                    match env.meta
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("Metadata call error"))?
                                    {
                                        Ok(e) => Ok((flag, e)),
                                        Err(err) => Err(err)
                                    }
                                } else if is_async {
                                    Arbiter::spawn(env.external.send(call).then(|res| {
                                        match res {
                                            Ok(Err(e)) => log::warn!("{}", e),
                                            Err(e) => log::warn!("{}", e),
                                            _ => (),
                                        };
                                        async {}
                                    }));
                                    Ok((flag, Expr::from(())))
                                } else {
                                    match env.external
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("capnp error"))?
                                    {
                                        Ok(e) => Ok((flag, e)),
                                        Err(err) => Err(err)
                                    }
                                }
                            } else {
                                Err(Error::from(format!("peval, call: {}: {:?}", function, args)))
                            }
                        },
                        None if !flag => {
                            if let Some(mut r) = env.get(&function) {
                                //user defined function
                                //partial evaluation + inlining
                                for a in args.into_iter().map(|x| x.1) {
                                    r = r.apply(&a)?
                                }
                                match r.pevaluate(state, env).await {
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
        env: CPEnv
    ) -> Result<(bool, Self), self::Error> {
        let (b, e) = self.peval(state.clone(), env).await?;
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
            None =>  return Err(Error::from(format!(
                "compile_{}, {} is undefined in global policy", 
                if f_egress {"egress"} else {"ingress"},
                function
            ))), 
            Some(ref fexpr) => {    
                //Checking header type
                let n_args = check_header(pol, function)?;

                //Replacing the "from" (resp "to") variable by the ID of the µservice
                //http_rest_request: must be true at call time => from is known
                //http_rest_response: must be true at return time => to is known ??
                let fexpr = fexpr.clone().propagate_subst(
                    if f_egress {n_args-1} else {n_args-2}, 
                    if f_egress {0} else {1}, 
                    &Expr::LitExpr(Literal::id(to.clone()))
                ); 


                let body = match fexpr.at_depth(n_args-1) {
                   Some(e) => e,
                    _ => unreachable!("check_header prevent this from happening"),
                };

                match body.pevaluate(state.clone(), env).await {                        
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

                                let mut e = e.subst(2, &Expr::call(&get_from_name[..], vec![Expr::bvar("req", 1)]), false);
                                //e = e.subst(1, &Expr::bvar("req", 1), false);
                                //e = e.subst(0, &Expr::bvar("payload", 0), false);

                                Expr::Closure(
                                    Ident("req".to_string()), 
                                    Box::new(Expr::Closure(
                                        Ident("payload".to_string()), 
                                        Box::new(e)
                                    ))
                                )
                            },
                            policies::ALLOW_TCP_CONNECTION => {                                    
                                let mut e = e.subst(1,
                                    &Expr::call(
                                        &format!("Connection::{}", if f_egress {"to"} else {"from"} )[..],
                                        vec![Expr::bvar("conn", 0)]
                                    ),
                                    false
                                );
                                //e = e.subst(0, &Expr::bvar("conn", 0), false);

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

                                let mut e = e.subst(3, 
                                    &Expr::call(
                                        &format!("Connection::{}", if f_egress {"to"} else {"from"} )[..],
                                        vec![Expr::bvar("conn", 2)],
                                    ),
                                    false
                                );
                                //e = e.subst(0, &Expr::bvar("conn", 2), false);
                                //e = e.subst(0, &Expr::bvar("i", 1), false);
                                //e = e.subst(0, &Expr::bvar("j", 0), false);

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