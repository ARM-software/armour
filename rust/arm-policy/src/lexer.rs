use super::literals;
/// lexer
// Originally based on https://github.com/Rydgel/monkey-rust/tree/master/lib/lexer
// There have been significant modifications, in particular making use of nom_locate
use nom::types::CompleteStr;
use nom::*;
use nom_locate::LocatedSpan;
use std::fmt;
use std::iter::Enumerate;
use std::ops::{Range, RangeFrom, RangeFull, RangeTo};

pub type Span<'a> = LocatedSpan<CompleteStr<'a>>;

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Loc(u32, usize);

impl<'a> fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"line {}, column {}"#, self.0, self.1,)
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Token {
    Illegal,
    EOF,
    // identifier and literals
    Comment(String),
    Ident(String),
    DataLiteral(Vec<u8>),
    StringLiteral(String),
    FloatLiteral(f64),
    IntLiteral(i64),
    BoolLiteral(bool),
    PolicyLiteral(literals::Policy),
    Some,
    // statements
    Assign,
    If,
    Else,
    // operators
    Plus,
    Minus,
    Divide,
    Multiply,
    Equal,
    NotEqual,
    GreaterThanEqual,
    LessThanEqual,
    GreaterThan,
    LessThan,
    Not,
    And,
    Or,
    Dot,
    PlusPlus, // concat
    Percent,  // remainder
    Optional,
    // reserved words
    Function,
    All,
    Any,
    Filter,
    FilterMap,
    ForEach,
    Map,
    Let,
    Return,
    In,
    Matches,
    AndAlso,
    As,
    External,
    // punctuation
    Comma,
    Colon,
    SemiColon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    // misc.
    Underscore,
    Arrow,
    Bar,
    ColonColon,
    At,
}

#[derive(Clone, PartialEq, Debug)]
pub struct LocToken<'a> {
    pub loc: Span<'a>,
    pub tok: Token,
}

