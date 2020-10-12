/// really basic type system
use serde::{Deserialize, Serialize};
use std::fmt;
use std::result::Result;
use super::types::{Error, DPTyp, Typ, FlatTyp, TFlatTyp, Signature, DPError};

#[derive(Clone,  Debug, PartialEq, Serialize, Deserialize)]
pub enum CPFlatTyp {
   DPFlatTyp(FlatTyp),
   OnboardingData,
   OnboardingResult,
}

impl CPTyp {
    pub fn onboardingData() -> Self {
        Self::FlatTyp(CPFlatTyp::OnboardingData)
    }
    pub fn onboardingResult() -> Self {
        Self::FlatTyp(CPFlatTyp::OnboardingResult)
    }
}

impl From<FlatTyp> for CPFlatTyp {
    fn from(ty: FlatTyp) -> CPFlatTyp{
        CPFlatTyp::DPFlatTyp(ty)
    }
}
impl From<Typ<FlatTyp>> for CPTyp {
    fn from(ty: Typ<FlatTyp>) -> CPTyp{
       match ty {
            Typ::FlatTyp(fty) => Typ::FlatTyp(CPFlatTyp::from(fty)),
            Typ::Tuple(tys) => Typ::Tuple(tys.into_iter().map(|ty| -> Typ<CPFlatTyp> { CPTyp::from(ty) }).collect()),
            Typ::List(bty) => Typ::List(Box::new(CPTyp::from(*bty))),

       } 
    }
}



pub type CPTyp = Typ<CPFlatTyp>;
pub type CPSignature = Signature<CPFlatTyp>;

impl fmt::Display for CPFlatTyp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CPFlatTyp::DPFlatTyp(t) => FlatTyp::fmt(t, f),
            CPFlatTyp::OnboardingData => write!(f, "onboardingData"),
            CPFlatTyp::OnboardingResult => write!(f, "onboardingResult"),
        }
    }
}

impl Default for CPFlatTyp {
    fn default() -> Self { Self::DPFlatTyp(FlatTyp::default()) }
}
impl TFlatTyp for CPFlatTyp {
    fn rreturn() -> Self { Self::DPFlatTyp(FlatTyp::Return) } 
    fn unit() -> Self { Self::DPFlatTyp(FlatTyp::Unit) } 
    fn bool() -> Self { Self::DPFlatTyp(FlatTyp::Bool) } 
    fn connection() -> Self { Self::DPFlatTyp(FlatTyp::Connection) } 
    fn f64() -> Self { Self::DPFlatTyp(FlatTyp::F64) } 
    fn http_request() -> Self { Self::DPFlatTyp(FlatTyp::HttpRequest) } 
    fn httpResponse() -> Self { Self::DPFlatTyp(FlatTyp::HttpResponse) } 
    fn label() -> Self { Self::DPFlatTyp(FlatTyp::Label) } 
    fn i64() -> Self { Self::DPFlatTyp(FlatTyp::I64) } 
    fn id() -> Self { Self::DPFlatTyp(FlatTyp::ID) } 
    fn ipAddr() -> Self { Self::DPFlatTyp(FlatTyp::IpAddr) } 
    fn data() -> Self { Self::DPFlatTyp(FlatTyp::Data) }
    fn str() -> Self { Self::DPFlatTyp(FlatTyp::Str) } 
    fn regex() -> Self { Self::DPFlatTyp(FlatTyp::Regex) } 

    fn try_from_str(s: &str) -> Result<Self, self::CPError > {
        match s {
            "OnboardingData" => Ok(Self::OnboardingData),
            "OnboardingResult" => Ok(Self::OnboardingResult),
            s => match FlatTyp::try_from_str(s)  {
                Ok(t) => Ok(Self::DPFlatTyp(t)),
                Err(e) =>  Err(Error::from(e))
            }
        }
    }

