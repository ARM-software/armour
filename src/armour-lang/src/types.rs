/// really basic type system
use super::lexer::Loc;
use super::{parser};
use super::literals::TFlatLiteral;
use parser::{Infix, Prefix};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug,  PartialEq, Serialize, Deserialize)]
pub enum FlatTyp {
    Bool,
    Connection,
    Data,
    F64,
    HttpRequest,
    HttpResponse,
    I64,
    ID,
    IpAddr,
    Label,
    Regex,
    Return,
    Str,
    Unit
}



impl Default for FlatTyp {
    fn default() -> Self { Self::Unit }
}

impl fmt::Display for FlatTyp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FlatTyp::Bool => write!(f, "bool"),
            FlatTyp::Connection => write!(f, "Connection"),
            FlatTyp::Data => write!(f, "data"),
            FlatTyp::F64 => write!(f, "f64"),
            FlatTyp::HttpRequest => write!(f, "HttpRequest"),
            FlatTyp::HttpResponse => write!(f, "HttpResponse"),
            FlatTyp::I64 => write!(f, "i64"),
            FlatTyp::ID => write!(f, "ID"),
            FlatTyp::IpAddr => write!(f, "IpAddr"),
            FlatTyp::Label => write!(f, "Label"),
            FlatTyp::Regex => write!(f, "regex"),
            FlatTyp::Return => write!(f, "!"),
            FlatTyp::Str => write!(f, "str"),
            FlatTyp::Unit => write!(f, "unit")
        }
    }
}


#[derive(Clone, Debug,   PartialEq, Serialize, Deserialize)]
pub enum Typ<FlatTyp:TFlatTyp>{
    FlatTyp(FlatTyp),
    List(Box<Typ<FlatTyp>>),
    // tuples of length 0 and 1 are used to manage option types
    Tuple(Vec<Typ<FlatTyp>>),
}

impl<FlatTyp:TFlatTyp> fmt::Display for Typ<FlatTyp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Typ::FlatTyp(ft) => std::fmt::Display::fmt(&ft, f),
            Typ::List(t) => write!(f, "List<{}>", t.to_string()),
            Typ::Tuple(ts) => match ts.len() {
                0 => write!(f, "Option<?>"),
                1 => write!(f, "Option<{}>", ts.get(0).unwrap()),
                _ => write!(
                    f,
                    "({})",
                    ts.iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                ),
            },
        }
    }
}

pub type LocType<FlatTyp> = (Option<Loc>, Typ<FlatTyp>);//FIXME: i use value not ref because i can not create a fct &Typ -> &CPTyp due to lifetime (fct=fromres)
pub type LocTypes<FlatTyp> = Vec<LocType<FlatTyp>>;

//TBuiltin is a work around since specialization is unsable 
pub trait TBuiltin<FlatTyp:TFlatTyp>{
    fn builtins(_f: &str) -> Option<Signature<FlatTyp>> {None}
    fn internal_service(_f: &str) -> Option<Signature<FlatTyp>> {None}
}

pub trait TFlatTyp : fmt::Display + std::fmt::Debug + Sized + Serialize + Clone + PartialEq + TBuiltin<Self> + Unpin + std::marker::Send + Default + std::marker::Sync {
    //fn type_check(s: &str, v1: LocTypes<Self>, v2: LocTypes<Self>) -> Result<(), Error<Self>>; 
    
    fn rreturn() -> Self; 
    fn unit() -> Self;
    fn bool() -> Self;
    fn connection() -> Self;
    fn data() -> Self;
    fn f64() -> Self;
    fn label() -> Self;
    fn id() -> Self;
    fn i64() -> Self;
    fn ip_addr() -> Self;
    fn http_request() -> Self;
    fn http_response() -> Self;
    fn regex() -> Self;
    fn str() -> Self;

    fn try_from_str(s: &str) -> Result<Self, Error<Self> >; 
}

