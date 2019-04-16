/// really basic type system
use super::lexer::Loc;
use super::parser;
use super::parser::{Infix, Literal, Prefix};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Typ {
    Bool,
    I64,
    F64,
    Str,
    Data,
    Unit,
    Policy,
    Return,
}

type LocType<'a> = (Option<Loc>, &'a Typ);
type LocTypes<'a> = Vec<LocType<'a>>;

#[derive(Debug, Clone)]
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
                write!(f, "> {:?}", lt1.1)?;
                match &lt1.0 {
                    Some(loc) => writeln!(f, " on {}", loc)?,
                    None => writeln!(f, "")?,
                }
                write!(f, "< {:?}", lt2.1)?;
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
    fn unify(&self, other: &Typ) -> bool {
        *self == Typ::Return || *other == Typ::Return || self == other
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
            "policy" => Ok(Typ::Policy),
            s => Err(Error::Parse(s.to_string())),
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
            Infix::Concat => (Typ::Str, Typ::Str, Typ::Str),
            Infix::Equal | Infix::NotEqual => (Typ::Return, Typ::Return, Typ::Bool),
            Infix::And | Infix::Or => (Typ::Bool, Typ::Bool, Typ::Bool),
            Infix::Divide | Infix::Remainder | Infix::Minus | Infix::Plus | Infix::Multiply => {
                (Typ::I64, Typ::I64, Typ::I64)
            }
            Infix::GreaterThan
            | Infix::GreaterThanEqual
            | Infix::LessThan
            | Infix::LessThanEqual => (Typ::I64, Typ::I64, Typ::Bool),
            Infix::Module => panic!(),
        }
    }
}

impl parser::Param {
    pub fn typ(&self) -> Result<Typ, Error> {
        Typ::try_from_str(self.typ.id())
    }
}

pub type Signature = (Vec<Typ>, Typ);

impl parser::FnDecl {
    // TODO: report location of errors
    pub fn typ(&self) -> Result<Signature, Error> {
        let ty = match self.typ_id() {
            Some(id) => Typ::try_from_str(id.id())?,
            None => Typ::Unit,
        };
        let args: Result<Vec<Typ>, Error> = self.args().iter().map(|a| a.typ()).collect();
        Ok((args?, ty))
    }
}
