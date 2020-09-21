/// really basic type system
use super::lexer::Loc;
use super::parser;
use parser::{Infix, Prefix};
use serde::{Deserialize, Serialize};
use std::fmt;

pub trait TTyp : fmt::Display {
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Typ {
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
    Unit,
    List(Box<Typ>),
    // tuples of length 0 and 1 are used to manage option types
    Tuple(Vec<Typ>),
}

impl fmt::Display for Typ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Typ::Bool => write!(f, "bool"),
            Typ::Connection => write!(f, "Connection"),
            Typ::Data => write!(f, "data"),
            Typ::F64 => write!(f, "f64"),
            Typ::HttpRequest => write!(f, "HttpRequest"),
            Typ::HttpResponse => write!(f, "HttpResponse"),
            Typ::I64 => write!(f, "i64"),
            Typ::ID => write!(f, "ID"),
            Typ::IpAddr => write!(f, "IpAddr"),
            Typ::Label => write!(f, "Label"),
            Typ::Regex => write!(f, "regex"),
            Typ::Return => write!(f, "!"),
            Typ::Str => write!(f, "str"),
            Typ::Unit => write!(f, "unit"),
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

impl TTyp for Typ{}

type LocType<Typ> = (Option<Loc>, Typ);//FIXME: i use value not ref because i can not create a fct &Typ -> &CPTyp due to lifetime (fct=fromres)
pub type LocTypes<Typ> = Vec<LocType<Typ>>;

#[derive(Clone, Debug)]
pub enum Error<Typ:(fmt::Display)> {
    Mismatch(String, LocType<Typ>, LocType<Typ>),
    Args(String, usize, usize),
    Parse(String),
    Dest,
}

impl<'a, Typ:TTyp> fmt::Display for Error<Typ> {
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

impl Typ {
    pub fn any_option() -> Typ {
        Typ::Tuple(Vec::new())
    }
    pub fn option(&self) -> Typ {
        Typ::Tuple(vec![self.clone()])
    }
    pub fn intrinsic(&self) -> Option<String> {
        match self {
            Typ::Return => None,
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
    pub fn can_unify(&self, other: &Typ) -> bool {
        match (self, other) {
            (Typ::Return, _) | (_, Typ::Return) => true,
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
    pub fn unify(&self, other: &Typ) -> Typ {
        match (self, other) {
            (Typ::Return, x) | (x, Typ::Return) => x.clone(),
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
    pub fn type_check(s: &str, v1: LocTypes<Typ>, v2: LocTypes<Typ>) -> Result<(), Error<Typ>> {
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
    pub fn try_from_str(s: &str) -> Result<Self, self::Error<Typ> > {
        match s {
            "bool" => Ok(Typ::Bool),
            "Connection" => Ok(Typ::Connection),
            "data" => Ok(Typ::Data),
            "f64" => Ok(Typ::F64),
            "HttpRequest" => Ok(Typ::HttpRequest),
            "HttpResponse" => Ok(Typ::HttpResponse),
            "i64" => Ok(Typ::I64),
            "ID" => Ok(Typ::ID),
            "IpAddr" => Ok(Typ::IpAddr),
            "Label" => Ok(Typ::Label),
            "regex" => Ok(Typ::Regex),
            "str" => Ok(Typ::Str),
            "unit" => Ok(Typ::Unit),
            s => Err(Error::Parse(s.to_string())),
        }
    }
    pub fn from_parse(ty: &parser::Typ) -> Result<Self, self::Error<Typ> > {
        match ty {
            parser::Typ::Atom(a) => Typ::try_from_str(a.id()),
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
                0 => Ok(Typ::Unit),
                1 => Typ::from_parse(l.get(0).unwrap()),
                _ => {
                    let tys: Result<Vec<Self>, self::Error<Typ>> =
                        l.iter().map(|x| Typ::from_parse(x)).collect();
                    Ok(Typ::Tuple(tys?))
                }
            },
        }
    }
    pub fn is_unit(&self) -> bool {
        Typ::type_check("", vec![(None, self.clone())], vec![(None, Typ::Unit)]).is_ok()
    }
    pub fn dest_option(&self) -> Result<Typ, Error<Typ>> {
        match self {
            Typ::Tuple(ts) => match ts.as_slice() {
                [] => Ok(Typ::Return),
                [t] => Ok(t.clone()),
                _ => Err(Error::Dest),
            },
            _ => Err(Error::Dest),
        }
    }
    pub fn dest_list(&self) -> Option<Typ> {
        match self {
            Typ::List(ty) => Some(*ty.clone()),
            _ => None,
        }
    }
}

impl Prefix {
    pub fn typ(&self) -> (Typ, Typ) {
        match self {
            Prefix::Not => (Typ::Bool, Typ::Bool),
            Prefix::Minus => (Typ::I64, Typ::I64),
        }
    }
}

impl Infix {
    pub fn typ(&self) -> (Typ, Typ, Typ) {
        match self {
            Infix::Concat => (
                Typ::List(Box::new(Typ::Return)),
                Typ::List(Box::new(Typ::Return)),
                Typ::List(Box::new(Typ::Return)),
            ),
            Infix::ConcatStr => (Typ::Str, Typ::Str, Typ::Str),
            Infix::Equal | Infix::NotEqual => (Typ::Return, Typ::Return, Typ::Bool),
            Infix::In => (Typ::Return, Typ::List(Box::new(Typ::Return)), Typ::Bool),
            Infix::And | Infix::Or => (Typ::Bool, Typ::Bool, Typ::Bool),
            Infix::Divide | Infix::Remainder | Infix::Minus | Infix::Plus | Infix::Multiply => {
                (Typ::I64, Typ::I64, Typ::I64)
            }
            Infix::GreaterThan
            | Infix::GreaterThanEqual
            | Infix::LessThan
            | Infix::LessThanEqual => (Typ::I64, Typ::I64, Typ::Bool),
            Infix::Module | Infix::Dot => panic!(),
        }
    }
}

impl parser::Param {
    pub fn typ(&self) -> Result<Typ, Error<Typ>> {
        Typ::from_parse(&self.typ)
    }
}

impl parser::Pattern {
    pub fn typ(&self) -> &Typ {
        match self {
            parser::Pattern::Regex(_) => &Typ::Str,
            parser::Pattern::Label(_) => &Typ::Label,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Signature<Typ:TTyp>(Option<Vec<Typ>>, Typ);

impl Default for Signature<Typ> {
    fn default() -> Self {
        Signature(None, Typ::Unit)
    }
}

impl<Typ:TTyp> Signature<Typ> {
    pub fn new(args: Vec<Typ>, typ: Typ) -> Self {
        Signature(Some(args), typ)
    }
    pub fn new_noargs(typ:Typ) -> Self {
        Signature(None, typ)
    }
    pub fn any(typ: Typ) -> Self {
        Signature(None, typ)
    }
    pub fn split(self) -> (Option<Vec<Typ>>, Typ) {
        (self.0, self.1)
    }
    pub fn split_as_ref(&self) -> (Option<&Vec<Typ>>, &Typ) {
        (self.0.as_ref(), &self.1)
    }
    pub fn args(self) -> Option<Vec<Typ>> {
        self.0
    }
    pub fn typ(self) -> Typ {
        self.1
    }
}

impl<Typ:TTyp> fmt::Display for Signature<Typ> {
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

impl parser::FnDecl {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature<Typ>, Error<Typ>> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::Unit,
        };
        let args: Result<Vec<Typ>, Error<Typ>> = self.args().iter().map(|a| a.typ()).collect();
        Ok(Signature::new(args?, ty))
    }
}

impl parser::Head {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature<Typ>, Error<Typ>> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::Unit,
        };
        if let Some(args) = self.args() {
            let args: Result<Vec<Typ>, Error<Typ>> = args.iter().map(|a| Typ::from_parse(a)).collect();
            Ok(Signature::new(args?, ty))
        } else {
            Ok(Signature::any(ty))
        }
    }
}