#[derive(Clone, Debug)]
pub enum Error<FlatTyp:TFlatTyp> {
    Mismatch(String, LocType<FlatTyp>, LocType<FlatTyp>),
    Args(String, usize, usize),
    Parse(String),
    Dest,
}
impl<'a, FlatTyp:TFlatTyp> fmt::Display for Error<FlatTyp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parse(s) => writeln!(f, "expecting type, got {}", s),
            Error::Args(s, i, j) => writeln!(
                f,
                "type error in \"{}\": expecting {} value(s), got {}",
                s, j, i
            ),
            Error::Mismatch(s, lt1, lt2) => {
                writeln!(f, "type error in \"{}\".\nmismatch:", s)?;
                write!(f, "> {}", lt1.1)?;
                match &lt1.0 {
                    Some(loc) => writeln!(f, " on {}", loc)?,
                    None => writeln!(f)?,
                }
                write!(f, "< {}", lt2.1)?;
                match &lt2.0 {
                    Some(loc) => writeln!(f, " on {}", loc),
                    None => writeln!(f),
                }
            }
            Error::Dest => write!(f, "expecting Option<..> type"),
        }
    }
}
    
pub type DPError = Error<FlatTyp>;

impl<FlatTyp:TFlatTyp> Typ<FlatTyp> {
    pub fn any_option() -> Self {
        Typ::Tuple(Vec::new())
    }
    pub fn option(&self) -> Self {
        Typ::Tuple(vec![self.clone()])
    }

    pub fn unify(&self, other: &Typ<FlatTyp>) -> Self {
        match (self, other) {
            (ret, x) | (x, ret) if *ret == Typ::rreturn() => x.clone(),
            (Typ::List(l1), Typ::List(l2)) => Typ::List(Box::new(l1.unify(l2))),
            (Typ::Tuple(l1), Typ::Tuple(l2)) => {
                let n1 = l1.len();
                let n2 = l2.len();
                if n1 == 0 {
                    other.clone()
                } else if n2 == 0 {
                    self.clone()
                } else {
                    Typ::Tuple(l1.iter().zip(l2).map(|(t1, t2)| t1.unify(t2)).collect())
                }
            }
            _ => self.clone(),
        }
    }

    pub fn is_unit(&self) -> bool {
        Typ::type_check("", vec![(None, self.clone())], vec![(None, Typ::unit())]).is_ok()
    }


    pub fn dest_option(&self) -> Result<Self, Error<FlatTyp>> {
        match self {
            Typ::Tuple(ts) => match ts.as_slice() {
                [] => Ok(Typ::rreturn()),
                [t] => Ok(t.clone()),
                _ => Err(Error::Dest),
            },
            _ => Err(Error::Dest),
        }
    }
    pub fn dest_list(&self) -> Option<Self> {
        match self {
            Typ::List(ty) => Some(*ty.clone()),
            _ => None,
        }
    }
    pub fn from_parse(ty: &parser::Typ) -> Result<Self, self::Error<FlatTyp> > {
        match ty {
            parser::Typ::Atom(a) => match FlatTyp::try_from_str(a.id()) {
                Ok(fl) => Ok(Typ::FlatTyp(fl)),
                Err(e) => Err(e)
            }
            parser::Typ::Cons(c, b) => {
                if c.id() == "List" {
                    Ok(Typ::List(Box::new(Typ::from_parse(b)?)))
                } else if c.id() == "Option" {
                    Ok(Typ::Tuple(vec![Typ::from_parse(b)?]))
                } else {
                    Err(Error::Parse(format!("expecting \"List\", got {}", c.id())))
                }
            }
            parser::Typ::Tuple(l) => match l.len() {
                0 => Ok(Typ::unit()),
                1 => Typ::from_parse(l.get(0).unwrap()),
                _ => {
                    let tys: Result<Vec<Self>, self::Error<FlatTyp>> =
                        l.iter().map(|x| Typ::from_parse(x)).collect();
                    Ok(Typ::Tuple(tys?))
                }
            },
        }
    }
    
    pub fn intrinsic(&self) -> Option<String> {
        match self {
            ret if *ret == Typ::rreturn() => None,
            Typ::List(_) => Some("list".to_string()),
            Typ::Tuple(t) => {
                if t.len() < 2 {
                    Some("option".to_string())
                } else {
                    None
                }
            }
            _ => Some(self.to_string()),
        }
    }
    fn can_unify(&self, other: &Self) -> bool {
        match (self, other) {
            (ret, _) | (_, ret) if *ret == Self::rreturn() => true,
            (Typ::List(l1), Typ::List(l2)) => l1.can_unify(l2),
            (Typ::Tuple(l1), Typ::Tuple(l2)) => {
                let n1 = l1.len();
                let n2 = l2.len();
                (n1 == n2 && l1.iter().zip(l2).all(|(t1, t2)| t1.can_unify(t2)))
                    || n1 == 0 && n2 == 1
                    || n1 == 1 && n2 == 0
            }
            _ => self == other,
        }
    }

