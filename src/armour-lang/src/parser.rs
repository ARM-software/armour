/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

/// parser
use super::lexer::{Loc, Token, Tokens};
use super::literals::{Literal, DPFlatLiteral, CPFlatLiteral, TFlatLiteral};
use super::{types};
use super::types::{TFlatTyp};
use nom::error::ErrorKind;
use nom::{self,*};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::marker::PhantomData;

pub type DPExpr = Expr<types::FlatTyp, DPFlatLiteral>;
pub type CPExpr = Expr<types::CPTyp, CPFlatLiteral>;

pub type Program<FlatTyp, FlatLiteral> = Vec<Decl<FlatTyp, FlatLiteral>>;
pub type DPProgram = Program<types::FlatTyp, DPFlatLiteral>;
pub type CPProgram = Program<types::CPTyp, CPFlatLiteral>;

pub enum Decl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    External(External),
    FnDecl(FnDecl<FlatTyp, FlatLiteral>),
    Phantom(PhantomData<Typ>)
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Typ {
    Atom(LocIdent),
    Cons(LocIdent, Box<Typ>),
    Tuple(Vec<Typ>),
}

type DPTyp = Typ;

pub trait TPTyp : Clone + PartialEq {}

impl TPTyp for Typ {}


pub type CPTyp = DPTyp;

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
pub struct FnDecl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    head: FnHead,
    body: BlockStmt<FlatTyp, FlatLiteral>,
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> FnDecl<FlatTyp, FlatLiteral> {
    pub fn name(&self) -> &str {
        self.head.id.id()
    }
    pub fn args(&self) -> &Vec<Param> {
        &self.head.params
    }
    pub fn body(&self) -> &BlockStmt<FlatTyp, FlatLiteral> {
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
pub enum Stmt<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    LetStmt(Vec<LocIdent>, LocExpr<FlatTyp, FlatLiteral>),
    ReturnStmt(LocExpr<FlatTyp, FlatLiteral>),
    ExprStmt {
        exp: Expr<FlatTyp, FlatLiteral>,
        async_tag: bool,
        semi: bool,
    },
}


#[derive(Debug, Clone)]
pub struct LocStmt<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(Loc, Stmt<FlatTyp, FlatLiteral>);

impl<FlatTyp, FlatLiteral> LocStmt<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn let_stmt(l: Loc, i: Vec<LocIdent>, e: LocExpr<FlatTyp, FlatLiteral>) -> LocStmt<FlatTyp, FlatLiteral> {
        LocStmt(l, Stmt::LetStmt(i, e))
    }
    fn return_stmt(l: Loc, e: LocExpr<FlatTyp, FlatLiteral>) -> LocStmt<FlatTyp, FlatLiteral> {
        LocStmt(l, Stmt::ReturnStmt(e))
    }
    fn expr_stmt(e: LocExpr<FlatTyp, FlatLiteral>, async_tag: bool, semi: bool) -> LocStmt<FlatTyp, FlatLiteral> {
        LocStmt(
            e.loc(),
            Stmt::ExprStmt {
                exp: e.1,
                async_tag,
                semi,
            },
        )
    }
    pub fn loc(&self) -> Loc {
        self.0.clone()
    }
    pub fn stmt(&self) -> &Stmt<FlatTyp, FlatLiteral> {
        &self.1
    }
}

#[derive(Debug, Clone)]
pub struct BlockStmt<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub statements: Vec<LocStmt<FlatTyp, FlatLiteral>>,
    async_tag: bool,
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> BlockStmt<FlatTyp, FlatLiteral> {
    pub fn as_ref(&self) -> BlockStmtRef<FlatTyp, FlatLiteral> {
        BlockStmtRef {
            statements: self.statements.as_ref(),
            async_tag: self.async_tag,
        }
    }
    pub fn loc(&self, default: Loc) -> Loc {
        self.statements.get(0).map_or(default, |s| s.loc())
    }
}

impl<FlatTyp, FlatLiteral> From<LocExpr<FlatTyp, FlatLiteral>> for BlockStmt<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn from(e: LocExpr<FlatTyp, FlatLiteral>) -> BlockStmt<FlatTyp, FlatLiteral> {
        BlockStmt {
            statements: vec![LocStmt::expr_stmt(e, false, false)],
            async_tag: false,
        }
    }
}

#[derive(Debug)]
pub struct BlockStmtRef<'a, FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub statements: &'a [LocStmt<FlatTyp, FlatLiteral>],
    async_tag: bool,
}

