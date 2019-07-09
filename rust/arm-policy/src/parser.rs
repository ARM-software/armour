/// parser
use super::lexer::{Loc, Token, Tokens};
use super::literals::Literal;
use nom::*;
use regex::Regex;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub type Program = Vec<Decl>;

pub enum Decl {
    External(External),
    FnDecl(FnDecl),
}

pub struct External {
    name: LocIdent,
    url: LocIdent,
    pub headers: Vec<Head>,
}

impl External {
    pub fn name(&self) -> &str {
        self.name.id()
    }
    pub fn url(&self) -> &str {
        self.url.id()
    }
}

#[derive(Debug, Clone)]
pub enum Typ {
    Atom(LocIdent),
    Cons(LocIdent, Box<Typ>),
    Tuple(Vec<Typ>),
}

pub struct Head {
    id: LocIdent,
    typs: Option<Vec<Typ>>,
    typ: Option<Typ>,
}

impl Head {
    pub fn name(&self) -> &str {
        self.id.id()
    }
    pub fn args(&self) -> Option<&Vec<Typ>> {
        self.typs.as_ref()
    }
    pub fn typ_id(&self) -> Option<&Typ> {
        self.typ.as_ref()
    }
    pub fn loc(&self) -> Loc {
        self.id.loc()
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    name: LocIdent,
    pub typ: Typ,
}

impl Param {
    pub fn name(&self) -> &str {
        self.name.id()
    }
}

#[derive(Debug, Clone)]
pub struct FnHead {
    id: LocIdent,
    params: Vec<Param>,
    typ: Option<Typ>,
}

impl FnHead {
    pub fn name(&self) -> &str {
        self.id.id()
    }
}

#[derive(Debug, Clone)]
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
    pub fn typ_id(&self) -> &Option<Typ> {
        &self.head.typ
    }
    pub fn loc(&self) -> Loc {
        self.head.id.loc()
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    LetStmt(Vec<LocIdent>, LocExpr),
    ReturnStmt(LocExpr),
    ExprStmt(Expr, bool),
}

#[derive(Debug, Clone)]
pub struct LocStmt(Loc, Stmt);

impl LocStmt {
    fn let_stmt(l: Loc, i: Vec<LocIdent>, e: LocExpr) -> LocStmt {
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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Iter {
    All,
    Any,
    Filter,
    FilterMap,
    ForEach,
    Map,
}

impl std::fmt::Display for Iter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Iter::All => write!(f, "all"),
            Iter::Any => write!(f, "any"),
            Iter::Filter => write!(f, "filter"),
            Iter::FilterMap => write!(f, "filter_map"),
            Iter::Map => write!(f, "map"),
            Iter::ForEach => write!(f, "foreach"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    IdentExpr(Ident),
    LitExpr(Literal),
    ListExpr(Vec<LocExpr>),
    TupleExpr(Vec<LocExpr>),
    PrefixExpr(Prefix, Box<LocExpr>),
    InfixExpr(Infix, Box<LocExpr>, Box<LocExpr>),
    IterExpr {
        op: Iter,
        idents: Vec<LocIdent>,
        expr: Box<LocExpr>,
        body: BlockStmt,
    },
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
    IfSomeMatchExpr {
        var: LocIdent,
        expr: Box<LocExpr>,
        consequence: BlockStmt,
        alternative: Option<BlockStmt>,
    },
    CallExpr {
        loc: Loc,
        function: String,
        arguments: Vec<LocExpr>,
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum As {
    Str,
    I64,
    Base64,
}

#[derive(Debug, Clone)]
pub enum Pat {
    Any,
    Lit(String),
    Class(String),
    Alt(Vec<Pat>),
    Seq(Vec<Pat>),
    As(Ident, As),
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
            Pat::As(id, As::Str) => format!("(?P<{}>.+?)", id.0),
            Pat::As(id, As::I64) => format!("(?P<_i64{}>-?[[:digit:]]+)", id.0),
            Pat::As(id, As::Base64) => format!(
                "(?P<_b64{}>([A-Za-z0-9+/]{{4}})*([A-Za-z0-9+/]{{2}}==|[A-Za-z0-9+/]{{3}}=)?)",
                id.0
            ),
            Pat::Opt(p) => format!("{}?", p.to_regex_str(ignore_ws)),
            Pat::Star(p) => format!("{}*?", p.to_regex_str(ignore_ws)),
            Pat::Plus(p) => format!("{}+?", p.to_regex_str(ignore_ws)),
            Pat::CaseInsensitive(p) => format!("(?i:{})", p.to_regex_str(ignore_ws)),
            Pat::IgnoreWhitespace(p) => p.to_regex_str(true),
        }
    }
    pub fn strip_as(s: &str) -> (String, As) {
        if s.starts_with("_i64") {
            (s.trim_start_matches("_i64").to_string(), As::I64)
        } else if s.starts_with("_b64") {
            (s.trim_start_matches("_b64").to_string(), As::Base64)
        } else {
            (s.to_string(), As::Str)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRegex(#[serde(with = "serde_regex")] pub Regex);

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

enum LocExprOrMatches {
    Expr(LocExpr),
    Matches(Vec<(LocExpr, Pat)>),
    SomeMatch(LocIdent, LocExpr),
}

#[derive(Debug, Clone)]
pub struct LocLiteral(Loc, Literal);

impl LocLiteral {
    fn loc(&self) -> Loc {
        self.0.clone()
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Ident(pub String);

#[derive(Debug, Clone)]
pub struct LocIdent(Loc, Ident);

impl LocIdent {
    fn loc(&self) -> Loc {
        self.0.clone()
    }
    pub fn id(&self) -> &str {
        &(self.1).0
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Prefix {
    PrefixMinus,
    Not,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
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
    ConcatStr,
    Module,
    In,
    Dot,
}

pub enum Assoc {
    Left,
    Right,
}

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    PLowest,
    POr,
    PAnd,
    PEquals,
    PLessGreater,
    PIn,
    PSum,
    PProduct,
    PDot,
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
    terminated!(
        many0!(
            alt_complete!(
                do_parse!(f: parse_fn_expr >> (Decl::FnDecl(f))) |
                do_parse!(e: parse_external >> (Decl::External(e)))
            )
        ),
        tag_token!(Token::EOF)
    )
);

named!(pub parse_fn_head<Tokens, FnHead>,
    do_parse!(
        tag_token!(Token::Function) >>
        id: parse_ident!() >>
        tag_token!(Token::LParen) >>
        params: opt!(parse_params) >>
        tag_token!(Token::RParen) >>
        typ: opt!(preceded!(tag_token!(Token::Arrow), parse_type)) >>
        (FnHead {id, params: params.unwrap_or(Vec::new()), typ})
    )
);

named!(parse_fn_expr<Tokens, FnDecl>,
    do_parse!(
        head: parse_fn_head >>
        body: parse_block_stmt >>
        (FnDecl {head, body })
    )
);

named!(parse_param<Tokens, Param>,
    do_parse!(
        name: parse_ident!() >>
        tag_token!(Token::Colon) >>
        typ: parse_type >>
        (Param {name, typ})
    )
);

named!(parse_comma_param<Tokens, Param>,
    preceded!(tag_token!(Token::Comma), parse_param)
);

named!(parse_params<Tokens, Vec<Param>>,
    do_parse!(
        p: parse_param >>
        ps: many0!(parse_comma_param) >>
        ([&vec!(p)[..], &ps[..]].concat())
    )
);

named!(tuple_ident<Tokens, LocIdent>, alt_complete!(
    do_parse!(u: tag_token!(Token::Underscore) >> (LocIdent(u.loc(), Ident("_".to_string())))) |
    parse_ident!()
));

named!(parse_comma_tuple_ident<Tokens, LocIdent>,
    preceded!(tag_token!(Token::Comma), tuple_ident)
);

named!(parse_idents<Tokens, Vec<LocIdent>>, alt_complete!(
    do_parse!(
        tag_token!(Token::LParen) >>
        id: tuple_ident >>
        ids: many1!(parse_comma_tuple_ident) >>
        tag_token!(Token::RParen) >>
        ([&vec!(id)[..], &ids[..]].concat())
    ) |
    do_parse!(id: parse_ident!() >> (vec![id]))
    )
);

macro_rules! parse_string_as_ident (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::StringLiteral(s) => Ok((i1, LocIdent(t1.loc(), Ident(s)))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

macro_rules! parse_int_literal (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::IntLiteral(i) => Ok((i1, (t1.loc(), i))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
);

macro_rules! parse_literal (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::IntLiteral(i) => Ok((i1, LocLiteral(t1.loc(), Literal::Int(i)))),
                Token::FloatLiteral(f) => Ok((i1, LocLiteral(t1.loc(), Literal::Float(f)))),
                Token::BoolLiteral(b) => Ok((i1, LocLiteral(t1.loc(), Literal::Bool(b)))),
                Token::DataLiteral(d) => Ok((i1, LocLiteral(t1.loc(), Literal::Data(d)))),
                Token::StringLiteral(s) => Ok((i1, LocLiteral(t1.loc(), Literal::Str(s)))),
                Token::PolicyLiteral(p) => Ok((i1, LocLiteral(t1.loc(), Literal::Policy(p)))),
                Token::Ident(ref s) if s == "None" => Ok((i1, LocLiteral(t1.loc(), Literal::none()))),
                _ => Err(nom::Err::Error(error_position!($i, nom::ErrorKind::Tag))),
            }
        }
    }
  );
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
        Token::At => (Precedence::PSum, Some((Assoc::Right, Infix::Concat))),
        Token::PlusPlus => (Precedence::PSum, Some((Assoc::Right, Infix::ConcatStr))),
        Token::Multiply => (Precedence::PProduct, Some((Assoc::Left, Infix::Multiply))),
        Token::Divide => (Precedence::PProduct, Some((Assoc::Left, Infix::Divide))),
        Token::Percent => (Precedence::PProduct, Some((Assoc::Left, Infix::Remainder))),
        Token::In => (Precedence::PIn, Some((Assoc::Left, Infix::In))),
        Token::Dot => (Precedence::PDot, None),
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
        idents: parse_idents >>
        tag_token!(Token::Assign) >>
        expr: parse_expr >>
        tag_token!(Token::SemiColon) >>
        (LocStmt::let_stmt(t.loc(), idents, expr))
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
    parse_paren_expr |
    parse_list_expr |
    parse_if_expr |
    parse_iter_expr
));

named!(parse_iter_expr<Tokens, LocExpr>,
    do_parse!(
        t: alt!(
            tag_token!(Token::All) |
            tag_token!(Token::Any) |
            tag_token!(Token::Filter) |
            tag_token!(Token::FilterMap) |
            tag_token!(Token::ForEach) |
            tag_token!(Token::Map)
        ) >>
        idents: parse_idents >>
        tag_token!(Token::In) >>
        expr: parse_expr >>
        body: parse_block_stmt >>
        (LocExpr(
            t.loc(),
            Expr::IterExpr {
                op: match t.tok0() {
                    Token::All => Iter::All,
                    Token::Any => Iter::Any,
                    Token::Filter => Iter::Filter,
                    Token::FilterMap => Iter::FilterMap,
                    Token::ForEach => Iter::ForEach,
                    Token::Map => Iter::Map,
                    _ => unreachable!(),
                },
                idents,
                expr: Box::new(expr),
                body
            }
        ))
    )
);

named!(parse_paren_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::LParen) >>
        es: opt!(parse_exprs) >>
        tag_token!(Token::RParen) >>
        (LocExpr(
            t.loc(),
            {
                let es = es.unwrap_or(Vec::new());
                let n = es.len();
                if n == 0 {
                    Expr::LitExpr(Literal::Unit)
                } else if n == 1 {
                    es[0].expr().clone()
                } else {
                    Expr::TupleExpr(es)
                }
            }
        ))
    )
);

named!(parse_lit_expr<Tokens, LocExpr>, alt_complete!(
    do_parse!(
        tag_token!(Token::Dot) >>
        i: parse_int_literal!() >>
        (LocExpr(i.0, Expr::LitExpr(Literal::Float(format!(".{}", i.1).parse().unwrap()))))
    ) |
    do_parse!(
        lit: parse_literal!() >>
        (LocExpr(lit.loc(), Expr::LitExpr(lit.1)))
    )
));

named!(parse_ident_expr<Tokens, LocExpr>,
    alt_complete!(
        do_parse!(
            ident: parse_ident!() >>
            (LocExpr(ident.loc(), Expr::IdentExpr(ident.1)))
        ) |
        do_parse!(
            t: tag_token!(Token::Some) >>
            (LocExpr(t.loc(), Expr::IdentExpr(Ident("option::Some".to_string()))))
        )
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
            (Precedence::PDot, _) if precedence < Precedence::PDot => {
                let (i2, left2) = try_parse!(input, apply!(parse_dot_expr, left));
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

fn parse_dot_expr(input: Tokens, left: LocExpr) -> IResult<Tokens, LocExpr> {
    preceded!(
        input,
        tag_token!(Token::Dot),
        alt_complete!(
            do_parse!(
                i: parse_int_literal!()
                    >> (LocExpr(
                        left.loc(),
                        Expr::CallExpr {
                            loc: i.0,
                            function: i.1.to_string(),
                            arguments: vec![left.clone()],
                        }
                    ))
            ) | do_parse!(
                id: parse_ident_expr
                    >> call: apply!(parse_call_expr, id)
                    >> (match call.expr() {
                        Expr::CallExpr {
                            loc,
                            function,
                            arguments,
                        } => LocExpr(
                            left.loc(),
                            Expr::CallExpr {
                                loc: loc.clone(),
                                function: format!(".::{}", function),
                                arguments: [&vec!(left)[..], &arguments[..]].concat(),
                            }
                        ),
                        _ => unreachable!(),
                    })
            )
        )
    )
}

fn parse_call_expr(input: Tokens, fn_handle: LocExpr) -> IResult<Tokens, LocExpr> {
    do_parse!(
        input,
        tag_token!(Token::LParen)
            >> arguments: opt!(parse_exprs)
            >> tag_token!(Token::RParen)
            >> (LocExpr(
                fn_handle.loc(),
                Expr::CallExpr {
                    loc: fn_handle.loc(),
                    function: match fn_handle.expr().eval_call_function() {
                        Some(s) => s,
                        None => {
                            return Err(nom::Err::Error(error_position!(input, ErrorKind::Tag)))
                        }
                    },
                    arguments: arguments.unwrap_or(Vec::new())
                }
            ))
    )
}

named!(parse_list_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::LBracket) >>
        items: opt!(parse_exprs) >>
        tag_token!(Token::RBracket) >>
        (LocExpr(t.loc(), Expr::ListExpr(items.unwrap_or(Vec::new()))))
    )
);

named!(parse_if_expr<Tokens, LocExpr>,
    do_parse!(
        t: tag_token!(Token::If) >>
        b: alt_complete!(
                do_parse!(m: parse_match_exprs >> (LocExprOrMatches::Matches(m))) |
                do_parse!(s: parse_some_match >> (LocExprOrMatches::SomeMatch(s.0, s.1))) |
                do_parse!(e: parse_expr >> (LocExprOrMatches::Expr(e)))
            ) >>
        consequence: parse_block_stmt >>
        alternative: opt!(parse_else_expr) >>
        (LocExpr(
            t.loc(),
            match b {
                LocExprOrMatches::Expr(expr) => Expr::IfExpr { cond: Box::new(expr), consequence, alternative },
                LocExprOrMatches::Matches(matches) => Expr::IfMatchExpr { matches, consequence, alternative },
                LocExprOrMatches::SomeMatch(var, expr) => Expr::IfSomeMatchExpr { var, expr: Box::new(expr), consequence, alternative },
            }
        ))
    )
);

named!(parse_else_expr<Tokens, BlockStmt>,
    preceded!(
        tag_token!(Token::Else),
        alt_complete!(
            parse_block_stmt |
            do_parse!(e: parse_if_expr >> (vec![LocStmt::expr_stmt(e, false)]))
        )
    )
);

named!(parse_some_match<Tokens, (LocIdent, LocExpr)>,
    do_parse!(
        tag_token!(Token::Let) >>
        tag_token!(Token::Some) >>
        id: delimited!(tag_token!(Token::LParen), parse_ident!(), tag_token!(Token::RParen)) >>
        tag_token!(Token::Assign) >>
        e: parse_expr >>
        ((id, e))
    )
);

named!(parse_match_expr<Tokens, (LocExpr, Pat)>,
    do_parse!(
        e: parse_expr >>
        tag_token!(Token::Matches) >>
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
                Token::Ident(ref s) if s == "i64" => Ok((i1, As::I64)),
                Token::Ident(ref s) if s == "base64" => Ok((i1, As::Base64)),
                Token::Ident(ref s) if s == "str" => Ok((i1, As::Str)),
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
        (Pat::As(i.1, j.unwrap_or(As::Str)))
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

named!(parse_external<Tokens, External>,
    do_parse!(
        tag_token!(Token::External) >>
        name: parse_ident!() >>
        tag_token!(Token::At) >>
        url: parse_string_as_ident!() >>
        headers: delimited!(tag_token!(Token::LBrace), many0!(parse_head), tag_token!(Token::RBrace)) >>
        (External {name, url, headers})
    )
);

named!(parse_head<Tokens, Head>,
    do_parse!(
        tag_token!(Token::Function) >>
        id: parse_ident!() >>
        tag_token!(Token::LParen) >>
        typs: alt_complete!(
                value!(None, tag_token!(Token::Underscore)) |
                do_parse!(typs: opt!(parse_types) >> (Some(typs.unwrap_or(Vec::new()))))
        ) >>
        tag_token!(Token::RParen) >>
        typ: opt!(preceded!(tag_token!(Token::Arrow), parse_type)) >>
        (Head {id, typs, typ})
    )
);

named!(parse_atom_type<Tokens, Typ>,
    do_parse!(
        t: parse_ident!() >>
        oty: opt!(delimited!(tag_token!(Token::LessThan), parse_type, tag_token!(Token::GreaterThan))) >>
        (oty.map(|ty| Typ::Cons(t.clone(), Box::new(ty))).unwrap_or(Typ::Atom(t)))
    )
);

named!(parse_type<Tokens, Typ>, alt_complete!(
    parse_atom_type |
    parse_tuple_type
));

named!(parse_tuple_type<Tokens, Typ>,
    do_parse!(
        tag_token!(Token::LParen) >>
        typs: opt!(parse_types) >>
        tag_token!(Token::RParen) >>
        (Typ::Tuple(typs.unwrap_or(Vec::new())))
    )
);

named!(parse_comma_type<Tokens, Typ>,
    preceded!(tag_token!(Token::Comma), parse_type)
);

named!(parse_types<Tokens, Vec<Typ>>,
    do_parse!(
        ty: parse_type >>
        tys: many0!(parse_comma_type) >>
        ([&vec!(ty)[..], &tys[..]].concat())
    )
);
