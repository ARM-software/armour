/// really basic type system
use super::lexer::Loc;
use super::parser;
use parser::{Infix, Prefix};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum Typ {
    Return,
    Bool,
    I64,
    F64,
    Str,
    Data,
    Unit,
    Policy,
    HttpRequest,
    Ipv4Addr,
    List(Box<Typ>),
    // tuples of length 0 and 1 are used to manage option types
    Tuple(Vec<Typ>),
}

impl fmt::Display for Typ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Typ::Bool => write!(f, "bool"),
            Typ::I64 => write!(f, "i64"),
            Typ::F64 => write!(f, "f64"),
            Typ::Str => write!(f, "str"),
            Typ::Data => write!(f, "data"),
            Typ::Unit => write!(f, "unit"),
            Typ::Policy => write!(f, "Policy"),
            Typ::Return => write!(f, "!"),
            Typ::HttpRequest => write!(f, "HttpRequest"),
            Typ::Ipv4Addr => write!(f, "Ipv4Addr"),
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

type LocType<'a> = (Option<Loc>, &'a Typ);
type LocTypes<'a> = Vec<LocType<'a>>;

#[derive(Clone)]
pub enum Error<'a> {
    Mismatch(String, LocType<'a>, LocType<'a>),
    Args(String, usize, usize),
    Parse(String),
    Dest,
}

impl<'a> fmt::Display for Error<'a> {
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
                    None => writeln!(f, "")?,
                }
                write!(f, "< {}", lt2.1)?;
                match &lt2.0 {
                    Some(loc) => writeln!(f, " on {}", loc),
                    None => writeln!(f, ""),
                }
            }
            Error::Dest => write!(f, "expecting Option<..> type"),
        }
    }
}

impl Typ {
    pub fn any_option() -> Typ {Typ::Tuple(Vec::new())}
    pub fn option(&self) -> Typ {Typ::Tuple(vec![self.clone()])}
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
    fn can_unify(&self, other: &Typ) -> bool {
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
    pub fn type_check<'a>(s: &str, v1: LocTypes<'a>, v2: LocTypes<'a>) -> Result<(), Error<'a>> {
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
    fn try_from_str<'a>(s: &'a str) -> Result<Self, self::Error<'a>> {
        match s {
            "unit" => Ok(Typ::Unit),
            "bool" => Ok(Typ::Bool),
            "i64" => Ok(Typ::I64),
            "f64" => Ok(Typ::F64),
            "data" => Ok(Typ::Data),
            "str" => Ok(Typ::Str),
            "Policy" => Ok(Typ::Policy),
            "HttpRequest" => Ok(Typ::HttpRequest),
            "Ipv4Addr" => Ok(Typ::Ipv4Addr),
            s => Err(Error::Parse(s.to_string())),
        }
    }
    fn from_parse<'a>(ty: &'a parser::Typ) -> Result<Self, self::Error<'a>> {
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
                    let tys: Result<Vec<Self>, self::Error> =
                        l.iter().map(|x| Typ::from_parse(x)).collect();
                    Ok(Typ::Tuple(tys?))
                }
            },
        }
    }
    pub fn is_unit(&self) -> bool {
        Typ::type_check("", vec![(None, self)], vec![(None, &Typ::Unit)]).is_ok()
    }
    pub fn dest_option(&self) -> Result<Typ, Error> {
        match self {
            Typ::Tuple(ts) => match ts.as_slice() {
                [] => Ok(Typ::Return),
                [t] => Ok(t.clone()),
                _ => Err(Error::Dest),
            },
            _ => Err(Error::Dest),
        }
    }
}

impl Prefix {
    pub fn typ(&self) -> (Typ, Typ) {
        match self {
            Prefix::Not => (Typ::Bool, Typ::Bool),
            Prefix::PrefixMinus => (Typ::I64, Typ::I64),
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
    pub fn typ(&self) -> Result<Typ, Error> {
        Typ::from_parse(&self.typ)
    }
}

pub type Signature = (Vec<Typ>, Typ);

impl parser::FnDecl {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature, Error> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::Unit,
        };
        let args: Result<Vec<Typ>, Error> = self.args().iter().map(|a| a.typ()).collect();
        Ok((args?, ty))
    }
}

impl parser::Head {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature, Error> {
        let ty = match self.typ_id() {
            Some(id) => Typ::from_parse(id)?,
            None => Typ::Unit,
        };
        let args: Result<Vec<Typ>, Error> =
            self.args().iter().map(|a| Typ::from_parse(a)).collect();
        Ok((args?, ty))
    }
}