impl<'a, FlatTyp, FlatLiteral>  BlockStmtRef<'a, FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    pub fn split_first(&self) -> Option<(&LocStmt<FlatTyp, FlatLiteral>, BlockStmtRef<FlatTyp, FlatLiteral>)> {
        if let Some((first, statements)) = self.statements.split_first() {
            Some((
                first,
                BlockStmtRef {
                    statements: statements,
                    async_tag: self.async_tag,
                },
            ))
        } else {
            None
        }
    }
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
    pub fn async_tag(&self) -> bool {
        self.async_tag
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Iter {
    All,
    Any,
    Filter,
    FilterMap,
    ForEach,
    Fold,
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
            Iter::Fold => write!(f, "fold"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    IdentExpr(Ident),
    LitExpr(Literal<FlatTyp, FlatLiteral>),
    ListExpr(Vec<LocExpr<FlatTyp, FlatLiteral>>),
    TupleExpr(Vec<LocExpr<FlatTyp, FlatLiteral>>),
    PrefixExpr(Prefix<FlatTyp>, Box<LocExpr<FlatTyp, FlatLiteral>>),
    InfixExpr(
        Infix<FlatTyp>, 
        Box<LocExpr<FlatTyp, FlatLiteral>>, 
        Box<LocExpr<FlatTyp, FlatLiteral>>
    ),
    IterExpr {
        op: Iter,
        idents: Vec<LocIdent>,
        expr: Box<LocExpr<FlatTyp, FlatLiteral>>,
        body: BlockStmt<FlatTyp, FlatLiteral>,
        accumulator: Option<(LocIdent, Box<LocExpr<FlatTyp, FlatLiteral>>)>
    },
    IfExpr {
        cond: Box<LocExpr<FlatTyp, FlatLiteral>>,
        consequence: BlockStmt<FlatTyp, FlatLiteral>,
        alternative: Option<BlockStmt<FlatTyp, FlatLiteral>>,
    },
    IfMatchExpr {
        matches: Vec<(LocExpr<FlatTyp, FlatLiteral>, Pattern<FlatTyp>)>,
        consequence: BlockStmt<FlatTyp, FlatLiteral>,
        alternative: Option<BlockStmt<FlatTyp, FlatLiteral>>,
    },
    IfSomeMatchExpr {
        var: LocIdent,
        expr: Box<LocExpr<FlatTyp, FlatLiteral>>,
        consequence: BlockStmt<FlatTyp, FlatLiteral>,
        alternative: Option<BlockStmt<FlatTyp, FlatLiteral>>,
    },
    CallExpr {
        loc: Loc,
        function: String,
        arguments: Vec<LocExpr<FlatTyp, FlatLiteral>>,
    },
}


impl<FlatTyp, FlatLiteral> Expr<FlatTyp, FlatLiteral>
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
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
pub struct LocExpr<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(Loc, Expr<FlatTyp, FlatLiteral>);

impl<FlatTyp, FlatLiteral> LocExpr<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    pub fn new(l: &Loc, e: &Expr<FlatTyp, FlatLiteral>) -> Self {
        LocExpr(l.clone(), e.clone())
    }
    pub fn loc(&self) -> Loc {
        self.0.clone()
    }
    pub fn expr(&self) -> &Expr<FlatTyp, FlatLiteral> {
        &self.1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum As {
    Str,
    I64,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
pub enum Pattern<FlatTyp:TFlatTyp> {
    Regex(Pat),
    Label(super::labels::Label),
    Phantom(PhantomData<FlatTyp>)
}

impl Pat {
    fn to_regex_str(&self, ignore_ws: bool) -> String {
        match self {
            Pat::Any => ".".to_string(),
            Pat::Lit(s) => regex::escape(s),
            Pat::Class(s) => (match s.as_str() {
                "alpha" => "[[:alpha:]]",
                "alphanum" => "[[:alnum:]]",
                "base64" => "(([A-Za-z0-9+/]{4})*([A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?)",
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
    fn has_as(&self) -> bool {
        match self {
            Pat::Any | Pat::Lit(_) | Pat::Class(_) => false,
            Pat::As(_, _) => true,
            Pat::Alt(v) | Pat::Seq(v) => v.iter().any(|p| p.has_as()),
            Pat::Opt(p)
            | Pat::Star(p)
            | Pat::Plus(p)
            | Pat::CaseInsensitive(p)
            | Pat::IgnoreWhitespace(p) => p.has_as(),
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
pub struct PolicyRegex(pub Pat, #[serde(with = "serde_regex")] pub Regex);

impl PartialEq for PolicyRegex {
    fn eq(&self, other: &PolicyRegex) -> bool {
        self.0 == other.0
    }
}

impl PolicyRegex {
    pub fn from_pat(p: &Pat) -> Result<PolicyRegex, regex::Error> {
        let re = Regex::new(&format!("^{}$", p.to_regex_str(false)))?;
        Ok(PolicyRegex(p.clone(), re))
    }
    pub fn is_match(&self, s: &str) -> bool {
        self.1.is_match(s)
    }
    pub fn capture_names(&self) -> regex::CaptureNames {
        self.1.capture_names()
    }
    pub fn captures<'a>(&self, s: &'a str) -> Option<regex::Captures<'a>> {
        self.1.captures(s)
    }
}

enum LocExprOrMatches<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    Expr(LocExpr<FlatTyp, FlatLiteral>),
    Matches(Vec<(LocExpr<FlatTyp, FlatLiteral>, Pattern<FlatTyp>)>),
    SomeMatch(LocIdent, LocExpr<FlatTyp, FlatLiteral>),
}

#[derive(Debug, Clone)]
pub struct LocLiteral<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(
    Loc,
    Literal<FlatTyp, FlatLiteral>,
    PhantomData<(FlatTyp, FlatLiteral)>
);
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> LocLiteral<FlatTyp, FlatLiteral> {
    fn new(l: Loc, ll: Literal<FlatTyp, FlatLiteral>) -> Self {
        LocLiteral(l, ll, PhantomData)
    }
    fn loc(&self) -> Loc {
        self.0.clone()
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Ident(pub String);

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Ident(s.to_string())
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
pub enum Prefix<FlatTyp:TFlatTyp> {
    Minus,
    Not,
    Phantom(PhantomData<FlatTyp>)
}

impl<FlatTyp:TFlatTyp> std::fmt::Display for Prefix<FlatTyp>{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Prefix::Minus => write!(f, "-"),
            Prefix::Not => write!(f, "!"),
            Prefix::Phantom(_) => unreachable!()
        }
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Infix<FlatTyp:TFlatTyp> {
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
    Phantom(PhantomData<FlatTyp>)
}

impl<FlatTyp:TFlatTyp> std::fmt::Display for Infix<FlatTyp> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Infix::Equal => write!(f, "=="),
            Infix::NotEqual => write!(f, "!="),
            Infix::Plus => write!(f, "+"),
            Infix::Minus => write!(f, "-"),
            Infix::Divide => write!(f, "/"),
            Infix::Multiply => write!(f, "*"),
            Infix::Remainder => write!(f, "%"),
            Infix::GreaterThanEqual => write!(f, ">="),
            Infix::LessThanEqual => write!(f, "<="),
            Infix::GreaterThan => write!(f, ">"),
            Infix::LessThan => write!(f, "<"),
            Infix::And => write!(f, "&&"),
            Infix::Or => write!(f, "||"),
            Infix::Concat => write!(f, "@"),
            Infix::ConcatStr => write!(f, "++"),
            Infix::Module => write!(f, "::"),
            Infix::In => write!(f, "in"),
            Infix::Dot => write!(f, "."),
            Infix::Phantom(_) => unreachable!()
        }
    }
}

#[derive(PartialEq)]
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
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Incomplete(Needed::Size(1)))
        } else {
            if *t1.tok0() == $tag {
                Ok((i1, t1))
            } else {
                Err(nom::Err::Error(error_position!($i, ErrorKind::Count)))
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
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(name) => Ok((i1, (LocIdent(t1.loc(), Ident(name))))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
            }
        }
    }
  );
);

/// Used to called methods then move self back into self
#[macro_export(local_inner_macros)]
macro_rules! call_mm (
  ($i:expr, $method:path) => (
    {
      $method($i)
    }
  );
  ($i:expr, $self_:ident.$method:ident) => (
    {
      $self_.$method($i)
    }
  );
  ($i:expr, $self_:ident.$method:ident, $($args:expr),* ) => (
    {
      $self_.$method($i, $($args),*)
    }
  );
  ($i:expr, $method:path, $($args:expr),* ) => (
    {
      $method($i, $($args),*)
    }
  );
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

named!(tuple_ident<Tokens, LocIdent>, alt!(
    complete!(do_parse!(u: tag_token!(Token::Underscore) >> (LocIdent(u.loc(), Ident("_".to_string()))))) |
    complete!(parse_ident!())
));

named!(parse_comma_tuple_ident<Tokens, LocIdent>,
    preceded!(tag_token!(Token::Comma), tuple_ident)
);

named!(parse_idents<Tokens, Vec<LocIdent>>, alt!(
    complete!(do_parse!(
        tag_token!(Token::LParen) >>
        id: tuple_ident >>
        ids: many1!(parse_comma_tuple_ident) >>
        tag_token!(Token::RParen) >>
        ([&vec!(id)[..], &ids[..]].concat())
    )) |
    complete!(do_parse!(id: parse_ident!() >> (vec![id]))) |
    value!(vec![LocIdent(Loc::default(), Ident::from("_"))], tag_token!(Token::Underscore))
    )
);

macro_rules! parse_string_as_ident (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::StringLiteral(s) => Ok((i1, LocIdent(t1.loc(), Ident(s)))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
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
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::IntLiteral(i) => Ok((i1, (t1.loc(), i))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
            }
        }
    }
  );
);

macro_rules! parse_label_literal (
    ($i: expr,) => (
      {
          let (i1, t1) = try_parse!($i, take!(1));
          if t1.tok.is_empty() {
              Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
          } else {
              match t1.tok0().clone() {
                  Token::LabelLiteral(i) => Ok((i1, (t1.loc(), i))),
                  _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
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
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::BoolLiteral(b) => Ok((i1, LocLiteral::new(t1.loc(), Literal::bool(b)))),
                Token::DataLiteral(d) => Ok((i1, LocLiteral::new(t1.loc(), Literal::data(d)))),
                Token::FloatLiteral(f) => Ok((i1, LocLiteral::new(t1.loc(), Literal::float(f)))),
                Token::Ident(ref s) if s == "None" => Ok((i1, LocLiteral::new(t1.loc(), Literal::none()))),
                Token::IntLiteral(i) => Ok((i1, LocLiteral::new(t1.loc(), Literal::int(i)))),
                Token::LabelLiteral(l) => Ok((i1, LocLiteral::new(t1.loc(), Literal::label(l)))),
                Token::StringLiteral(s) => Ok((i1, LocLiteral::new(t1.loc(), Literal::str(s)))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
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
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::StringLiteral(s) => Ok((i1, Pat::Lit(s))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
            }
        }
    }
  );
);


lazy_static::lazy_static! {
    static ref CLASSES: HashSet<&'static str> =
        vec!["alpha", "alphanum", "base64", "digit", "hex_digit", "s"]
            .into_iter()
            .collect();
}

macro_rules! parse_pat_class (
  ($i: expr,) => (
    {
        let (i1, t1) = try_parse!($i, take!(1));
        if t1.tok.is_empty() {
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(ref name) if CLASSES.contains(name.as_str()) => Ok((i1, Pat::Class(name.to_string()))),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
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
            Err(nom::Err::Error(error_position!($i, ErrorKind::Tag)))
        } else {
            match t1.tok0().clone() {
                Token::Ident(ref s) if s == "i64" => Ok((i1, As::I64)),
                Token::Ident(ref s) if s == "base64" => Ok((i1, As::Base64)),
                Token::Ident(ref s) if s == "str" => Ok((i1, As::Str)),
                _ => Err(nom::Err::Error(error_position!($i, ErrorKind::Tag))),
            }
        }
    }
  );
);

named!(parse_atom_pat<Tokens, Pat>, alt!(
    complete!(parse_pat_literal!()) |
    complete!(value!(Pat::Any, tag_token!(Token::Dot))) |
    complete!(delimited!(tag_token!(Token::Colon), parse_pat_class!(), tag_token!(Token::Colon))) |
    complete!(parse_as_pat) |
    complete!(delimited!(tag_token!(Token::LParen), parse_pat, tag_token!(Token::RParen)))
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
        postfix: many0!(alt!(
            complete!(value!(Token::Multiply, tag_token!(Token::Multiply)))
                | complete!(value!(Token::Plus, tag_token!(Token::Plus)))
                | complete!(value!(Token::QuestionMark, tag_token!(Token::QuestionMark)))
                | complete!(value!(Token::Not, tag_token!(Token::Not)))
                | complete!(value!(Token::Percent, tag_token!(Token::Percent)))
        )) >>
        ({
            let mut r = a;
            for p in postfix.iter() {
                match p {
                    Token::Multiply => r = Pat::Star(Box::new(r)),
                    Token::Plus => r = Pat::Plus(Box::new(r)),
                    Token::QuestionMark => r = Pat::Opt(Box::new(r)),
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
        (if p.len() == 1 {
            #[allow(clippy::redundant_clone)]
            p.clone().pop().unwrap()
         } else {Pat::Seq(p)})
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


fn parse_pat_no_bind(input: Tokens) -> IResult<Tokens, PolicyRegex> {
    match parse_pat(input) {
        Ok((rest, p)) => {
            if p.has_as() {
                Err(nom::Err::Error(error_position!(input, ErrorKind::Tag)))
            } else if let Ok(r) = PolicyRegex::from_pat(&p) {
                Ok((rest, r))
            } else {
                Err(nom::Err::Error(error_position!(input, ErrorKind::Tag)))
            }
        }
        Err(e) => Err(e),
    }
}

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
        typs: alt!(
                complete!(value!(None, tag_token!(Token::Underscore))) |
                complete!(do_parse!(typs: opt!(parse_types) >> (Some(typs.unwrap_or_default()))))
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

named!(parse_type<Tokens, Typ>, alt!(
    complete!(parse_atom_type) |
    complete!(parse_tuple_type)
));

named!(parse_tuple_type<Tokens, Typ>,
    do_parse!(
        tag_token!(Token::LParen) >>
        typs: opt!(parse_types) >>
        tag_token!(Token::RParen) >>
        (Typ::Tuple(typs.unwrap_or_default()))
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
pub trait TParser<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    //fn parse_program(&self, t:Tokens) -> Program<FlatTyp, FlatLiteral>;
    named!(parse_program<Tokens, Program<FlatTyp, FlatLiteral>>, 
        terminated!(
            many0!(
                alt!(
                    complete!(do_parse!(f: call_mm!(Self::parse_fn_expr) >> (Decl::FnDecl(f)))) |
                    complete!(do_parse!(e: parse_external >> (Decl::External(e))))
                )
            ),
            tag_token!(Token::EOF)
        )
    );

    named!(parse_fn_expr<Tokens, FnDecl<FlatTyp, FlatLiteral>>,
        do_parse!(
            head: call_mm!(Self::parse_fn_head) >>
            body: call_mm!(Self::parse_block_stmt) >>
            (FnDecl {head, body })
        )
    );
    named!(parse_fn_head<Tokens, FnHead>,
        do_parse!(
            tag_token!(Token::Function) >>
            id: parse_ident!() >>
            tag_token!(Token::LParen) >>
            params: opt!(parse_params) >>
            tag_token!(Token::RParen) >>
            typ: opt!(preceded!(tag_token!(Token::Arrow), parse_type)) >>
            (FnHead {id, params: params.unwrap_or_default(), typ})
        )
    );

    fn infix_op(t: &Token) -> (Precedence, Option<(Assoc, Infix<FlatTyp>)>) {
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

    named!(parse_expr_eof<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        terminated!(call_mm!(Self::parse_expr), tag_token!(Token::EOF))
    );

    named!(parse_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>, 
        call_mm!(Self::parse_pratt_expr, Precedence::PLowest)
    );
    named!(parse_block_stmt_eof<Tokens, BlockStmt<FlatTyp, FlatLiteral>>,
        terminated!(call_mm!(Self::parse_block_stmt), tag_token!(Token::EOF))
    );

    named!(parse_block_stmt<Tokens, BlockStmt<FlatTyp, FlatLiteral>>,
        do_parse!(
            async_tag: opt!(tag_token!(Token::Async)) >>
            statements: delimited!(tag_token!(Token::LBrace), many0!(call_mm!(Self::parse_stmt)), tag_token!(Token::RBrace)) >>
            (BlockStmt {statements, async_tag: async_tag.is_some()})
        )
    );

    named!(parse_stmt<Tokens, LocStmt<FlatTyp, FlatLiteral>>, alt!(
        complete!(call_mm!(Self::parse_let_stmt)) |
        complete!(call_mm!(Self::parse_return_stmt)) |
        complete!(call_mm!(Self::parse_expr_stmt))
    ));

    named!(parse_let_stmt<Tokens, LocStmt<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: tag_token!(Token::Let) >>
            idents: parse_idents >>
            tag_token!(Token::Assign) >>
            expr: call_mm!(Self::parse_expr) >>
            tag_token!(Token::SemiColon) >>
            (LocStmt::let_stmt(t.loc(), idents, expr))
        )
    );

    named!(parse_return_stmt<Tokens, LocStmt<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: tag_token!(Token::Return) >>
            expr: call_mm!(Self::parse_expr) >>
            (LocStmt::return_stmt(t.loc(), expr))
        )
    );

    named!(parse_expr_stmt<Tokens, LocStmt<FlatTyp, FlatLiteral>>,
        do_parse!(
            async_tag: opt!(tag_token!(Token::Async)) >>
            expr: call_mm!(Self::parse_expr) >>
            semi: opt!(tag_token!(Token::SemiColon)) >>
            (LocStmt::expr_stmt(expr, async_tag.is_some(), semi.is_some()))
        )
    );

    named!(parse_atom_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>, alt!(
        complete!(call_mm!(Self::parse_lit_expr)) |
        complete!(call_mm!(Self::parse_ident_expr)) |
        complete!(call_mm!(Self::parse_prefix_expr)) |
        complete!(call_mm!(Self::parse_paren_expr)) |
        complete!(call_mm!(Self::parse_list_expr)) |
        complete!(call_mm!(Self::parse_if_expr)) |
        complete!(call_mm!(Self::parse_iter_expr)) |
        complete!(call_mm!(Self::parse_iter_acc_expr)) |
        complete!(call_mm!(Self::parse_all_any_expr))
    ));

    named!(parse_iter_acc_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: alt!(
                tag_token!(Token::Fold)
            ) >>
            idents: parse_idents >>
            tag_token!(Token::In) >>
            expr: call_mm!(Self::parse_expr) >>
            body: call_mm!(Self::parse_block_stmt) >>
            tag_token!(Token::Where) >>
            acc_ident: parse_ident!() >>
            tag_token!(Token::Assign) >>
            acc_expr: call_mm!(Self::parse_expr) >>   
            (LocExpr(
                t.loc(),
                Expr::IterExpr {
                    op: match t.tok0() {
                        Token::Fold => Iter::Fold,
                        _ => unreachable!(),
                    },
                    idents,
                    expr: Box::new(expr),
                    body,
                    accumulator: Some((acc_ident, Box::new(acc_expr)))
                }
            ))
        )
    );

    named!(parse_iter_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
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
            expr: call_mm!(Self::parse_expr) >>
            body: call_mm!(Self::parse_block_stmt) >>
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
                    body,
                    accumulator: None 
                }
            ))
        )
    );

    named!(parse_all_any_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: alt!(
                tag_token!(Token::All) |
                tag_token!(Token::Any)
            ) >>
            expr: call_mm!(Self::parse_list_expr) >>
            (LocExpr(
                t.loc(),
                Expr::IterExpr {
                    op: match t.tok0() {
                        Token::All => Iter::All,
                        Token::Any => Iter::Any,
                        _ => unreachable!(),
                    },
                    idents: vec![LocIdent(Loc::default(), Ident::from("x"))],
                    expr: Box::new(expr),
                    body: BlockStmt::from(LocExpr(Loc::default(), Expr::IdentExpr(Ident::from("x")))),
                    accumulator: None
                }
            ))
        )
    );

    named!(parse_paren_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: tag_token!(Token::LParen) >>
            es: opt!(call_mm!(Self::parse_exprs)) >>
            tag_token!(Token::RParen) >>
            (LocExpr(
                t.loc(),
                {
                    let es = es.unwrap_or_default();
                    let n = es.len();
                    if n == 0 {
                        Expr::LitExpr(Literal::unit())
                    } else if n == 1 {
                        es[0].expr().clone()
                    } else {
                        Expr::TupleExpr(es)
                    }
                }
            ))
        )
    );

    named!(parse_lit_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>, alt!(
        complete!(do_parse!(
            tag_token!(Token::Dot) >>
            i: parse_int_literal!() >>
            (LocExpr(i.0, Expr::LitExpr(Literal::float(format!(".{}", i.1).parse().unwrap()))))
        )) |
        complete!(do_parse!(
            lit: parse_literal!() >>
            (LocExpr(lit.loc(), Expr::LitExpr(lit.1)))
        )) |
        complete!(do_parse!(
            t: tag_token!(Token::Regex) >>
            tag_token!(Token::LParen) >>
            regex: parse_pat_no_bind >>
            tag_token!(Token::RParen) >>
            (LocExpr(t.loc(), Expr::LitExpr(Literal::regex(regex))))
        ))
    ));

    named!(parse_ident_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        alt!(
            complete!(do_parse!(
                ident: parse_ident!() >>
                (LocExpr(ident.loc(), Expr::IdentExpr(ident.1)))
            )) |
            complete!(do_parse!(
                t: tag_token!(Token::Some) >>
                (LocExpr(t.loc(), Expr::IdentExpr(Ident("option::Some".to_string()))))
            ))
        )
    );

    named!(parse_comma_exprs<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        preceded!(tag_token!(Token::Comma), call_mm!(Self::parse_expr))
    );

    named!(parse_exprs<Tokens, Vec<LocExpr<FlatTyp, FlatLiteral>>>,
        do_parse!(
            e: call_mm!(Self::parse_expr) >>
            es: many0!(call_mm!(Self::parse_comma_exprs)) >>
            ([&vec!(e)[..], &es[..]].concat())
        )
    );
    fn parse_prefix_expr(input: Tokens) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        let (i1, t1) = try_parse!(
            input,
            alt!(complete!(tag_token!(Token::Minus)) | complete!(tag_token!(Token::Not)))
        );

        if t1.tok.is_empty() {
            Err(Err::Error(error_position!(input, ErrorKind::Tag)))
        } else {
            let (i2, e) = try_parse!(i1, call_mm!(Self::parse_expr));

            match t1.tok0().clone() {
                Token::Minus => Ok((
                    i2,
                    LocExpr(t1.loc(), Expr::PrefixExpr(Prefix::Minus, Box::new(e))),
                )),
                Token::Not => Ok((
                    i2,
                    LocExpr(t1.loc(), Expr::PrefixExpr(Prefix::Not, Box::new(e))),
                )),
                _ => Err(Err::Error(error_position!(input, ErrorKind::Tag))),
            }
        }
    }

    fn parse_pratt_expr(input: Tokens, precedence: Precedence) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        do_parse!(
            input,
            left: call_mm!(Self::parse_atom_expr) >> i: call_mm!(Self::go_parse_pratt_expr, precedence, left) >> (i)
        )
    }

    fn go_parse_pratt_expr(
        input: Tokens,
        precedence: Precedence,
        left: LocExpr<FlatTyp, FlatLiteral>,
    ) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        let (i1, t1) = try_parse!(input, take!(1));
        if t1.tok.is_empty() {
            Ok((i1, left))
        } else {
            let preview = t1.tok0().clone();
            match Self::infix_op(&preview) {
                (Precedence::PDot, _) if precedence < Precedence::PDot => {
                    let (i2, left2) = try_parse!(input, call_mm!(Self::parse_dot_expr, left));
                    Self::go_parse_pratt_expr(i2, precedence, left2)
                }
                (Precedence::PCall, _) if precedence < Precedence::PCall => {
                    let (i2, left2) = try_parse!(input, call_mm!(Self::parse_call_expr, left));
                    Self::go_parse_pratt_expr(i2, precedence, left2)
                }
                (ref peek_precedence, Some((Assoc::Right, _))) if precedence <= *peek_precedence => {
                    let (i2, left2) = try_parse!(input, call_mm!(Self::parse_infix_expr, left));
                    Self::go_parse_pratt_expr(i2, precedence, left2)
                }
                (ref peek_precedence, _) if precedence < *peek_precedence => {
                    let (i2, left2) = try_parse!(input, call_mm!(Self::parse_infix_expr, left));
                    Self::go_parse_pratt_expr(i2, precedence, left2)
                }
                _ => Ok((input, left)),
            }
        }
    }

    fn parse_infix_expr(input: Tokens, left: LocExpr<FlatTyp, FlatLiteral>) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        let (i1, t1) = try_parse!(input, take!(1));
        if t1.tok.is_empty() {
            Err(Err::Error(error_position!(input, ErrorKind::Tag)))
        } else {
            let next = t1.tok0().clone();
            let (precedence, maybe_op) = Self::infix_op(&next);
            match maybe_op {
                None => Err(Err::Error(error_position!(input, ErrorKind::Tag))),
                Some((_, op)) => {
                    let (i2, right) = try_parse!(i1, call_mm!(Self::parse_pratt_expr, precedence));
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

    // fn parse_dot_expr(input: Tokens, left: LocExpr) -> IResult<Tokens, LocExpr<Typ, FlatLiteral>> {
    fn parse_dot_expr(input: Tokens, left: LocExpr<FlatTyp, FlatLiteral>) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        let left_clone = left.clone();
        alt!(
            input,
            complete!(do_parse!(
                tag_token!(Token::Dot)
                    >> i: parse_int_literal!()
                    >> (LocExpr(
                        left_clone.loc(),
                        Expr::CallExpr {
                            loc: i.0,
                            function: i.1.to_string(),
                            arguments: vec![left_clone.clone()],
                        }
                    ))
            )) | complete!(do_parse!(
                tag_token!(Token::Dot)
                    >> id: call_mm!(Self::parse_ident_expr)
                    >> call: call_mm!(Self::parse_call_expr, id)
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
                                // arguments: vec![],
                                arguments: [&vec!(left.clone())[..], &arguments[..]].concat(),
                            }
                        ),
                        _ => unreachable!(),
                    })
            ))
        )
    }

    fn parse_call_expr(input: Tokens, fn_handle: LocExpr<FlatTyp, FlatLiteral>) -> IResult<Tokens, LocExpr<FlatTyp, FlatLiteral>> {
        do_parse!(
            input,
            tag_token!(Token::LParen)
                >> arguments: opt!(call_mm!(Self::parse_exprs))
                >> tag_token!(Token::RParen)
                >> (LocExpr(
                    fn_handle.loc(),
                    Expr::CallExpr {
                        loc: fn_handle.loc(),
                        function: match fn_handle.expr().eval_call_function() {
                            Some(s) => s,
                            None => {
                                return Err(nom::Err::Error(error_position!(input, ErrorKind::Tag)));
                            }
                        },
                        arguments: arguments.unwrap_or_default()
                    }
                ))
        )
    }

    named!(parse_list_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: tag_token!(Token::LBracket) >>
            items: opt!(call_mm!(Self::parse_exprs)) >>
            tag_token!(Token::RBracket) >>
            (LocExpr(t.loc(), Expr::ListExpr(items.unwrap_or_default())))
        )
    );

    named!(parse_if_expr<Tokens, LocExpr<FlatTyp, FlatLiteral>>,
        do_parse!(
            t: tag_token!(Token::If) >>
            b: alt!(
                    complete!(do_parse!(m: call_mm!(Self::parse_match_exprs) >> (LocExprOrMatches::Matches(m)))) |
                    complete!(do_parse!(s: call_mm!(Self::parse_some_match) >> (LocExprOrMatches::SomeMatch(s.0, s.1)))) |
                    complete!(do_parse!(e: call_mm!(Self::parse_expr) >> (LocExprOrMatches::Expr(e))))
                ) >>
            consequence: call_mm!(Self::parse_block_stmt) >>
            alternative: opt!(call_mm!(Self::parse_else_expr)) >>
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

    named!(parse_else_expr<Tokens, BlockStmt<FlatTyp, FlatLiteral>>,
        preceded!(
            tag_token!(Token::Else),
            alt!(
                complete!(call_mm!(Self::parse_block_stmt)) |
                complete!(do_parse!(e: call_mm!(Self::parse_if_expr) >> (BlockStmt::from(e))))
            )
        )
    );

    named!(parse_some_match<Tokens, (LocIdent, LocExpr<FlatTyp, FlatLiteral>)>,
        do_parse!(
            tag_token!(Token::Let) >>
            tag_token!(Token::Some) >>
            id: delimited!(tag_token!(Token::LParen), parse_ident!(), tag_token!(Token::RParen)) >>
            tag_token!(Token::Assign) >>
            e: call_mm!(Self::parse_expr) >>
            ((id, e))
        )
    );

    named!(parse_match_expr<Tokens, (LocExpr<FlatTyp, FlatLiteral>, Pattern<FlatTyp>)>,
        do_parse!(
            e: call_mm!(Self::parse_expr) >>
            tag_token!(Token::Matches) >>
            pat: call_mm!(Self::parse_pattern) >>
            ((e, pat))
        )
    );

    named!(parse_and_match_exprs<Tokens, (LocExpr<FlatTyp, FlatLiteral>, Pattern<FlatTyp>)>,
        preceded!(tag_token!(Token::And), call_mm!(Self::parse_match_expr))
    );

    named!(parse_match_exprs<Tokens, Vec<(LocExpr<FlatTyp, FlatLiteral>, Pattern<FlatTyp>)>>,
        do_parse!(
            e: call_mm!(Self::parse_match_expr) >>
            es: many0!(call_mm!(Self::parse_and_match_exprs)) >>
            ([&vec!(e)[..], &es[..]].concat())
        )
    );

    named!(parse_pattern<Tokens, Pattern<FlatTyp>>, alt!(
            complete!(do_parse!(p: parse_pat >> (Pattern::Regex(p)))) |
            complete!(do_parse!(p: parse_label_literal!() >> (Pattern::Label(p.1))))
        )
    );
}


pub struct Parser<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> (PhantomData<FlatTyp>, PhantomData<FlatLiteral>); 
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Parser<FlatTyp, FlatLiteral> {
    pub fn new() -> Self { Parser(PhantomData, PhantomData) }
}  
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> TParser<FlatTyp, FlatLiteral>  for Parser<FlatTyp, FlatLiteral>  {
}

//If need wue the following stuff + pass parser object as argument:TParser in expression and lang
//pub type DPParser = Parser<types::FlatTyp, DPFlatLiteral>; 
//impl TParser<types::FlatTyp, DPFlatLiteral> for DPParser {
//}
//
//pub type CPParser = Parser<types_cp::CPFlatTyp, CPFlatLiteral>; 
//impl TParser<types_cp::CPFlatTyp, CPFlatLiteral> for CPParser {
//}


