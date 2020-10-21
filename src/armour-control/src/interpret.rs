/// onboarding policy language interpreter
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
use armour_lang::specialize;
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
use async_trait::async_trait;
use bson::doc;




fn to_bson<T: ?Sized>(value: &T) -> Result<bson::Bson, self::Error>
where
    T: serde::Serialize,
{
    bson::to_bson(value).on_err("Bson conversion error")
}

pub async fn present(
    col: &mongodb::Collection,
    filter: impl Into<Option<bson::Document>>,
) -> Result<bool, self::Error> {
    use futures::StreamExt;
    Ok(col
        .find(filter, None)
        .await
        .on_err("MongoDB query error")?
        .next()
        .await
        .is_some())
}

//Workaround since we can not do impl CPFLat.. since not in the same crate 
#[async_trait]
pub trait TSLitInterpret {
    fn seval_call0(state: State, f: &str) -> Option<CPLiteral>;
    fn seval_call1(&self, state:State, f: &str) -> Option<CPLiteral>;
    async fn seval_call2(&self, state: State, f: &str, other: &Self) -> Option<CPLiteral>;
    async fn seval_call3(&self, state: State, f: &str, l1: &Self, l2: &Self) -> Option<CPLiteral>;
    async fn seval_call4(&self, state: State, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<CPLiteral>;
    async fn helper_sevalexpr(state: State, e : Expr<CPFlatTyp, CPFlatLiteral>, env: CPEnv) -> Result<CPExpr, self::Error>;
}

#[async_trait]
pub trait TSExprInterpret : Sized{
    async fn seval_call(state: State, function: &str, args: Vec<Self>) -> Result<Self, self::Error>;
    fn seval(self, state: State, env: CPEnv) -> BoxFuture<'static, Result<Self, self::Error>>;
    async fn sevaluate(self, state: &State, env: CPEnv) -> Result<Self, self::Error>;
}

macro_rules! cpflatlit (
  ($i: ident ($($args:tt)*) ) => (
        CPFlatLiteral::$i($($args)*)
  );
);
macro_rules! cplit (
  ($i: ident ($($args:tt)*) ) => (
      Literal::FlatLiteral(CPFlatLiteral::$i($($args)*))
  );
);

trait OnErr<T, E>
where
    Self: Into<Result<T, E>>,
{
    fn on_err(self, b: &str) -> Result<T, self::Error> {
        self.into().map_err(|_| self::Error::new(b.to_string()))
    }
}
impl<T> OnErr<T, bson::de::Error> for bson::de::Result<T> {}
impl<T> OnErr<T, bson::ser::Error> for bson::ser::Result<T> {}
impl<T> OnErr<T, mongodb::error::Error> for mongodb::error::Result<T> {}

fn tt<T, E>(x:Result<Option<T>,E>) -> Option<T> {
    match x {
        Ok(x) => x,
        _ => None
    }
 }

async fn helper_compile_ingress(state: State, function: &String, id: &CPID) ->  Result<Option<CPLiteral>, self::Error> {
    let col = collection(&state, POLICIES_COL);
    if let Ok(Some(doc)) = col
        .find_one(Some(doc! {"label" : ONBOARDING_POLICY_KEY}), None)
        .await
    {
        let global_pol=
            bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc))
                .on_err("Bson conversion error")?;
        Ok(Some(specialize::compile_ingress(global_pol.policy, function, id)))
    } else {
        Ok(None) //TODO
    }
}
async fn helper_compile_egress(state: State, function: &String, id: &CPID) ->  Result<Option<CPLiteral>, self::Error> {
    let col = collection(&state, POLICIES_COL);
    if let Ok(Some(doc)) = col
        .find_one(Some(doc! {"label" : ONBOARDING_POLICY_KEY}), None)
        .await
    {
        let global_pol=
            bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc))
                .on_err("Bson conversion error")?;
        Ok(Some(specialize::compile_egress(global_pol.policy, function, id)))
    } else {
        Ok(None) //TODO
    }
}

