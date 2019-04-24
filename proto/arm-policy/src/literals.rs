use std::fmt;

#[derive(PartialEq, Debug, Clone)]
pub enum Policy {
    Accept,
    Forward,
    Reject,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Literal {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    DataLiteral(String),
    StringLiteral(String),
    PolicyLiteral(Policy),
    Unit,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::IntLiteral(i) => write!(f, "{:2}", i),
            Literal::FloatLiteral(d) => write!(f, "{}", d),
            Literal::BoolLiteral(b) => write!(f, "{}", b),
            Literal::DataLiteral(d) => write!(f, r#"b"{}""#, d),
            Literal::StringLiteral(s) => write!(f, r#""{}""#, s),
            Literal::PolicyLiteral(p) => write!(f, "{:?}", p),
            Literal::Unit => write!(f, "()"),
        }
    }
}