impl<'a> LocToken<'a> {
    pub fn new(loc: Span<'a>, tok: Token) -> LocToken<'a> {
        LocToken { loc, tok }
    }
    fn new_span(line: u32, offset: usize, s: &'a str, tok: Token) -> LocToken<'a> {
        LocToken {
            loc: LocatedSpan {
                line,
                offset,
                fragment: CompleteStr(s),
            },
            tok,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub struct Tokens<'a> {
    pub tok: &'a [LocToken<'a>],
    pub start: usize,
    pub end: usize,
}

impl<'a> Tokens<'a> {
    pub fn new(vec: &'a Vec<LocToken>) -> Self {
        Tokens {
            tok: vec.as_slice(),
            start: 0,
            end: vec.len(),
        }
    }
    pub fn tok0(&self) -> &Token {
        &self.tok[0].tok
    }
    pub fn loc(&self) -> Loc {
        let loc = self.tok[0].loc;
        Loc(loc.line, loc.get_column())
    }
}

impl<'a> nom::InputLength for Tokens<'a> {
    #[inline]
    fn input_len(&self) -> usize {
        self.tok.len()
    }
}

impl<'a> nom::AtEof for Tokens<'a> {
    #[inline]
    fn at_eof(&self) -> bool {
        true
    }
}

impl<'a> nom::InputTake for Tokens<'a> {
    #[inline]
    fn take(&self, count: usize) -> Self {
        Tokens {
            tok: &self.tok[0..count],
            start: 0,
            end: count,
        }
    }

    #[inline]
    fn take_split(&self, count: usize) -> (Self, Self) {
        let (prefix, suffix) = self.tok.split_at(count);
        let first = Tokens {
            tok: prefix,
            start: 0,
            end: prefix.len(),
        };
        let second = Tokens {
            tok: suffix,
            start: 0,
            end: suffix.len(),
        };
        (second, first)
    }
}

impl nom::InputLength for Token {
    #[inline]
    fn input_len(&self) -> usize {
        1
    }
}

impl<'a> nom::Slice<Range<usize>> for Tokens<'a> {
    #[inline]
    fn slice(&self, range: Range<usize>) -> Self {
        Tokens {
            tok: self.tok.slice(range.clone()),
            start: self.start + range.start,
            end: self.start + range.end,
        }
    }
}

impl<'a> nom::Slice<RangeTo<usize>> for Tokens<'a> {
    #[inline]
    fn slice(&self, range: RangeTo<usize>) -> Self {
        self.slice(0..range.end)
    }
}

impl<'a> nom::Slice<RangeFrom<usize>> for Tokens<'a> {
    #[inline]
    fn slice(&self, range: RangeFrom<usize>) -> Self {
        self.slice(range.start..self.end - self.start)
    }
}

impl<'a> nom::Slice<RangeFull> for Tokens<'a> {
    #[inline]
    fn slice(&self, _: RangeFull) -> Self {
        Tokens {
            tok: self.tok,
            start: self.start,
            end: self.end,
        }
    }
}

impl<'a> nom::InputIter for Tokens<'a> {
    type Item = &'a LocToken<'a>;
    type RawItem = LocToken<'a>;
    type Iter = Enumerate<::std::slice::Iter<'a, LocToken<'a>>>;
    type IterElem = ::std::slice::Iter<'a, LocToken<'a>>;

    #[inline]
    fn iter_indices(&self) -> Enumerate<::std::slice::Iter<'a, LocToken<'a>>> {
        self.tok.iter().enumerate()
    }
    #[inline]
    fn iter_elements(&self) -> ::std::slice::Iter<'a, LocToken<'a>> {
        self.tok.iter()
    }
    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::RawItem) -> bool,
    {
        self.tok.iter().position(|b| predicate(b.clone()))
    }
    #[inline]
    fn slice_index(&self, count: usize) -> Option<usize> {
        if self.tok.len() >= count {
            Some(count)
        } else {
            None
        }
    }
}

struct DisplaySpan<'a>(Span<'a>);

impl<'a> fmt::Display for DisplaySpan<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#"«{}» at line {}, column {}"#,
            self.0.fragment,
            self.0.line,
            self.0.get_column(),
        )
    }
}

impl<'a> fmt::Display for LocToken<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        DisplaySpan(self.loc).fmt(f)
    }
}

impl<'a> fmt::Display for Tokens<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[")?;
        for loc_tok in self.tok.iter() {
            writeln!(f, "{:?},", loc_tok.tok)?
        }
        write!(f, "]")
    }
}

