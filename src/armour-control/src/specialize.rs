// Specialize global policy
// NOTE: no optimization
use armour_lang::expressions::{Block, CPExpr, DPExpr, Error, Expr, self, Pattern};
use armour_lang::externals::{Call, ExternalActor};
use armour_lang::headers::{self, CPHeaders, Headers, THeaders};
use armour_lang::interpret::{CPEnv, TInterpret};
use armour_lang::labels::Label;
use armour_lang::lang::{Code, Program};
use armour_lang::literals::{
    self, Connection, HttpRequest, HttpResponse, Literal,
    FlatLiteral, CPLiteral, CPID,
    DPFlatLiteral, CPFlatLiteral, Method, OnboardingData,
    OnboardingResult, TFlatLiteral, VecSet
};
use armour_lang::meta::{Egress, IngressEgress, Meta};
use armour_lang::parser::{As, Infix, Iter, Pat, PolicyRegex, Prefix};
use armour_lang::policies;
use armour_lang::types::{self, TFlatTyp};
use armour_lang::types_cp::{CPFlatTyp};
use actix::prelude::*;
use futures::future::{BoxFuture, FutureExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use armour_api::control;
use armour_utils::{Client, parse_https_url};
use clap::{crate_version, App};
use futures::executor;
use super::rest_api::{collection, ONBOARDING_POLICY_KEY, POLICIES_COL, SERVICES_COL, State};
use super::interpret::{TSExprInterpret, TSLitInterpret};
use async_trait::async_trait;
use bson::doc;

macro_rules! cplit (
  ($i: ident ($($args:tt)*) ) => (
      Literal::FlatLiteral(CPFlatLiteral::$i($($args)*))
  );
);

#[async_trait]
pub trait TSExprPEval : Sized{
    fn peval(self, state: State, env: CPEnv) -> BoxFuture<'static, Result<(bool, Self), self::Error>>; 
    async fn pevaluate(self, state: &State, env: CPEnv) -> Result<(bool, Self), self::Error>; 
}

#[async_trait]
impl TSExprPEval for CPExpr {
    fn peval(self, state: State, env: CPEnv) -> BoxFuture<'static, Result<(bool, Self), self::Error>> {
        println!("### Peval, interpreting expression: ");
        self.print_debug();
        async { 
            match self {
                Expr::Var(_) | Expr::BVar(_, _) => Ok((false, self)),
                Expr::LitExpr(_) => Ok((true, self)),
                Expr::Closure(x, e) => {
                    let (_, e) = e.peval(state, env).await?;
                    Ok((false, Expr::Closure(x, Box::new(e))))    
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
                    _ => Err(Error::new("ppeval, prefix")),
                },
                // short circuit for &&
                Expr::InfixExpr(Infix::And, e1, e2) =>{ 
                    let (b1, n_e1) =  e1.peval(state.clone(), env.clone()).await?;
                    let (b2, n_e2) = e2.peval(state, env).await?;
                    let flag = b1 && b2; 

                    match n_e1 {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(false))) => Ok((flag, r)),
                        Expr::LitExpr(cplit!(Bool(true))) => match n_e2 {
                            r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(_))) => Ok((flag, r)),
                            _ => Err(Error::new("peval, infix")),
                        },
                        _ if !flag => Ok((flag, Expr::InfixExpr(Infix::And, Box::new(n_e1), Box::new(n_e2)))),
                        _ => Err(Error::new("peval, infix")),
                    }
                },
                // short circuit for ||
                Expr::InfixExpr(Infix::Or, e1, e2) => {
                    let (b1, n_e1) =  e1.peval(state.clone(), env.clone()).await?;
                    let (b2, n_e2) = e2.peval(state, env).await?;
                    let flag = b1 && b2; 

                    match n_e1 {
                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(true))) => Ok((flag, r)),
                        Expr::LitExpr(cplit!(Bool(false))) => match n_e2 {
                            r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(_))) => Ok((flag, r)),
                            _ => Err(Error::new("peval, infix")),
                        },
                        _ => Err(Error::new("peval, infix")),
                }
                },
                Expr::InfixExpr(op, e1, e2) => {
                    let r1 = e1.peval(state.clone(), env.clone()).await?;
                    let r2 = e2.peval(state, env).await?;
                    match (r1, r2) {
                        ((false, x), (_, y)) | ((_,x), (false, y)) => Ok((false, Expr::InfixExpr(op, Box::new(x), Box::new(y)))),
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
                Expr::Let(vs, e1, e2) => match e1.peval(state.clone(), env.clone()).await? {
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
                    (false, ee1) =>  Ok((false, Expr::Let(vs, Box::new(ee1), e2))),
                    _ => Err(Error::new("peval, let-expression")),
                },
                Expr::Iter(op, vs, e1, e2) => match e1.peval(state.clone(), env.clone()).await? {
                    (_, r @ Expr::ReturnExpr(_)) => Ok((true, r)),
                    (false, e1) => {
                        let (_, e2) = e2.peval(state.clone(), env.clone()).await?;
                        Ok((false, Expr::Iter(op, vs, Box::new(e1), Box::new(e2))))
                    }
                    (true, Expr::LitExpr(Literal::List(lits))) => {
                        let mut res = Vec::new();
                        for l in lits.iter() {
                            match l {
                                Literal::Tuple(ref ts) if vs.len() != 1 => {
                                    if vs.len() == ts.len() {
                                        let mut e = *e2.clone();
                                        for (v, lit) in vs.iter().zip(ts) {
                                            if v != "_" {
                                                e = e.apply(&Expr::LitExpr(lit.clone()))?
                                            }
                                        }
                                        res.push(e.peval(state.clone(), env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "peval, iter-expression (tuple length mismatch)",
                                        ));
                                    }
                                }
                                _ => {
                                    if vs.len() == 1 {
                                        let mut e = *e2.clone();
                                        if vs[0] != "_" {
                                            e = e.clone().apply(&Expr::LitExpr(l.clone()))?
                                        }
                                        res.push(e.peval(state.clone(), env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "peval, iter-expression (not a tuple list)",
                                        ));
                                    }
                                }
                            }
                        }
                        
                        let flag = res.iter().fold(true, |f, e| f && e.0 );
                        let mut res = res.into_iter().map(|e| e.1);

                        match res.find(|r| r.is_return()) {
                            Some(r) => Ok((flag, r.clone())),
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

                                    Iter::All => iter_lits.iter().all(|l| l.get_bool()).into(),
                                    Iter::Any => iter_lits.iter().any(|l| l.get_bool()).into(),
                                })),
                                Err(err) => Err(err),
                            },
                            None if !flag => Ok((false, Expr::Iter(op, vs, Box::new(Expr::LitExpr(Literal::List(lits))), e2))),
                            _ => unimplemented!("Could not happen in classical logic")
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
                                r.pevaluate(&state, env).await
                            } else if CPHeaders::is_builtin(&function) {
                                    Ok((flag, Expr::seval_call(
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
                                // user defined function
                                //partial evaluation + inlining
                                for a in args.into_iter().map(|x| x.1) {
                                    r = r.apply(&a)?
                                }
                                match r.pevaluate(&state, env).await {
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
                        _ => unimplemented!("Could not happen in classical logic")
                    }
                },
                Expr::Phantom(_) => unimplemented!()
            }
        }.boxed()
    }

    async fn pevaluate(self, state: &State, env: CPEnv) -> Result<(bool, Self), self::Error> {
        let (b, e) = self.peval(state.clone(), env).await?;
        Ok((b,e.strip_return()))
    }
}

