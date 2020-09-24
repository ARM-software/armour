// Control plane policy language
use super::{headers, lexer, parser} ;
use super::expressions::{Error, Expr, TExpr, ExprAndMeta, Context, ReturnType};
use super::types_cp::{CPTyp};
use super::types::{Typ, Signature};
use headers::Headers;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap};
use std::marker::PhantomData;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum CPExpr {
    DPExpr(Expr),
}
impl From<&Expr> for CPExpr {
    fn from(e: &Expr) -> Self {
        CPExpr::DPExpr(e.clone())
    }
}
impl From<Expr> for CPExpr {
    fn from(e: Expr) -> Self {
        CPExpr::DPExpr(e)
    }
}
impl From<CPTyp> for Typ {
    fn from(t: CPTyp) -> Self {
        match t {
            CPTyp::DPTyp(t) => t,
            _ => panic!("Not yet implemented!! For now OnboardingData and  result can not be a return type.")
        }
    }
}

impl From<&Signature<parser::CPTyp, CPTyp>> for Signature<parser::Typ, Typ> {
    fn from(s: &Signature<parser::CPTyp, CPTyp>) -> Self {
        match s.clone().args() { 
            None => Signature::any(Typ::from(s.clone().typ())),
            Some(ts) => Signature::new(ts.into_iter().map(|t| Typ::from(t)).collect(), Typ::from(s.clone().typ()))
        }
    }
}

impl From<&Headers<parser::CPTyp, CPTyp>> for Headers<parser::Typ, Typ> {
    fn from(h: &Headers<parser::CPTyp, CPTyp>) -> Self {
        let mut _header = Headers::default();
        for (k, v) in &h.0 {
           _header.0.insert(k.clone(), Signature::from(v)); 
        }
        _header
    }

}

impl From<&mut ReturnType<parser::CPTyp, CPTyp>> for ReturnType<parser::Typ, Typ> {
    fn from(r: &mut ReturnType<parser::CPTyp, CPTyp>) -> Self {
        ReturnType(
            match r.clone().0 {
                None => None,
                Some(CPTyp::DPTyp(t)) => Some(t.clone()),
                _ => unimplemented!("Not yet implemented!! For now OnboardingData and  result can not be a return type.")
        }, PhantomData)
    }

}

impl From<&Context<parser::CPTyp, CPTyp>> for Context<parser::Typ, Typ> {
    fn from(c : &Context<parser::CPTyp, CPTyp>) -> Self {
        let mut variables = HashMap::new();
        for (k, v) in &c.variables {
            variables.insert(k.clone(), Typ::from(v.clone()));
        }

        Context{variables: variables, async_tag: c.async_tag, phantom: PhantomData}
    }

}

impl From<ExprAndMeta<parser::Typ, Typ, Expr>> for ExprAndMeta<parser::CPTyp, CPTyp, CPExpr> {
    fn from(em: ExprAndMeta<parser::Typ, Typ, Expr>) -> Self {
        ExprAndMeta{
            expr: CPExpr::from(em.expr), 
            calls: em.calls,
            typ: CPTyp::from(em.typ),
            phantom: PhantomData
        } 
    }
}
//can not define convert since result external to the crate ....
fn fromres(res:Result<ExprAndMeta<parser::Typ, Typ, Expr>, self::Error>
    ) -> Result<ExprAndMeta<parser::CPTyp, CPTyp, CPExpr>, self::Error>{
    match res {
        Ok(em) => Ok(ExprAndMeta::from(em)),
        Err(e) => Err(e)
    }
}
impl TExpr<parser::CPTyp, parser::CPExpr, CPTyp> for CPExpr {
    fn closure_expr(self, v: &str) -> Self {
        if v == "_" {
            self
        } else {
            let tmp : CPExpr = self.abs(0, v);
            let e : Expr = match tmp { CPExpr::DPExpr(e) => e};
            CPExpr::DPExpr(Expr::Closure(parser::Ident::from(v), Box::new(e)))//FIXME: at some point we will have to allow CPEXPR (Expr(CPExpr ..)) => rectyp or macro ofr code gen
        }
    }