// Symbols
named!(lex_operator<Span, LocToken>,
    switch!(
        take!(1),
        t @ LocatedSpan{fragment: CompleteStr("@"), ..} => value!(LocToken::new(t, Token::At)) |
        t @ LocatedSpan{fragment: CompleteStr("?"), ..} => value!(LocToken::new(t, Token::Optional)) |
        t @ LocatedSpan{fragment: CompleteStr("."), ..} => value!(LocToken::new(t, Token::Dot)) |
        t @ LocatedSpan{fragment: CompleteStr("_"), ..} => value!(LocToken::new(t, Token::Underscore)) |
        t @ LocatedSpan{fragment: CompleteStr("*"), ..} => value!(LocToken::new(t, Token::Multiply)) |
        t @ LocatedSpan{fragment: CompleteStr("%"), ..} => value!(LocToken::new(t, Token::Percent)) |
        t @ LocatedSpan{fragment: CompleteStr(","), ..} => value!(LocToken::new(t, Token::Comma)) |
        t @ LocatedSpan{fragment: CompleteStr(";"), ..} => value!(LocToken::new(t, Token::SemiColon)) |
        t @ LocatedSpan{fragment: CompleteStr("("), ..} => value!(LocToken::new(t, Token::LParen)) |
        t @ LocatedSpan{fragment: CompleteStr(")"), ..} => value!(LocToken::new(t, Token::RParen)) |
        t @ LocatedSpan{fragment: CompleteStr("{"), ..} => value!(LocToken::new(t, Token::LBrace)) |
        t @ LocatedSpan{fragment: CompleteStr("}"), ..} => value!(LocToken::new(t, Token::RBrace)) |
        t @ LocatedSpan{fragment: CompleteStr("["), ..} => value!(LocToken::new(t, Token::LBracket)) |
        t @ LocatedSpan{fragment: CompleteStr("]"), ..} => value!(LocToken::new(t, Token::RBracket)) |

        LocatedSpan{fragment: CompleteStr("+"), line: l, offset: o} => 
            alt!(
               do_parse!(tag!("+") >> (LocToken::new_span(l, o, "++", Token::PlusPlus))) |
               value!(LocToken::new_span(l, o, "+", Token::Plus))
            ) |
        LocatedSpan{fragment: CompleteStr(":"), line: l, offset: o} => 
            alt!(
               do_parse!(tag!(":") >> (LocToken::new_span(l, o, "::", Token::ColonColon))) |
               value!(LocToken::new_span(l, o, ":", Token::Colon))
            ) |
        LocatedSpan{fragment: CompleteStr("/"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("/") >> t: not_line_ending >> (LocToken::new(t, Token::Comment(t.to_string())))) |
                value!(LocToken::new_span(l, o, "/", Token::Divide))
            ) |
        LocatedSpan{fragment: CompleteStr("|"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("|") >> (LocToken::new_span(l, o, "||", Token::Or))) |
                value!(LocToken::new_span(l, o, "|", Token::Bar))
            ) |
        LocatedSpan{fragment: CompleteStr("&"), line: l, offset: o} =>
            do_parse!(tag!("&") >> (LocToken::new_span(l, o, "&&", Token::And))) |
        LocatedSpan{fragment: CompleteStr("-"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!(">") >> (LocToken::new_span(l, o, "->", Token::Arrow))) |
                value!(LocToken::new_span(l, o, "-", Token::Minus))
            ) |
        LocatedSpan{fragment: CompleteStr("<"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "<=", Token::LessThanEqual))) |
                value!(LocToken::new_span(l, o, "<", Token::LessThan))
            ) |
        LocatedSpan{fragment: CompleteStr(">"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, ">=", Token::GreaterThanEqual))) |
                value!(LocToken::new_span(l, o, ">", Token::GreaterThan))
            ) |
        LocatedSpan{fragment: CompleteStr("="), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "==", Token::Equal))) |
                value!(LocToken::new_span(l, o, "=", Token::Assign))
            ) |
        LocatedSpan{fragment: CompleteStr("!"), line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "!=", Token::NotEqual))) |
                value!(LocToken::new_span(l, o, "!", Token::Not))
            )
  )
);

fn lex_number_<'a>(input: Span<'a>, double_span: Span<'a>) -> IResult<Span<'a>, LocToken<'a>> {
    let float = double_span.to_string();
    if float.ends_with('.') {
        if float.starts_with('-') {
            do_parse!(input, t: tag!("-") >> (LocToken::new(t, Token::Minus)))
        } else {
            do_parse!(
                input,
                i: recognize!(digit)
                    >> (LocToken::new(i, Token::IntLiteral(i.to_string().parse().unwrap())))
            )
        }
    } else if float.starts_with('.') && !float.contains('e') {
        do_parse!(input, t: tag!(".") >> (LocToken::new(t, Token::Dot)))
    } else if let Ok(number) = float.parse::<i64>() {
        do_parse!(
            input,
            i: opt!(tag!("-"))
                >> j: recognize!(digit)
                >> (LocToken::new(i.unwrap_or(j), Token::IntLiteral(number)))
        )
    } else {
        do_parse!(
            input,
            d: recognize!(double)
                >> (LocToken::new(d, Token::FloatLiteral(float.parse().unwrap())))
        )
    }
}

// Number literals
fn lex_number<'a>(input: Span<'a>) -> IResult<Span, LocToken<'a>> {
    do_parse!(
        input,
        d: peek!(recognize!(double)) >> r: apply!(lex_number_, d) >> (r)
    )
}

