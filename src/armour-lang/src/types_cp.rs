/// really basic type system
use super::parser;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::result::Result;
use super::types::{Error, Typ, TTyp, Signature, LocTypes, DPError};

#[derive(Clone,  Debug, PartialEq, Serialize, Deserialize)]
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
impl TTyp<parser::CPTyp> for CPTyp {
    fn intrinsic(&self) -> Option<String> {
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
    fn try_from_str(s: &str) -> Result<Self, self::CPError > {
        match s {
            "OnboardingData" => Ok(CPTyp::OnboardingData),
            "OnboardingResult" => Ok(CPTyp::OnboardingResult),
            s => match Typ::try_from_str(s)  {
                //Ok(t) => {let tmp :  Result<Self, self::CPError > = Ok(CPTyp::DPTyp(t)); tmp},
                //Err(e) => {let tmp : self::CPError = Error::from(e); Err(tmp)}
                Ok(t) => Ok(CPTyp::DPTyp(t)),
                Err(e) =>  Err(Error::from(e))
            }
        }
    }

    //FIXME generalize a bit more or use macro
    fn from_parse(ty: &parser::CPTyp) -> Result<Self, self::CPError > {
        match ty {
            parser::CPTyp::DPTyp(ty) => match ty{ 
                parser::Typ::Atom(a) => Self::try_from_str(a.id()),
                parser::Typ::Cons(c, b) => {
                    if c.id() == "List" {
                        Ok(CPTyp::from(Typ::List(Box::new(Typ::from_parse(b)?))))//FIXME should be generalized type and cptyp should have their own tuple
                    } else if c.id() == "Option" {
                        Ok(CPTyp::from(Typ::Tuple(vec![Typ::from_parse(b)?])))
                    } else {
                        Err(Error::Parse(format!("expecting \"List\", got {}", c.id())))
                    }
                }
                parser::Typ::Tuple(l) => match l.len() {
                    0 => Ok(CPTyp::from(Typ::Unit)),
                    1 => unimplemented!(),//Self::from_parse(&parser::CPTyp::from(l.get(0).unwrap())),
                    _ => {
                        let tys: Result<Vec<Typ>, self::DPError> =
                            l.iter().map(|x| Typ::from_parse(x)).collect();
                        Ok(CPTyp::from(Typ::Tuple(tys?)))
                    }
                },
            }
        }
    }
}

impl Default for Signature<parser::CPTyp, CPTyp> {
    fn default() -> Self {
        Signature::new_noargs(CPTyp::DPTyp(Typ::Unit))
    }
}

type CPError = Error<parser::CPTyp, CPTyp>;

impl From<DPError> for CPError{
    fn from(error: DPError) -> Self{
        match error {
            Error::Mismatch(s, (ol1, t1), (ol2, t2)) =>{
                let t1=CPTyp::DPTyp(t1.clone());
                let t2=CPTyp::DPTyp(t2.clone());
                Error::Mismatch(s, (ol1, t1), (ol2, t2))//can not return &CPTyp think created in the fct
            },
            Error::Args(x, y, z) => Error::Args(x, y, z),
            Error::Parse(x) => Error::Parse(x),
            Error::Dest => Error::Dest,
            Error::Phantom(PhantomData) => panic!("Phantom data")
        }

    }

}

//can not define convert since result external to the crate ....
type CPResult = Result<CPTyp, CPError>;
fn fromres(res:Result<Typ, DPError>) -> CPResult{
    match res {
        Ok(t) => Ok(CPTyp::DPTyp(t)),
        Err(e) => Err(CPError::from(e))
    }
}



impl CPTyp {
    // ... as in types.rs


    pub fn unify(&self, other: &CPTyp) -> CPTyp {
        match (self, other) {
            (CPTyp::DPTyp(t1), CPTyp::DPTyp(t2)) => CPTyp::DPTyp(Typ::unify(t1, t2)),
            _ => self.clone(),
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
            //FIXME parser::CPTyp::Atom(a) => CPTyp::try_from_str(a.id()),
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

impl From<&Typ> for CPTyp {
    fn from(t: &Typ) -> Self {
        CPTyp::DPTyp(t.clone())
    }
}
impl From<Typ> for CPTyp {
    fn from(t: Typ) -> Self {
        CPTyp::DPTyp(t)
    }
}