    fn abs(self, i: usize, v: &str) -> Self {
        match self {
            CPExpr::DPExpr(e) => CPExpr::DPExpr(Expr::abs(e, i, v))
        }
    }

    fn from_block_stmt(
        block: parser::BlockStmtRef<parser::CPExpr>,
        headers: &Headers<parser::CPTyp, CPTyp>,
        ret: &mut ReturnType<parser::CPTyp, CPTyp>,
        ctxt: &Context<parser::CPTyp, CPTyp>,
    ) -> Result<ExprAndMeta<parser::CPTyp, CPTyp, CPExpr>, self::Error> {

        fromres(Expr::from_block_stmt(
            parser::BlockStmtRef::from(block), 
            &Headers::from(headers),
            &mut ReturnType::from(ret),//FIXME avoid the clone + &
            &Context::from(ctxt)
            )
        )
    }

    fn from_loc_expr(
        e: &parser::LocExpr<parser::CPExpr>,
        headers: &Headers<parser::CPTyp, CPTyp>,
        ret: &mut ReturnType<parser::CPTyp, CPTyp>,
        ctxt: &Context<parser::CPTyp, CPTyp>,
    ) -> Result<ExprAndMeta<parser::CPTyp, CPTyp, CPExpr>, Error> {
        match e.expr() {
            parser::CPExpr::DPExpr(expr) =>
            fromres(Expr::from_loc_expr(
                &parser::LocExpr::new(&e.loc(), &expr), 
                &Headers::from(headers),
                &mut ReturnType::from(ret),
                &Context::from(ctxt)
                )
            )
        }
    }
    fn from_string(buf: &str, headers: &Headers<parser::CPTyp, CPTyp>) -> Result<Self, self::Error>{
        let lex = lexer::lex(buf);
        let toks = lexer::Tokens::new(&lex);
        // println!("{}", toks);
        panic!("TODO")
        //TODO
        //match parser::parse_block_stmt_eof(toks) {
        //    Ok((_rest, block)) => {
        //        // println!("{:#?}", block);
        //        Ok(
        //            Self::check_from_block_stmt(block.as_ref(), headers, &Context::new(), None)?
        //                .expr,
        //        )
        //    }
        //    Err(_) => match parser::parse_expr_eof(toks) {
        //        Ok((_rest, e)) => {
        //            // println!("{:#?}", e);
        //            Ok(Self::check_from_loc_expr(&e, headers, &Context::new())?.expr)
        //        }
        //        Err(nom::Err::Error((toks, _))) => {
        //            Err(Error::from(format!("syntax error: {}", toks.tok[0])))
        //        }
        //        Err(err) => Err(Error::from(format!("{:?}", err))),
        //    },
        //}
    }
}

impl CPExpr {

    pub fn from_string(buf: &str, headers: &Headers<parser::CPTyp, CPTyp>) -> Result<CPExpr, self::Error> {
        let lex = lexer::lex(buf);//TODO do we need to have two distinct lexer ?
        let toks = lexer::Tokens::new(&lex);
        // println!("{}", toks);
        unimplemented!()
        //match parser::parse_block_stmt_eof(toks) {
        //    Ok((_rest, block)) => {
        //        // println!("{:#?}", block);
        //        Ok(
        //            //Type check
        //            CPExpr::check_from_block_stmt(block.as_ref(), headers, &Context::new(), None)?
        //                .expr,
        //        )
        //    }
        //    Err(_) => match parser::parse_expr_eof(toks) {
        //        Ok((_rest, e)) => {
        //            // println!("{:#?}", e);
        //            Ok(CPExpr::check_from_loc_expr(&e, headers, &Context::new())?.expr)
        //        }
        //        Err(nom::Err::Error((toks, _))) => {
        //            Err(Error::from(format!("syntax error: {}", toks.tok[0])))
        //        }
        //        Err(err) => Err(Error::from(format!("{:?}", err))),
        //    },
        //}
    }
}

impl std::str::FromStr for CPExpr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let headers : Headers<parser::CPTyp, CPTyp>= Headers::default();
        Self::from_string(s, &headers)
    }
}