// Reserved or ident
fn parse_reserved<'a>(t: Span<'a>) -> LocToken<'a> {
    let string = t.to_string();
    match string.as_ref() {
        "fn" => LocToken::new(t, Token::Function),
        "all" => LocToken::new(t, Token::All),
        "any" => LocToken::new(t, Token::Any),
        "filter" => LocToken::new(t, Token::Filter),
        "filter_map" => LocToken::new(t, Token::FilterMap),
        "foreach" => LocToken::new(t, Token::ForEach),
        "map" => LocToken::new(t, Token::Map),
        "let" => LocToken::new(t, Token::Let),
        "if" => LocToken::new(t, Token::If),
        "else" => LocToken::new(t, Token::Else),
        "return" => LocToken::new(t, Token::Return),
        "true" => LocToken::new(t, Token::BoolLiteral(true)),
        "false" => LocToken::new(t, Token::BoolLiteral(false)),
        "Accept" => LocToken::new(t, Token::PolicyLiteral(literals::Policy::Accept)),
        "Forward" => LocToken::new(t, Token::PolicyLiteral(literals::Policy::Forward)),
        "Reject" => LocToken::new(t, Token::PolicyLiteral(literals::Policy::Reject)),
        "in" => LocToken::new(t, Token::In),
        "matches" => LocToken::new(t, Token::Matches),
        "and" => LocToken::new(t, Token::AndAlso),
        "as" => LocToken::new(t, Token::As),
        "external" => LocToken::new(t, Token::External),
        "Some" => LocToken::new(t, Token::Some),
        _ => LocToken::new(t, Token::Ident(string)),
    }
}

fn take1alpha(s: Span) -> IResult<Span, ()> {
    if let Some(c) = s.fragment.as_bytes().iter().next() {
        if c.as_char().is_alphabetic() {
            Ok((s, ()))
        } else {
            Err(Err::Incomplete(Needed::Size(1)))
        }
    } else {
        Err(Err::Incomplete(Needed::Size(1)))
    }
}

named!(lex_reserved_ident<Span, LocToken>,
    do_parse!(
        peek!(take1alpha) >>
        s: take_while1!(|c| char::is_alphanumeric(c) || c == '_') >>
        (parse_reserved(s))
    )
);

// String literals
named!(not_escaped<Span, Span>,
    do_parse!(peek!(not!(one_of!("\\\""))) >> t: take!(1) >> (t))
);

named!(lex_string<Span, LocToken>,
    do_parse!(
        b: alt!(tag!("b\"") | tag!("\"")) >>
        t: escaped!(call!(not_escaped), '\\', one_of!("\\\"")) >>
        tag!("\"") >>
        (LocToken::new(
            t,
            {
                let s = t.to_string().replace("\\\"", "\"").replace("\\\\", "\\");
                if b.to_string().starts_with('b') {
                    Token::DataLiteral(s.as_bytes().to_vec())
                } else {
                    Token::StringLiteral(s)
                }
            }
        ))
    )
);

// Illegal tokens
named!(lex_illegal<Span, LocToken>,
    do_parse!(t: take!(1) >> (LocToken::new(t, Token::Illegal)))
);

named!(lex_token<Span, LocToken>,
    alt_complete!(
        lex_number |
        lex_string |
        lex_reserved_ident |
        lex_operator |
        lex_illegal
    )
);

named!(lex_tokens<Span, Vec<LocToken>>, ws!(many0!(lex_token)));

pub fn lex(buf: &str) -> Vec<LocToken> {
    let (loc, mut tokens) = lex_tokens(Span::new(CompleteStr(buf))).unwrap();
    tokens.push(LocToken::new(loc, Token::EOF));
    tokens
        .into_iter()
        .filter(|t| match t {
            LocToken {
                tok: Token::Comment(_),
                ..
            } => false,
            _ => true,
        })
        .collect::<Vec<LocToken>>()
}