async fn helper_onboarded(state: State, service: &Label, host: &Label) ->  Result<Option<CPLiteral>, self::Error>  {
    let col = collection(&state, SERVICES_COL);

    //FIXME we assume that (service, host) is unique
    if let Ok(Some(doc)) = col
        .find_one(Some(doc! { "service" : to_bson(&service)?, "host" : to_bson(&host)? }), None)
        .await
    {
        let request =
            bson::from_bson::<control::POnboardServiceRequest>(bson::Bson::Document(doc))
                .on_err("Bson conversion error")?;
        Ok(Some(cplit!(Label(request.service_id)).some()))
    } else {
        Ok(Some(Literal::none()))
    }
}

async fn helper_onboard(state: State, service: &Label, service_id: &Label, host: &Label) ->  Result<Option<CPLiteral>, self::Error>  {
    let request = control::POnboardServiceRequest {
        service: service.clone(),
        service_id: service_id.clone(),
        host: host.clone()
    };                       
    let col = collection(&state, SERVICES_COL);

    // Check if the service is already there
    if present(&col, doc! { "service_id" : to_bson(service_id)? }).await? {
        Ok(Some(cplit!(Bool(true))))

    } else if let bson::Bson::Document(document) = to_bson(&request)? {
        col.insert_one(document, None) // Insert into a MongoDB collection
            .await
            .on_err("error inserting in MongoDB")?;
        Ok(Some(cplit!(Bool(true))))

    } else {
        Ok(Some(cplit!(Bool(false))))
    }
}


#[async_trait]
impl TSLitInterpret for CPLiteral {
    fn seval_call0(state: State, f: &str) -> Option<CPLiteral> {
        match f {
            _ => Self::eval_call0(f),
        }
    }
    fn seval_call1(&self, state:State, f: &str) -> Option<CPLiteral> {
        match (f, self) {
            _ => self.eval_call1(f),
        }
    }
    async fn seval_call2(&self, state: State, f: &str, other: &Self) -> Option<CPLiteral> {
        match (f, self, other) {
            ("compile_ingress", cplit!(Str(function)), cplit!(ID(id))) => {
                tt(helper_compile_ingress(state, function, id).await)
            },
            ("compile_egress", cplit!(Str(function)), cplit!(ID(id))) =>  {
                tt(helper_compile_egress(state, function, id).await)
            },
            (
                "ControlPlane::onboarded", 
                cplit!(Label(service)), 
                cplit!(Label(host))
            ) => { 
                tt(helper_onboarded(state, service, host).await)
            }
            (
                "ControlPlane::newID", 
                cplit!(Label(service)), 
                cplit!(Label(host))
            ) => { 
                let service_id = Label::concat(host, service);//TODO refine newid
                Some(cplit!(Label(service_id)))
            }
            _ => self.eval_call2(f, other),
        }
    }
    async fn seval_call3(&self, state: State, f: &str, l1: &Self, l2: &Self) -> Option<CPLiteral> {
        match (f, self, l1, l2) {
            (
                "ControlPlane::onboard",
                cplit!(Label(service)),
                cplit!(Label(service_id)), 
                cplit!(Label(host))
            ) => { 
                tt(helper_onboard(state, service, service_id, host).await)
            }
            _ => self.eval_call3(f, l1, l2),
        }
    }
    #[allow(clippy::many_single_char_names)]
    async fn seval_call4(&self, state:State, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Option<CPLiteral> {
        match (f, self, l1, l2, l3) {
            _ => self.eval_call4(f, l1, l2, l3),
        }
    }
    async fn helper_sevalexpr(state: State, e : Expr<CPFlatTyp, CPFlatLiteral>, env: CPEnv) -> Result<CPExpr, self::Error>{
        match e {
            // short circuit for &&
            Expr::InfixExpr(Infix::And, e1, e2) => match e1.seval(state.clone(), env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(false))) => Ok(r),
                Expr::LitExpr(cplit!(Bool(true))) => match e2.seval(state, env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            // short circuit for ||
            Expr::InfixExpr(Infix::Or, e1, e2) => match e1.seval(state.clone(), env.clone()).await? {
                r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(true))) => Ok(r),
                Expr::LitExpr(cplit!(Bool(false))) => match e2.seval(state, env).await? {
                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(cplit!(Bool(_))) => Ok(r),
                    _ => Err(Error::new("eval, infix")),
                },
                _ => Err(Error::new("eval, infix")),
            },
            _ => unimplemented!()
        }
    }
}

