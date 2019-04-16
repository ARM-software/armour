/// parser
use crate::lexer::{Loc, Token, Tokens};
use nom::*;
use regex::Regex;
use std::collections::HashSet;
use std::fmt;

pub type Program = Vec<FnDecl>;

#[derive(PartialEq, Debug, Clone)]
pub struct Param {
    name: LocIdent,
    pub typ: LocIdent,
}

impl Param {
    pub fn name(&self) -> &str {
        self.name.id()
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct FnHead {
    id: LocIdent,
    params: Vec<Param>,
    typ: Option<LocIdent>,
}

impl FnHead {
    pub fn name(&self) -> &str {
        self.id.id()
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct FnDecl {
    head: FnHead,
    body: BlockStmt,
}

impl FnDecl {
    pub fn name(&self) -> &str {
        self.head.id.id()
    }
    pub fn args(&self) -> &Vec<Param> {
        &self.head.params
    }
    pub fn body(&self) -> &BlockStmt {
        &self.body
    }
    pub fn typ_id(&self) -> &Option<LocIdent> {
        &self.head.typ
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Stmt {
    LetStmt(LocIdent, LocExpr),
    ReturnStmt(LocExpr),
    ExprStmt(Expr, bool),
}

#[derive(PartialEq, Debug, Clone)]
pub struct LocStmt(Loc, Stmt);

impl LocStmt {
    fn let_stmt(l: Loc, i: LocIdent, e: LocExpr) -> LocStmt {
        LocStmt(l, Stmt::LetStmt(i, e))
    }
    fn return_stmt(l: Loc, e: LocExpr) -> LocStmt {
        LocStmt(l, Stmt::ReturnStmt(e))
    }
    fn expr_stmt(e: LocExpr, b: bool) -> LocStmt {
        LocStmt(e.loc(), Stmt::ExprStmt(e.1, b))
    }
    pub fn loc(&self) -> &Loc {
        &self.0
    }
    pub fn stmt(&self) -> &Stmt {
        &self.1
    }
}

pub type BlockStmt = Vec<LocStmt>;

#[derive(PartialEq, Debug, Clone)]
pub enum Expr {
    IdentExpr(Ident),
    LitExpr(Literal),
    PrefixExpr(Prefix, Box<LocExpr>),
    InfixExpr(Infix, Box<LocExpr>, Box<LocExpr>),
    IfExpr {
        cond: Box<LocExpr>,
        consequence: BlockStmt,
        alternative: Option<BlockStmt>,
    },
    IfMatchExpr {
        matches: Vec<(LocExpr, Pat)>,
        consequence: BlockStmt,
        alternative: Option<BlockStmt>,
    },
    CallExpr {
        loc: Loc,
        function: String,
        arguments: Vec<LocExpr>,
    },
    InExpr {
        val: Box<LocExpr>,
        vals: Vec<LocExpr>,
    },
}

impl Expr {
    fn eval_call_function(&self) -> Option<String> {
        match self {
            Expr::IdentExpr(id) => Some(id.0.to_string()),
            Expr::InfixExpr(Infix::Module, e1, e2) => {
                match (
                    e1.expr().eval_call_function(),
                    e2.expr().eval_call_function(),
                ) {
                    (Some(s1), Some(s2)) => Some(format!("{}::{}", s1, s2)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct LocExpr(Loc, Expr);

impl LocExpr {
    pub fn new(l: &Loc, e: &Expr) -> LocExpr {
        LocExpr(l.clone(), e.clone())
    }
    pub fn loc(&self) -> Loc {
        self.0.clone()
    }
    pub fn expr(&self) -> &Expr {
        &self.1
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Pat {
    Any,
    Lit(String),
    Class(String),
    Alt(Vec<Pat>),
    Seq(Vec<Pat>),
    As(Ident, bool), // true - match i64, false - match str
    Opt(Box<Pat>),
    Star(Box<Pat>),
    Plus(Box<Pat>),
    CaseInsensitive(Box<Pat>),
    IgnoreWhitespace(Box<Pat>),
}

impl Pat {
    fn to_regex_str(&self, ignore_ws: bool) -> String {
        match self {
            Pat::Any => ".".to_string(),
            Pat::Lit(s) => regex::escape(s),
            Pat::Class(s) => (match s.as_str() {
                "alpha" => "[[:alpha:]]",
                "alphanum" => "[[:alnum:]]",
                "digit" => "[[:digit:]]",
                "hex_digit" => "[[:xdigit:]]",
                "s" => "[[:space:]]",
                _ => unreachable!(),
            })
            .to_string(),
            Pat::Alt(vs) => format!(
                "({})",
                vs.iter()
                    .map(|v| v.to_regex_str(ignore_ws))
                    .collect::<Vec<String>>()
                    .join("|")
            ),
            Pat::Seq(vs) => format!(
                "({})",
                vs.iter()
                    .map(|v| v.to_regex_str(ignore_ws))
                    .collect::<Vec<String>>()
                    .join(if ignore_ws { "\\s*" } else { "" })
            ),
            Pat::As(id, false) => format!("(?P<{}>.+?)", id.0),
            Pat::As(id, true) => format!("(?P<_{}>-?[[:digit:]]+)", id.0),
            Pat::Opt(p) => format!("{}?", p.to_regex_str(ignore_ws)),
            Pat::Star(p) => format!("{}*", p.to_regex_str(ignore_ws)),
            Pat::Plus(p) => format!("{}+", p.to_regex_str(ignore_ws)),
            Pat::CaseInsensitive(p) => format!("(?i:{})", p.to_regex_str(ignore_ws)),
            Pat::IgnoreWhitespace(p) => p.to_regex_str(true),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolicyRegex(pub Regex);

impl PartialEq for PolicyRegex {
    fn eq(&self, other: &PolicyRegex) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl PolicyRegex {
    pub fn from_pat(p: &Pat) -> Result<PolicyRegex, regex::Error> {
        let re = Regex::new(&format!("^{}$", p.to_regex_str(false)))?;
        Ok(PolicyRegex(re))
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Literal {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    DataLiteral(String),
    StringLiteral(String),
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
            Literal::Unit => write!(f, "()"),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct LocLiteral(Loc, Literal);

impl LocLiteral {
    fn loc(&self) -> Loc {
        self.0.clone()
    }
}

#[derive(PartialEq, Debug, Eq, Clone)]
pub struct Ident(pub String);

#[derive(PartialEq, Debug, Clone)]
pub struct LocIdent(Loc, Ident);

impl LocIdent {
    fn loc(&self) -> Loc {
        self.0.clone()
    }
    pub fn id(&self) -> &str {
        &(self.1).0
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Prefix {
    PrefixMinus,
    Not,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Infix {
    Equal,
    NotEqual,
    Plus,
    Minus,
    Divide,
    Multiply,
    Remainder,
    GreaterThanEqual,
    LessThanEqual,
    GreaterThan,
    LessThan,
    And,
    Or,
    Concat,
    Module,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Assoc {
    Left,
    Right,
}

#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub enum Precedence {
    PLowest,
    POr,
    PAnd,
    PEquals,
    PLessGreater,
    PSum,
    PProduct,
    PIn,
    PCall,
    PModule,
}

macro_rules! tag_token (
  ($i: expr, $tag: expr) => (
    {
        use std::result::Result::*;
        use nom::{Err,ErrorKind};

        let (i1, t1) = try_parse!($i, take!(1));

        if t1.tok.is_empty() {
            Err(Err::Incomplete(Needed::Size(1)))
        } else {
            if *t1.tok0() == $tag {
                Ok((i1, t1))
            } else {
                Err(Err::Error(error_position!($i, ErrorKind::Count)))
            }
        }
    }
  );
);

macro_rules! parse_ident (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(name) => Ok((i1, (LocIdent(t1.loc(), Ident(name))))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

named!(pub parse_expr_eof<Tokens, LocExpr>,
    terminated!(parse_expr, tag_token!(Token::EOF))
);

named!(pub parse_block_stmt_eof<Tokens, BlockStmt>,
    terminated!(parse_block_stmt, tag_token!(Token::EOF))
);

named!(pub parse_program<Tokens, Program>,
    terminated!(many0!(parse_fn_expr), tag_token!(Token::EOF))
);

named!(pub parse_fn_head<Tokens, FnHead>,
    do_parse!(
        tag_token!(Token::Function) >>
        i: parse_ident!() >>
        tag_token!(Token::LParen) >>
        p: alt_complete!(parse_params | empty_params) >>
        tag_token!(Token::RParen) >>
        t: opt!(preceded!(tag_token!(Token::Arrow), parse_ident!())) >>
        (FnHead {id: i, params: p, typ: t})
    )
);

named!(parse_fn_expr<Tokens, FnDecl>,
    do_parse!(
        h: parse_fn_head >>
        b: parse_block_stmt >>
        (FnDecl {head: h, body: b })
    )
);

fn empty_params(i: Tokens) -> IResult<Tokens, Vec<Param>> {
    Ok((i, vec![]))
}

named!(parse_param<Tokens, Param>,
    do_parse!(
        p: parse_ident!() >>
        tag_token!(Token::Colon) >>
        t: parse_ident!() >>
        (Param {name: p, typ: t})
    )
);

named!(parse_params<Tokens, Vec<Param>>,
    do_parse!(
        p: parse_param >>
        ps: many0!(
                preceded!(
                    tag_token!(Token::Comma),
                    parse_param
                )
            ) >>
        ([&vec!(p)[..], &ps[..]].concat())
    )
);

macro_rules! parse_literal (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::IntLiteral(i) => Ok((i1, LocLiteral(t1.loc(), Literal::IntLiteral(i)))),
                Token::FloatLiteral(f) => Ok((i1, LocLiteral(t1.loc(), Literal::FloatLiteral(f)))),
                Token::BoolLiteral(b) => Ok((i1, LocLiteral(t1.loc(), Literal::BoolLiteral(b)))),
                Token::DataLiteral(d) => Ok((i1, LocLiteral(t1.loc(), Literal::DataLiteral(d)))),
                Token::StringLiteral(s) => Ok((i1, LocLiteral(t1.loc(), Literal::StringLiteral(s)))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

fn infix_op(t: &Token) -> (Precedence, Option<(Assoc, Infix)>) {
    match *t {
        Token::Equal => (Precedence::PEquals, Some((Assoc::Right, Infix::Equal))),
        Token::NotEqual => (Precedence::PEquals, Some((Assoc::Right, Infix::NotEqual))),
        Token::LessThanEqual => (
            Precedence::PLessGreater,
            Some((Assoc::Right, Infix::LessThanEqual)),
        ),
        Token::GreaterThanEqual => (
            Precedence::PLessGreater,
            Some((Assoc::Right, Infix::GreaterThanEqual)),
        ),
        Token::LessThan => (
            Precedence::PLessGreater,
            Some((Assoc::Right, Infix::LessThan)),
        ),
        Token::GreaterThan => (
            Precedence::PLessGreater,
            Some((Assoc::Right, Infix::GreaterThan)),
        ),
        Token::Or => (Precedence::POr, Some((Assoc::Right, Infix::Or))),
        Token::And => (Precedence::PAnd, Some((Assoc::Right, Infix::And))),
        Token::Plus => (Precedence::PSum, Some((Assoc::Left, Infix::Plus))),
        Token::Minus => (Precedence::PSum, Some((Assoc::Left, Infix::Minus))),
        Token::PlusPlus => (Precedence::PSum, Some((Assoc::Right, Infix::Concat))),
        Token::Multiply => (Precedence::PProduct, Some((Assoc::Left, Infix::Multiply))),
        Token::Divide => (Precedence::PProduct, Some((Assoc::Left, Infix::Divide))),
        Token::Percent => (Precedence::PProduct, Some((Assoc::Left, Infix::Remainder))),
        Token::In => (Precedence::PIn, None),
        Token::LParen => (Precedence::PCall, None),
        Token::ColonColon => (Precedence::PModule, Some((Assoc::Right, Infix::Module))),
        _ => (Precedence::PLowest, None),
    }
}

named!(parse_expr<Tokens, LocExpr>,
    apply!(parse_pratt_expr, Precedence::PLowest)
);

named!(parse_stmt<Tokens, LocStmt>, alt_complete!(
    parse_let_stmt |
    parse_return_stmt |
    parse_expr_stmt
));

named!(parse_let_stmt<Tokens, LocStmt>,
    do_parse!(
        t: tag_token!(Token::Let) >>
        ident: parse_ident!() >>
        tag_token!(Token::Assign) >>
        expr: parse_expr >>
        tag_token!(Token::SemiColon) >>
        (LocStmt::let_stmt(t.loc(), ident, expr))
    )
);

named!(parse_return_stmt<Tokens, LocStmt>,
    do_parse!(
        t: tag_token!(Token::Return) >>
        expr: parse_expr >>
        (LocStmt::return_stmt(t.loc(), expr))
    )
);

named!(parse_expr_stmt<Tokens, LocStmt>,
    do_parse!(
        expr: parse_expr >>
        semi: opt!(tag_token!(Token::SemiColon)) >>
        (LocStmt::expr_stmt(expr, semi.is_some()))
    )
);

named!(pub parse_block_stmt<Tokens, BlockStmt>,
    delimited!(tag_token!(Token::LBrace), many0!(parse_stmt), tag_token!(Token::RBrace))
);

named!(parse_atom_expr<Tokens, LocExpr>, alt_complete!(
    parse_lit_expr |
    parse_ident_expr |
    parse_prefix_expr |
    parse_unit_expr |
    parse_paren_expr |
    parse_if_expr |
    parse_if_match_expr
));

named!(parse_unit_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::LParen) >>
        tag_token!(Token::RParen) >>
        (LocExpr(t.loc(), Expr::LitExpr(Literal::Unit)))
    )
);

named!(parse_paren_expr<Tokens, LocExpr>,
    delimited!(tag_token!(Token::LParen), parse_expr, tag_token!(Token::RParen))
);

named!(parse_lit_expr<Tokens, LocExpr>,
    do_parse!(
        lit: parse_literal!() >>
        (LocExpr(lit.loc(), Expr::LitExpr(lit.1)))
    )
);

named!(parse_ident_expr<Tokens, LocExpr>,
    do_parse!(
        ident: parse_ident!() >>
        (LocExpr(ident.loc(), Expr::IdentExpr(ident.1)))
    )
);

named!(parse_comma_exprs<Tokens, LocExpr>,
    preceded!(tag_token!(Token::Comma), parse_expr)
);

named!(parse_exprs<Tokens, Vec<LocExpr>>,
    do_parse!(
        e: parse_expr >>
        es: many0!(parse_comma_exprs) >>
        ([&vec!(e)[..], &es[..]].concat())
    )
);

fn empty_boxed_vec(i: Tokens) -> IResult<Tokens, Vec<LocExpr>> {
    Ok((i, vec![]))
}

fn parse_prefix_expr(input: Tokens) -> IResult<Tokens, LocExpr> {
    let (i1, t1) = try_parse!(
        input,
        alt_complete!(tag_token!(Token::Minus) | tag_token!(Token::Not))
    );

    if t1.tok.is_empty() {
        Err(Err::Error(error_position!(input, ErrorKind::Tag)))
    } else {
        let (i2, e) = try_parse!(i1, parse_atom_expr);

        match t1.tok0().clone() {
            Token::Minus => Ok((
                i2,
                LocExpr(t1.loc(), Expr::PrefixExpr(Prefix::PrefixMinus, Box::new(e))),
            )),
            Token::Not => Ok((
                i2,
                LocExpr(t1.loc(), Expr::PrefixExpr(Prefix::Not, Box::new(e))),
            )),
            _ => Err(Err::Error(error_position!(input, ErrorKind::Tag))),
        }
    }
}

fn parse_pratt_expr(input: Tokens, precedence: Precedence) -> IResult<Tokens, LocExpr> {
    do_parse!(
        input,
        left: parse_atom_expr >> i: apply!(go_parse_pratt_expr, precedence, left) >> (i)
    )
}

fn go_parse_pratt_expr(
    input: Tokens,
    precedence: Precedence,
    left: LocExpr,
) -> IResult<Tokens, LocExpr> {
    let (i1, t1) = try_parse!(input, take!(1));
    if t1.tok.is_empty() {
        Ok((i1, left))
    } else {
        let preview = t1.tok0().clone();
        match infix_op(&preview) {
            (Precedence::PIn, _) if precedence < Precedence::PIn => {
                let (i2, left2) = try_parse!(input, apply!(parse_in_expr, left));
                go_parse_pratt_expr(i2, precedence, left2)
            }
            (Precedence::PCall, _) if precedence < Precedence::PCall => {
                let (i2, left2) = try_parse!(input, apply!(parse_call_expr, left));
                go_parse_pratt_expr(i2, precedence, left2)
            }
            (ref peek_precedence, Some((Assoc::Right, _))) if precedence <= *peek_precedence => {
                let (i2, left2) = try_parse!(input, apply!(parse_infix_expr, left));
                go_parse_pratt_expr(i2, precedence, left2)
            }
            (ref peek_precedence, _) if precedence < *peek_precedence => {
                let (i2, left2) = try_parse!(input, apply!(parse_infix_expr, left));
                go_parse_pratt_expr(i2, precedence, left2)
            }
            _ => Ok((input, left)),
        }
    }
}

fn parse_infix_expr(input: Tokens, left: LocExpr) -> IResult<Tokens, LocExpr> {
    let (i1, t1) = try_parse!(input, take!(1));
    if t1.tok.is_empty() {
        Err(Err::Error(error_position!(input, ErrorKind::Tag)))
    } else {
        let next = t1.tok0().clone();
        let (precedence, maybe_op) = infix_op(&next);
        match maybe_op {
            None => Err(Err::Error(error_position!(input, ErrorKind::Tag))),
            Some((_, op)) => {
                let (i2, right) = try_parse!(i1, apply!(parse_pratt_expr, precedence));
                Ok((
                    i2,
                    LocExpr(
                        t1.loc(),
                        Expr::InfixExpr(op, Box::new(left), Box::new(right)),
                    ),
                ))
            }
        }
    }
}

fn parse_call_expr(input: Tokens, fn_handle: LocExpr) -> IResult<Tokens, LocExpr> {
    do_parse!(
        input,
        tag_token!(Token::LParen)
            >> args: alt_complete!(parse_exprs | empty_boxed_vec)
            >> tag_token!(Token::RParen)
            >> (LocExpr(
                fn_handle.loc(),
                Expr::CallExpr {
                    loc: fn_handle.loc(),
                    function: match fn_handle.expr().eval_call_function() {
                        Some(s) => s,
                        None => return Err(nom::Err::Error(error_position!(input, ErrorKind::Tag))),
                    },
                    arguments: args
                }
            ))
    )
}

fn parse_in_expr(input: Tokens, expr_handle: LocExpr) -> IResult<Tokens, LocExpr> {
    do_parse!(
        input,
        tag_token!(Token::In)
            >> tag_token!(Token::LBracket)
            >> args: alt_complete!(parse_exprs | empty_boxed_vec)
            >> tag_token!(Token::RBracket)
            >> (LocExpr(
                expr_handle.loc(),
                Expr::InExpr {
                    val: Box::new(expr_handle),
                    vals: args,
                }
            ))
    )
}

named!(parse_if_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::If) >>
        expr: parse_expr >>
        consequence: parse_block_stmt >>
        alternative: opt!(parse_else_expr) >>
        (LocExpr(t.loc(), Expr::IfExpr { cond: Box::new(expr), consequence, alternative }))
    )
);

named!(parse_else_expr<Tokens, BlockStmt>,
    preceded!(tag_token!(Token::Else), parse_block_stmt)
);

named!(parse_if_match_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::If) >>
        tag_token!(Token::Match) >>
        matches: parse_match_exprs >>
        consequence: parse_block_stmt >>
        alternative: opt!(parse_else_expr) >>
        (LocExpr(t.loc(), Expr::IfMatchExpr { matches, consequence, alternative }))
    )
);

named!(parse_match_expr<Tokens, (LocExpr, Pat)>,
    do_parse!(
        e: parse_expr >>
        tag_token!(Token::With) >>
        pat: parse_pat >>
        ((e, pat))
    )
);

named!(parse_and_match_exprs<Tokens, (LocExpr, Pat)>,
    preceded!(tag_token!(Token::AndAlso), parse_match_expr)
);

named!(parse_match_exprs<Tokens, Vec<(LocExpr, Pat)>>,
    do_parse!(
        e: parse_match_expr >>
        es: many0!(parse_and_match_exprs) >>
        ([&vec!(e)[..], &es[..]].concat())
    )
);

macro_rules! parse_pat_literal (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::StringLiteral(s) => Ok((i1, Pat::Lit(s))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

lazy_static! {
    static ref CLASSES: HashSet<&'static str> =
        vec!["alpha", "alphanum", "digit", "hex_digit", "s"]
            .into_iter()
            .collect();
}

macro_rules! parse_pat_class (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(ref name) if CLASSES.contains(name.as_str()) => Ok((i1, Pat::Class(name.to_string()))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

macro_rules! parse_pat_typ (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(ref s) if s == "i64" => Ok((i1, true)),
                Token::Ident(ref s) if s == "str" => Ok((i1, false)),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

named!(parse_atom_pat<Tokens, Pat>, alt_complete!(
    parse_pat_literal!() |
    value!(Pat::Any, tag_token!(Token::Dot)) |
    delimited!(tag_token!(Token::Colon), parse_pat_class!(), tag_token!(Token::Colon)) |
    parse_as_pat |
    delimited!(tag_token!(Token::LParen), parse_pat, tag_token!(Token::RParen))
));

named!(parse_as_pat<Tokens, Pat>,
    do_parse!(
        tag_token!(Token::LBracket) >>
        i: parse_ident!() >>
        j: opt!(preceded!(tag_token!(Token::As), parse_pat_typ!())) >>
        tag_token!(Token::RBracket) >>
        (Pat::As(i.1, j.unwrap_or(false)))
    )
);

named!(parse_postfix_pat<Tokens, Pat>,
    do_parse!(
        a: parse_atom_pat >>
        postfix: many0!(alt_complete!(
            value!(Token::Multiply, tag_token!(Token::Multiply))
                | value!(Token::Plus, tag_token!(Token::Plus))
                | value!(Token::Optional, tag_token!(Token::Optional))
                | value!(Token::Not, tag_token!(Token::Not))
                | value!(Token::Percent, tag_token!(Token::Percent))
        )) >>
        ({
            let mut r = a;
            for p in postfix.iter() {
                match p {
                    Token::Multiply => r = Pat::Star(Box::new(r)),
                    Token::Plus => r = Pat::Plus(Box::new(r)),
                    Token::Optional => r = Pat::Opt(Box::new(r)),
                    Token::Not => r = Pat::CaseInsensitive(Box::new(r)),
                    Token::Percent => r = Pat::IgnoreWhitespace(Box::new(r)),
                    _ => unreachable!(),
                }
            };
            r
        })
    )
);

named!(parse_pat_seq<Tokens, Pat>,
    do_parse!(
        p: many1!(parse_postfix_pat) >>
        (if p.len() == 1 {p.clone().pop().unwrap()} else {Pat::Seq(p)})
    )
);

named!(parse_pat<Tokens, Pat>,
    do_parse!(
        p: parse_pat_seq >>
        ps: many0!(
                preceded!(
                    tag_token!(Token::Bar),
                    parse_pat_seq
                )
            ) >>
        (if ps.is_empty() {p} else {Pat::Alt([&vec!(p)[..], &ps[..]].concat())})
    )
);