    pub fn type_check(s: &str, v1: LocTypes<FlatTyp>, v2: LocTypes<FlatTyp>) -> Result<(), Error<FlatTyp>> {
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
}


impl TFlatTyp for FlatTyp { 
    fn rreturn() -> Self { Self::Return } 
    fn unit() -> Self { Self::Unit } 
    fn bool() -> Self { Self::Bool } 
    fn connection() -> Self { Self::Connection } 
    fn data() -> Self { Self::Data }
    fn f64() -> Self{ Self::F64 }
    fn http_request() -> Self { Self::HttpRequest } 
    fn http_response() -> Self { Self::HttpResponse } 
    fn label() -> Self { Self::Label } 
    fn i64() -> Self { Self::I64 } 
    fn id() -> Self { Self::ID } 
    fn ip_addr() -> Self { Self::IpAddr } 
    fn regex() -> Self { Self::Regex } 
    fn str() -> Self { Self::Str } 

    fn try_from_str(s: &str) -> Result<Self, DPError > {
        match s {
            "bool" => Ok(Self::Bool),
            "Connection" => Ok(Self::Connection),
            "data" => Ok(Self::Data),
            "f64" => Ok(Self::F64),
            "HttpRequest" => Ok(Self::HttpRequest),
            "HttpResponse" => Ok(Self::HttpResponse),
            "i64" => Ok(Self::I64),
            "ID" => Ok(Self::ID),
            "IpAddr" => Ok(Self::IpAddr),
            "Label" => Ok(Self::Label),
            "regex" => Ok(Self::Regex),
            "str" => Ok(Self::Str),
            "unit" => Ok(Self::Unit),
            s => Err(Error::Parse(s.to_string())),
        }
    }
}

//workaround to replace specialization
pub trait TTyp<FlatTyp:TFlatTyp> : Sized {
    fn rreturn() -> Self;
    fn unit() -> Self;
    fn bool() -> Self;
    fn connection() -> Self;
    fn f64() -> Self;
    fn data() -> Self;
    fn http_request() -> Self;
    fn http_response() -> Self;
    fn label() -> Self;
    fn i64() -> Self;
    fn id() -> Self;
    fn ip_addr() -> Self;
    fn regex() -> Self;
    fn str() -> Self;

    fn try_from_str(s: &str) -> Result<Self, Error<FlatTyp> >; 
}

//impl TFlatTyp<parser::Typ> for DPTyp { 
impl<FlatTyp:TFlatTyp> TTyp<FlatTyp> for Typ<FlatTyp> { 
    fn rreturn() -> Self { Self::FlatTyp(FlatTyp::rreturn()) } 
    fn unit() -> Self { Self::FlatTyp(FlatTyp::unit()) } 
    fn data() -> Self { Self::FlatTyp(FlatTyp::data()) }
    fn bool() -> Self { Self::FlatTyp(FlatTyp::bool()) } 
    fn connection() -> Self { Self::FlatTyp(FlatTyp::connection()) } 
    fn f64() -> Self { Self::FlatTyp(FlatTyp::f64()) }
    fn http_request() -> Self { Self::FlatTyp(FlatTyp::http_request()) } 
    fn http_response() -> Self { Self::FlatTyp(FlatTyp::http_response()) } 
    fn label() -> Self { Self::FlatTyp(FlatTyp::label()) } 
    fn i64() -> Self { Self::FlatTyp(FlatTyp::i64()) } 
    fn id() -> Self { Self::FlatTyp(FlatTyp::id()) } 
    fn ip_addr() -> Self { Self::FlatTyp(FlatTyp::ip_addr()) } 
    fn str() -> Self { Self::FlatTyp(FlatTyp::str()) } 
    fn regex() -> Self { Self::FlatTyp(FlatTyp::regex()) } 