#[async_trait]
impl TSExprInterpret for CPExpr {
    async fn seval_call(state: State, function: &str, args: Vec<Self>) -> Result<Self, self::Error> {
        // builtin function
        match args.as_slice() {
            [] => match Literal::seval_call0(state, function) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(0): type error")),
            },
            [Expr::LitExpr(l1)] => match l1.seval_call1(state, &function) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(1): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2)] => match l1.seval_call2(state, &function, l2).await {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(2): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3)] => {
                match l1.seval_call3(state, &function, l2, l3).await {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(3): type error")),
                }
            }
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3), Expr::LitExpr(l4)] => {
                match l1.seval_call4(state, &function, l2, l3, l4).await {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(4): type error")),
                }
            }
            x => Err(Error::from(format!("eval, call: {}: {:?}", function, x))),
        }
    }
    #[allow(clippy::cognitive_complexity)]
    fn seval(self, state: State, env: CPEnv) -> BoxFuture<'static, Result<Self, self::Error>> {
        async {
            match self {
                Expr::Var(_) | Expr::BVar(_, _) => Err(Error::new("eval variable")),
                Expr::LitExpr(_) => Ok(self),
                Expr::Closure(_, _) => Err(Error::new("eval, closure")),

                Expr::ReturnExpr(e) => Ok(Expr::return_expr(e.seval(state, env).await?)),
                Expr::PrefixExpr(p, e) => match e.seval(state, env).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(l) => match l.eval_prefix(&p) {
                        Some(r) => Ok(r.into()),
                        None => Err(Error::new("eval prefix: type error")),
                    },
                    _ => Err(Error::new("eval, prefix")),
                },
                // short circuit for &&
                Expr::InfixExpr(Infix::And, _, _) => CPLiteral::helper_sevalexpr(state, self, env).await, 
                // short circuit for ||
                Expr::InfixExpr(Infix::Or, _, _) => CPLiteral::helper_sevalexpr(state, self, env).await,
                Expr::InfixExpr(op, e1, e2) => {
                    let r1 = e1.seval(state.clone(), env.clone()).await?;
                    match (r1, e2.seval(state, env).await?) {
                        (r @ Expr::ReturnExpr(_), _) => Ok(r),
                        (_, r @ Expr::ReturnExpr(_)) => Ok(r),
                        (Expr::LitExpr(l1), Expr::LitExpr(l2)) => match l1.eval_infix(&op, &l2) {
                            Some(r) => Ok(r.into()),
                            None => Err(Error::new("eval, infix: type error")),
                        },
                        _ => Err(Error::new("eval, infix: failed")),
                    }
                }
                Expr::BlockExpr(b, mut es) => {
                    if es.is_empty() {
                        Ok(Expr::LitExpr(if b == Block::List {
                            Literal::List(Vec::new())
                        } else {
                            Literal::unit()
                        }))
                    } else if b == Block::Block {
                        let e = es.remove(0);
                        let res = e.seval(state.clone(), env.clone()).await?;
                        if res.is_return() || es.is_empty() {
                            Ok(res)
                        } else {
                            Expr::BlockExpr(b, es).seval(state, env).await
                        }
                    } else {
                        // list or tuple
                        let mut rs = Vec::new();
                        for e in es.into_iter() {
                            rs.push(e.seval(state.clone(), env.clone()).await?)
                        }
                        match rs.iter().find(|r| r.is_return()) {
                            Some(r) => Ok(r.clone()),
                            _ => match Self::literal_vector(rs) {
                                Ok(lits) => Ok((if b == Block::List {
                                    Literal::List(lits)
                                } else {
                                    Literal::Tuple(lits)
                                })
                                .into()),
                                Err(err) => Err(err),
                            },
                        }
                    }
                }
                Expr::Let(vs, e1, e2) => match e1.seval(state.clone(), env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::Tuple(lits)) => {
                        let lits_len = lits.len();
                        if 1 < lits_len && vs.len() == lits_len {
                            let mut e2a = *e2.clone();
                            for (v, lit) in vs.iter().zip(lits) {
                                if v != "_" {
                                    e2a = e2a.apply(&Expr::LitExpr(lit))?
                                }
                            }
                            e2a.seval(state, env).await
                        } else if vs.len() == 1 {
                            e2.apply(&Expr::LitExpr(Literal::Tuple(lits)))?
                                .seval(state, env)
                                .await
                        } else {
                            Err(Error::new("eval, let-expression (tuple length mismatch)"))
                        }
                    }
                    l @ Expr::LitExpr(_) => {
                        if vs.len() == 1 {
                            e2.apply(&l)?.seval(state, env).await
                        } else {
                            Err(Error::new("eval, let-expression (literal not a tuple)"))
                        }
                    }
                    _ => Err(Error::new("eval, let-expression")),
                },
                Expr::Iter(op, vs, e1, e2) => match e1.seval(state.clone(), env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::List(lits)) => {
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
                                        res.push(e.seval(state.clone(), env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "eval, iter-expression (tuple length mismatch)",
                                        ));
                                    }
                                }
                                _ => {
                                    if vs.len() == 1 {
                                        let mut e = *e2.clone();
                                        if vs[0] != "_" {
                                            e = e.clone().apply(&Expr::LitExpr(l.clone()))?
                                        }
                                        res.push(e.seval(state.clone(), env.clone()).await?)
                                    } else {
                                        return Err(Error::new(
                                            "eval, iter-expression (not a tuple list)",
                                        ));
                                    }
                                }
                            }
                        }
                        match res.iter().find(|r| r.is_return()) {
                            Some(r) => Ok(r.clone()),
                            None => match Self::literal_vector(res) {
                                Ok(iter_lits) => match op {
                                    Iter::Map => Ok(Literal::List(iter_lits).into()),
                                    Iter::ForEach => Ok(Expr::from(())),
                                    Iter::Filter => {
                                        let filtered_lits = lits
                                            .into_iter()
                                            .zip(iter_lits.into_iter())
                                            .filter_map(
                                                |(l, b)| if b.get_bool() { Some(l) } else { None },
                                            )
                                            .collect();
                                        Ok(Literal::List(filtered_lits).into())
                                    }
                                    Iter::FilterMap => {
                                        let filtered_lits = iter_lits
                                            .iter()
                                            .filter_map(Literal::dest_some)
                                            .collect();
                                        Ok(Literal::List(filtered_lits).into())
                                    }
                                    Iter::All => Ok(iter_lits.iter().all(|l| l.get_bool()).into()),
                                    Iter::Any => Ok(iter_lits.iter().any(|l| l.get_bool()).into()),
                                },
                                Err(err) => Err(err),
                            },
                        }
                    }
                    _ => Err(Error::new("eval, map-expression")),
                },
                Expr::IfExpr {
                    cond,
                    consequence,
                    alternative,
                } => match cond.seval(state.clone(), env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && fl.get_bool() => consequence.seval(state, env).await,
                    Expr::LitExpr(Literal::FlatLiteral(fl)) if fl.is_bool() && !fl.get_bool() => match alternative {
                        Some(alt) => alt.seval(state, env).await,
                        None => Ok(Expr::from(())),
                    },
                    _ => Err(Error::new("eval, if-expression")),
                },
                Expr::IfSomeMatchExpr {
                    expr,
                    consequence,
                    alternative,
                } => match expr.seval(state.clone(), env.clone()).await? {
                    r @ Expr::ReturnExpr(_) => Ok(r),
                    Expr::LitExpr(Literal::Tuple(t)) => {
                        if t.len() == 1 {
                            match consequence.apply(&Expr::LitExpr(t[0].clone())) {
                                Ok(consequence_apply) => consequence_apply.seval(state, env).await,
                                Err(e) => Err(e),
                            }
                        } else {
                            match alternative {
                                Some(alt) => alt.seval(state, env).await,
                                None => Ok(Expr::from(())),
                            }
                        }
                    }
                    r => Err(Error::from(format!("eval, if-let-expression: {:#?}", r))),
                },
                Expr::IfMatchExpr {
                    variables,
                    matches,
                    consequence,
                    alternative,
                } => {
                    let mut rs = Vec::new();
                    for (e, re) in matches.into_iter() {
                        if let Some(r) = e.seval(state.clone(), env.clone()).await?.perform_match(re) {
                            rs.push(r)
                        } else {
                            return Err(Error::new("eval, if-match-expression: type error"));
                        }
                    }
                    match rs.iter().find(|(r, _captures)| r.is_return()) {
                        // early exit
                        Some((r, _captures)) => Ok(r.clone()),
                        None => {
                            if rs.iter().any(|(_r, captures)| captures.is_none()) {
                                // failed match
                                match alternative {
                                    None => Ok(Expr::from(())),
                                    Some(alt) => match alt.seval(state, env).await? {
                                        r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                                        _ => Err(Error::new("eval, if-match-expression")),
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
                                            "eval, if-match-expression: missing bind",
                                        ));
                                    }
                                }
                                match c.seval(state, env).await? {
                                    r @ Expr::ReturnExpr(_) | r @ Expr::LitExpr(_) => Ok(r),
                                    _ => Err(Error::new("eval, if-match-expression")),
                                }
                            }
                        }
                    }
                }
                Expr::CallExpr {
                    function,
                    arguments,
                    is_async,
                } => {
                    let mut args = Vec::new();
                    for e in arguments.into_iter() {
                        args.push(e.seval(state.clone(), env.clone()).await?)
                    }
                    match args.iter().find(|r| r.is_return()) {
                        Some(r) => Ok(r.clone()),
                        None => {
                            if let Some(mut r) = env.get(&function) {
                                // user defined function
                                for a in args {
                                    r = r.apply(&a)?
                                }
                                r.sevaluate(&state, env).await
                            } else if CPHeaders::is_builtin(&function) {
                                Expr::seval_call(state, function.as_str(), args).await
                            } else if let Some((external, method)) = CPHeaders::split(&function) {
                                // external function (RPC) or "Ingress/Egress" metadata
                                let args = Self::literal_vector(args)?;
                                let call = Call::new(external, method, args);
                                if external == "Ingress" || external == "Egress" {
                                    env.meta
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("Metadata call error"))?
                                } else if is_async {
                                    Arbiter::spawn(env.external.send(call).then(|res| {
                                        match res {
                                            Ok(Err(e)) => log::warn!("{}", e),
                                            Err(e) => log::warn!("{}", e),
                                            _ => (),
                                        };
                                        async {}
                                    }));
                                    Ok(Expr::from(()))
                                } else {
                                    env.external
                                        .send(call)
                                        .await
                                        .map_err(|_| Error::new("capnp error"))?
                                }
                            } else {
                                Err(Error::from(format!("eval, call: {}: {:?}", function, args)))
                            }
                        }
                    }
                },
                Expr::Phantom(_) => unimplemented!()
            }
        }
        .boxed()
    }
    async fn sevaluate(self, state: &State, env: CPEnv) -> Result<Self, self::Error> {
        Ok(self.seval(state.clone(), env).await?.strip_return())
    }
}