pub async fn compile_ingress(state: &State, mut global_pol: policies::GlobalPolicies, function: &String, to: &CPID) -> Result<policies::DPPolicies, self::Error> {
    for (_, pol)  in (&mut global_pol).policies_mut() {
        let env = CPEnv::new(&pol.program);        

        //FIXME check correct type of http_rest_request
        //let sig = Typ::Signature(Some(vec![
        //    Typ::FlatTyp(FlatTyp::connection()), 
        //    Typ::FlatTyp(FlatTyp::i64()),
        //    Typ::FlatTyp(FlatTyp::i64())
        //]));

        //Typ::type_check (sig, pol.pprogram.code.get ()) 
        //FIXME how to do the type conversion from fexpr to typ
        //or just check that there is four argument

        match pol.program.code.get(policies::ALLOW_REST_REQUEST.to_string()) {
            None => return Err(Error::from(format!("compile_ingress, {} not define in global policy", policies::ALLOW_REST_REQUEST))), 
            Some(ref fexpr) => {    
                let fexpr = fexpr.clone().propagate_subst(2, 1, &Expr::LitExpr(Literal::id(to.clone()))); 
                //println!("#### After subst of 'to'");
                //fexpr.print_debug();
                match fexpr.pevaluate(state, env).await {                        
                    Ok((f, e)) =>{ 

                        let mut e = e.apply(&Expr::call("HttpRequest::from", vec![Expr::bvar("req", 0)]))?;
                        //e = e.apply(&Expr::call("HttpRequest::to", vec![Expr::bvar("req", 0)]))?;
                        e = e.apply(&Expr::bvar("req", 0))?;
                        e = e.apply(&Expr::bvar("payload", 1))?;

                        pol.program.code.insert(policies::ALLOW_REST_REQUEST.to_string(), e.clone()); 
                    },
                    Err(err) => return Err(err)
                }
            }
        }
    }
    Ok(policies::DPPolicies::from(global_pol))
}

pub async fn compile_egress(state: &State, mut global_pol: policies::GlobalPolicies, function: &String, from: &CPID) -> Result<policies::DPPolicies, self::Error> {
    for (_, pol)  in (&mut global_pol).policies_mut() {
        let env = CPEnv::new(&pol.program);        

        //FIXME check correct type of http_rest_request
        //let sig = Typ::Signature(Some(vec![
        //    Typ::FlatTyp(FlatTyp::connection()), 
        //    Typ::FlatTyp(FlatTyp::i64()),
        //    Typ::FlatTyp(FlatTyp::i64())
        //]));

        //Typ::type_check (sig, pol.pprogram.code.get ()) 
        //FIXME how to do the type conversion from fexpr to typ
        //or just check that there is four argument

        match pol.program.code.get(policies::ALLOW_REST_RESPONSE.to_string()) {
            None => return Err(Error::from(format!("compile_ingress, {} not define in global policy", policies::ALLOW_REST_RESPONSE))), 
            Some(ref fexpr) => {    
                let fexpr = fexpr.clone().propagate_subst(3, 0, &Expr::LitExpr(Literal::id(from.clone()))); 
                match fexpr.pevaluate(state, env).await {                        
                    Ok((f, e)) =>{ 

                        let mut e = e.apply(&Expr::call("HttpRequest::to", vec![Expr::bvar("req", 0)]))?;
                        e = e.apply(&Expr::bvar("req", 0))?;
                        e = e.apply(&Expr::bvar("payload", 1))?;

                        pol.program.code.insert(policies::ALLOW_REST_RESPONSE.to_string(), e.clone()); 
                    },
                    Err(err) => return Err(err)
                }
            }
        }
    }
    Ok(policies::DPPolicies::from(global_pol))
}