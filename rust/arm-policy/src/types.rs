/// really basic type system
use super::lexer::Loc;
use super::literals::Literal;
use super::parser;
use parser::{Infix, Prefix};
use std::fmt;

#[derive(Clone, PartialEq)]
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
    List(Box<Typ>),
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
            Typ::List(t) => write!(f, "List<{}>", t.to_string()),
            Typ::Tuple(ts) => write!(
                f,
                "({})",
                ts.iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
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
                    Some(loc) => writeln!(f, " on {}", loc)?,
                    None => writeln!(f, "")?,
                }
                Ok(())
            }
        }
    }
}

impl Typ {
    pub fn intrinsic(&self) -> Option<String> {
        match self {
            Typ::Return | Typ::Tuple(_) => None,
            Typ::List(_) => Some("list".to_string()),
            _ => Some(self.to_string()),
        }
    }
    fn unify(&self, other: &Typ) -> bool {
        match (self, other) {
            (Typ::Return, _) => true,
            (_, Typ::Return) => true,
            (Typ::List(l1), Typ::List(l2)) => l1.unify(l2),
            (Typ::Tuple(l1), Typ::Tuple(l2)) => {
                l1.len() == l2.len() && l1.iter().zip(l2).all(|(t1, t2)| t1.unify(t2))
            }
            _ => self == other,
        }
    }
    pub fn pick(&self, other: &Typ) -> Typ {
        if *self == Typ::Return {
            other.clone()
        } else {
            self.clone()
        }
    }
    pub fn type_check<'a>(s: &str, v1: LocTypes<'a>, v2: LocTypes<'a>) -> Result<(), Error<'a>> {
        let len1 = v1.len();
        let len2 = v2.len();
        if len1 == len2 {
            for (t1, t2) in v1.into_iter().zip(v2.into_iter()) {
                if !t1.1.unify(&t2.1) {
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
            s => Err(Error::Parse(s.to_string())),
        }
    }
    fn from_parse<'a>(ty: &'a parser::Typ) -> Result<Self, self::Error<'a>> {
        match ty {
            parser::Typ::Atom(a) => Typ::try_from_str(a.id()),
            parser::Typ::Cons(c, b) => {
                if c.id() == "List" {
                    Ok(Typ::List(Box::new(Typ::from_parse(b)?)))
                } else {
                    Err(Error::Parse(format!("expecting \"List\", got {}", c.id())))
                }
            }
            parser::Typ::Tuple(l) => {
                if l.len() == 0 {
                    Ok(Typ::Unit)
                } else {
                    let tys: Result<Vec<Self>, self::Error> =
                        l.iter().map(|x| Typ::from_parse(x)).collect();
                    Ok(Typ::Tuple(tys?))
                }
            }
        }
    }
    pub fn is_unit(&self) -> bool {
        Typ::type_check("", vec![(None, self)], vec![(None, &Typ::Unit)]).is_ok()
    }
}

impl Literal {
    pub fn typ(&self) -> Typ {
        match self {
            Literal::Unit => Typ::Unit,
            Literal::BoolLiteral(_) => Typ::Bool,
            Literal::IntLiteral(_) => Typ::I64,
            Literal::FloatLiteral(_) => Typ::F64,
            Literal::StringLiteral(_) => Typ::Str,
            Literal::DataLiteral(_) => Typ::Data,
            Literal::PolicyLiteral(_) => Typ::Policy,
            Literal::List(l) => l.get(0).map(|l| l.typ()).unwrap_or(Typ::Return),
            Literal::Tuple(_) => unimplemented!(),
            Literal::HttpRequestLiteral(_) => Typ::HttpRequest,
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