    fn try_from_str(s: &str) -> Result<Self, Error<FlatTyp> > {
        match FlatTyp::try_from_str(s){
            Ok(x) => Ok(Typ::FlatTyp(x)),
            Err(e) => Err(e)
        }
    }
}

impl CPTyp {
    pub fn onboarding_data() -> Self {
        Self::FlatTyp(CPFlatTyp::OnboardingData)
    }
    pub fn onboarding_result() -> Self {
        Self::FlatTyp(CPFlatTyp::OnboardingResult)
    }
    pub fn policy() -> Self {
        Self::FlatTyp(CPFlatTyp::Policy)
    }
}

impl<FlatTyp:TFlatTyp> Prefix<FlatTyp> {
    pub fn typ(&self) -> (Typ<FlatTyp>, Typ<FlatTyp>) {
        let (t1, t2) = match self {
            Prefix::Not => (FlatTyp::bool(), FlatTyp::bool()),
            Prefix::Minus => (FlatTyp::i64(), FlatTyp::i64()),
            Prefix::Phantom(_) => unimplemented!()
        };
        (Typ::FlatTyp(t1), Typ::FlatTyp(t2))
    }
}

impl<FlatTyp:TFlatTyp> Infix<FlatTyp> {
    pub fn typ(&self) -> (Typ<FlatTyp>, Typ<FlatTyp>, Typ<FlatTyp>) {
        match self {
            Infix::Concat => (
                Typ::List(Box::new(Typ::rreturn())),
                Typ::List(Box::new(Typ::rreturn())),
                Typ::List(Box::new(Typ::rreturn())),
            ),
            Infix::Concat => unimplemented!(),
            Infix::ConcatStr => (Typ::str(), Typ::str(), Typ::str()),
            Infix::Equal | Infix::NotEqual => (Typ::rreturn(), Typ::rreturn(), Typ::bool()),
            Infix::In => (Typ::rreturn(), Typ::List(Box::new(Typ::rreturn())), Typ::bool()),
            Infix::And | Infix::Or => (Typ::bool(), Typ::bool(), Typ::bool()),
            Infix::Divide | Infix::Remainder | Infix::Minus | Infix::Plus | Infix::Multiply => {
                (Typ::i64(), Typ::i64(), Typ::i64())
            }
            Infix::GreaterThan
            | Infix::GreaterThanEqual
            | Infix::LessThan
            | Infix::LessThanEqual => (Typ::i64(), Typ::i64(), Typ::bool()),
            Infix::Module | Infix::Dot => panic!(),
            Infix::Phantom(_) => unimplemented!()
        }
    }
}

impl parser::Param {
    pub fn typ<FlatTyp:TFlatTyp>(&self) -> Result<Typ<FlatTyp>, Error<FlatTyp>> {
        Typ::from_parse(&self.typ)
    }
}

impl<FlatTyp:TFlatTyp> parser::Pattern<FlatTyp> {
    pub fn typ(&self) -> Typ<FlatTyp> {//Can not return & anymore, i think the borrowchecker is lost since fct call + generic
        match self {
            parser::Pattern::Regex(_) => Typ::str(),
            parser::Pattern::Label(_) => Typ::label(),
            parser::Pattern::Phantom(_) => unimplemented!()
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signature<FlatTyp: TFlatTyp >(Option<Vec<Typ<FlatTyp>>>, Typ<FlatTyp>);

impl<FlatTyp: TFlatTyp> Default for Signature<FlatTyp> {
    fn default() -> Self {
        Signature(None, Typ::FlatTyp(FlatTyp::unit()))
    }
}

impl<FlatTyp: TFlatTyp> Signature<FlatTyp> {
    pub fn new(args: Vec<Typ<FlatTyp>>, typ: Typ<FlatTyp>) -> Self {
        Signature(Some(args), typ)
    }
    pub fn new_noargs(typ:Typ<FlatTyp>) -> Self {
        Signature(None, typ)
    }
    pub fn any(typ: Typ<FlatTyp>) -> Self {
        Signature(None, typ)
    }
    pub fn split(self) -> (Option<Vec<Typ<FlatTyp>>>, Typ<FlatTyp>) {
        (self.0, self.1)
    }
    pub fn split_as_ref(&self) -> (Option<&Vec<Typ<FlatTyp>>>, &Typ<FlatTyp>) {
        (self.0.as_ref(), &self.1)
    }
    pub fn args(self) -> Option<Vec<Typ<FlatTyp>>> {
        self.0
    }
    pub fn typ(self) -> Typ<FlatTyp> {
        self.1
    }
}

impl<FlatTyp: TFlatTyp> fmt::Display for Signature<FlatTyp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        match self.0 {
            Some(ref tys) => write!(
                f,
                "{}",
                tys.iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )?,
            None => write!(f, "_")?,
        }
        write!(f, ") -> {}", self.1)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> parser::FnDecl<FlatTyp, FlatLiteral> {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature<FlatTyp>, Error<FlatTyp>> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::unit(),
        };
        let args: Result<Vec<Typ<FlatTyp>>, Error<FlatTyp>> = self.args().iter().map(|a| a.typ::<FlatTyp>()).collect();
        Ok(Signature::new(args?, ty))
    }
}

impl parser::Head {
    // TODO: report location of errors
    pub fn typ<FlatTyp:TFlatTyp>(&self) -> Result<Signature<FlatTyp>, Error<FlatTyp>> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::unit(),
        };
        if let Some(args) = self.args() {
        let args: Result<Vec<Typ<FlatTyp>>, Error<FlatTyp>> = args.iter().map(|a| Typ::from_parse(a)).collect();
            Ok(Signature::new(args?, ty))
        } else {
            Ok(Signature::any(ty))
        }
    }
}