    //FIXME generalize a bit more or use macro
   // fn from_parse(ty: &parser::CPTyp) -> Result<Self, self::CPError > {
   //     match ty {
   //         parser::CPTyp::DPTyp(ty) => match ty{ 
   //             parser::Typ::Atom(a) => Self::try_from_str(a.id()),
   //             parser::Typ::Cons(c, b) => {
   //                 if c.id() == "List" {
   //                     Ok(CPTyp::from(Typ::List(Box::new(Typ::from_parse(b)?))))//FIXME should be generalized type and cptyp should have their own tuple
   //                 } else if c.id() == "Option" {
   //                     Ok(CPTyp::from(Typ::Tuple(vec![Typ::from_parse(b)?])))
   //                 } else {
   //                     Err(Error::Parse(format!("expecting \"List\", got {}", c.id())))
   //                 }
   //             }
   //             parser::Typ::Tuple(l) => match l.len() {
   //                 0 => Ok(CPTyp::from(Typ::Unit)),
   //                 1 => unimplemented!(),//Self::from_parse(&parser::CPTyp::from(l.get(0).unwrap())),
   //                 _ => {
   //                     let tys: Result<Vec<Typ>, self::DPError> =
   //                         l.iter().map(|x| Typ::from_parse(x)).collect();
   //                     Ok(CPTyp::from(Typ::Tuple(tys?)))
   //                 }
   //             },
   //         }
   //     }
   // }
}

type CPError = Error<CPFlatTyp>;

impl From<DPError> for CPError{
    fn from(error: DPError) -> Self{
        match error {
            Error::Mismatch(s, (ol1, t1), (ol2, t2)) =>{
                let t1=CPTyp::from(t1.clone());
                let t2=CPTyp::from(t2.clone());
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
///fromfn fromres(res:Result<CPTyp, DPError>) -> CPResult{
//    match res {
//        Ok(t) => Ok(CPTyp::DPTyp(t)),
//        Err(e) => Err(CPError::from(e))
//    }
//}



impl CPTyp {
    // ... as in types.rs




    //fn try_from_str(s: &str) -> Result<Self, CPError> {
    //    match s {
    //        "OnboardingData" => Ok(CPTyp::OnboardingData),
    //        "OnboardingResult" => Ok(CPTyp::OnboardingResult),
    //        s => fromres( Typ::try_from_str(s) ),
    //    }
    //}

    //fn from_parse(ty: &parser::CPTyp) -> Result<Self, CPError > {
    //    match ty {
    //        parser::CPTyp::DPTyp(a) => fromres(Typ::from_parse(a)),
    //        //FIXME parser::CPTyp::Atom(a) => CPTyp::try_from_str(a.id()),
    //    }
    //}
    
    // TODO do we need to duplicat this fct 
    //pub fn is_unit(&self) -> bool {
    //    CPTyp::type_check("", vec![(None, self)], vec![(None, &CPTyp::DPTyp(Typ::Unit))]).is_ok()
    //}

    //FIXME do we need this fct ??
    //pub fn dest_option(&self) -> Result<CPTyp, CPError> {
    //    match self {
    //        CPTyp::DPTyp(t) => fromres(Typ::dest_option(t)),
    //        _ => Err(CPError::Dest),
    //    }
    //}

    ////FIXME do we need this fct
    //pub fn dest_list(&self) -> Option<CPTyp> {
    //    match self {
    //        CPTyp::DPTyp(Typ::List(ty)) => Some(CPTyp::DPTyp(*ty.clone())),
    //        _ => None,
    //    }
    //}

    //TODO Prefix and Infix
}

//impl From<&DPTyp> for CPTyp {
//    fn from(t: &DPTyp) -> Self {
//        match t {
//            Typ::FlatTyp(t) => CPTyp::FlatTyp(CPFlatTyp::DPTyp(t.clone())),
//            Typ::List(t) => Typ::List(CPTyp::from(&t)),
//            Typ::Tuple(t) => Typ::Tuple( t.into_iter().map()CPTyp::from(&t)),
//        }
//    }
//}
