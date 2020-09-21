/// really basic type system
use super::lexer::Loc;
use super::parser;
use parser::{Infix, Prefix};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::result::Result;
use super::types::{Error, Typ, TTyp, Signature, LocTypes};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CPTyp {
   DPTyp(Typ),
   OnboardingData,
   OnboardingResult,
}
impl fmt::Display for CPTyp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CPTyp::DPTyp(t) => Typ::fmt(t, f),
            CPTyp::OnboardingData => write!(f, "onboardingData"),
            CPTyp::OnboardingResult => write!(f, "onboardingResult"),
        }
    }
}
impl TTyp for CPTyp {}

impl Default for Signature<CPTyp> {
    fn default() -> Self {
        Signature::new_noargs(CPTyp::DPTyp(Typ::Unit))
    }
}

type CPError = Error<CPTyp>;
impl From<Error<Typ>> for Error<CPTyp>{
    fn from(error: Error<Typ>) -> Self{
        match error {
            Error::Mismatch(s, (ol1, t1), (ol2, t2)) =>{
                let t1=CPTyp::DPTyp(t1.clone());
                let t2=CPTyp::DPTyp(t2.clone());
                Error::Mismatch(s, (ol1, t1), (ol2, t2))//can not return &CPTyp think created in the fct
            },
            Error::Args(x, y, z) => Error::Args(x, y, z),
            Error::Parse(x) => Error::Parse(x),
            Error::Dest => Error::Dest,
        }

    }

}

//can not define convert since result external to the crate ....
type CPResult = Result<CPTyp, CPError>;
fn fromres(res:Result<Typ, Error<Typ>>) -> CPResult{
    match res {
        Ok(t) => Ok(CPTyp::DPTyp(t)),
        Err(e) => Err(CPError::from(e))
    }
}



impl CPTyp {
    // ... as in types.rs


    pub fn intrinsic(&self) -> Option<String> {
        match self {
            CPTyp::DPTyp(t) => Typ::intrinsic(t),
            _ => Some(self.to_string()),
        }
    }
    fn can_unify(&self, other: &CPTyp) -> bool {
        match (self, other) {
           (CPTyp::DPTyp(t1), CPTyp::DPTyp(t2)) => Typ::can_unify(t1, t2), 
            _ => self == other,
        }
    }
    pub fn unify(&self, other: &CPTyp) -> CPTyp {
        match (self, other) {
            (CPTyp::DPTyp(t1), CPTyp::DPTyp(t2)) => CPTyp::DPTyp(Typ::unify(t1, t2)),
            _ => self.clone(),
        }
    }

    //TODO type_check as a trait ?
    pub fn type_check(s: &str, v1: LocTypes<CPTyp>, v2: LocTypes<CPTyp>) -> Result<(), Error<CPTyp>> {
        let len1 = v1.len();
        let len2 = v2.len();
        if len1 == len2 {
            for (t1, t2) in v1.into_iter().zip(v2.into_iter()) {
                if !t1.1.can_unify(&t2.1) {
                    return Err(Error::Mismatch(s.to_string(), t1, t2));
                }
            }
            Ok(())
        } else {
            Err(Error::Args(s.to_string(), len1, len2))
        }
    }

    fn try_from_str(s: &str) -> Result<Self, CPError> {
        match s {
            "OnboardingData" => Ok(CPTyp::OnboardingData),
            "OnboardingResult" => Ok(CPTyp::OnboardingResult),
            s => fromres( Typ::try_from_str(s) ),
        }
    }

    fn from_parse(ty: &parser::CPTyp) -> Result<Self, CPError > {
        match ty {
            parser::CPTyp::DPTyp(a) => fromres(Typ::from_parse(a)),
            parser::CPTyp::Atom(a) => CPTyp::try_from_str(a.id()),
        }
    }
    
    // TODO do we need to duplicat this fct 
    //pub fn is_unit(&self) -> bool {
    //    CPTyp::type_check("", vec![(None, self)], vec![(None, &CPTyp::DPTyp(Typ::Unit))]).is_ok()
    //}

    //FIXME do we need this fct ??
    pub fn dest_option(&self) -> Result<CPTyp, CPError> {
        match self {
            CPTyp::DPTyp(t) => fromres(Typ::dest_option(t)),
            _ => Err(CPError::Dest),
        }
    }

    //FIXME do we need this fct
    pub fn dest_list(&self) -> Option<CPTyp> {
        match self {
            CPTyp::DPTyp(Typ::List(ty)) => Some(CPTyp::DPTyp(*ty.clone())),
            _ => None,
        }
    }

    //TODO Prefix and Infix
}