pub type DPTyp = Typ<FlatTyp>;
pub type DPSignature = Signature<FlatTyp>;


#[derive(Clone,  Debug,  PartialEq, Serialize, Deserialize)]
pub enum CPFlatTyp {
   DPFlatTyp(FlatTyp),
   OnboardingData,
   OnboardingResult,
   Policy,
}
pub type CPTyp = Typ<CPFlatTyp>;
pub type CPSignature = Signature<CPFlatTyp>;

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
impl From<CPFlatTyp> for FlatTyp {
    fn from(ty: CPFlatTyp) -> Self {
        match ty {
            CPFlatTyp::DPFlatTyp(dty) => dty,
            CPFlatTyp::OnboardingData => unimplemented!(),
            CPFlatTyp::OnboardingResult => unimplemented!(),
            CPFlatTyp::Policy => unimplemented!(),
        }
    }
}
impl From<CPTyp> for DPTyp {
    fn from(ty: CPTyp) -> Self {
       match ty {
            Typ::FlatTyp(fty) => Typ::FlatTyp(FlatTyp::from(fty)),
            Typ::Tuple(tys) => Typ::Tuple(tys.into_iter().map(|ty| -> Typ<FlatTyp> { DPTyp::from(ty) }).collect()),
            Typ::List(bty) => Typ::List(Box::new(DPTyp::from(*bty))),
       } 
    }
}



impl From<CPSignature> for DPSignature {
    fn from(cpsig:CPSignature) -> Self {
        match cpsig.split() {
            (None, t) => DPSignature::new_noargs(DPTyp::from(t)),
            (Some(args), t) => DPSignature::new(
                args.into_iter().map(|t| DPTyp::from(t)).collect(),
                DPTyp::from(t)
            ),
        }

    }
}

impl fmt::Display for CPFlatTyp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CPFlatTyp::DPFlatTyp(t) => FlatTyp::fmt(t, f),
            CPFlatTyp::OnboardingData => write!(f, "OnboardingData"),
            CPFlatTyp::OnboardingResult => write!(f, "OnboardingResult"),
            CPFlatTyp::Policy => write!(f, "Policy"),
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
    fn http_response() -> Self { Self::DPFlatTyp(FlatTyp::HttpResponse) } 
    fn label() -> Self { Self::DPFlatTyp(FlatTyp::Label) } 
    fn i64() -> Self { Self::DPFlatTyp(FlatTyp::I64) } 
    fn id() -> Self { Self::DPFlatTyp(FlatTyp::ID) } 
    fn ip_addr() -> Self { Self::DPFlatTyp(FlatTyp::IpAddr) } 
    fn data() -> Self { Self::DPFlatTyp(FlatTyp::Data) }
    fn str() -> Self { Self::DPFlatTyp(FlatTyp::Str) } 
    fn regex() -> Self { Self::DPFlatTyp(FlatTyp::Regex) } 

    fn try_from_str(s: &str) -> Result<Self, self::CPError > {
        match s {
            "OnboardingData" => Ok(Self::OnboardingData),
            "OnboardingResult" => Ok(Self::OnboardingResult),
            "Policy" => Ok(Self::Policy),
            s => match FlatTyp::try_from_str(s)  {
                Ok(t) => Ok(Self::DPFlatTyp(t)),
                Err(e) =>  Err(Error::from(e))
            }
        }
    }
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