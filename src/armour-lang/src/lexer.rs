/// lexer
// Originally based on https://github.com/Rydgel/monkey-rust/tree/master/lib/lexer
// There have been significant modifications, in particular making use of nom_locate
use super::labels;
use nom::character::complete::{digit1, multispace0, not_line_ending};
use nom::number::complete::recognize_float;
use nom::*;
use nom5_locate::LocatedSpan;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Enumerate;
use std::ops::{Range, RangeFrom, RangeFull, RangeTo};

pub type Span<'a> = LocatedSpan<&'a str>;

#[derive(Default, PartialEq, Eq, Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Loc(u32, usize);

impl Loc {
    pub fn dummy() -> Self {
        Loc(0,0)
    }
}

impl<'a> fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"line {}, column {}"#, self.0, self.1,)
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Token {
    Illegal(String),
    EOF,
    // identifier and literals
    Comment(String),
    Ident(String),
    BoolLiteral(bool),
    DataLiteral(Vec<u8>),
    FloatLiteral(f64),
    IntLiteral(i64),
    LabelLiteral(labels::Label),
    StringLiteral(String),
    Some,
    Regex,
    // statements
    Assign,
    If,
    Else,
    Async,
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
    PlusPlus,     // concat
    Percent,      // remainder
    QuestionMark, // regex option
    // reserved words
    Function,
    All,
    Any,
    Filter,
    FilterMap,
    ForEach,
    Fold,
    Map,
    Let,
    Return,
    In,
    Matches,
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
    fn new_span(line: u32, offset: usize, fragment: &'a str, tok: Token) -> LocToken<'a> {
        LocToken {
            loc: LocatedSpan {
                line,
                offset,
                fragment,
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
    pub fn new(vec: &'a [LocToken]) -> Self {
        Tokens {
            tok: vec,
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
        P: Fn(Self::Item) -> bool,
    {
        self.tok.iter().position(|b| predicate(b))
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
        t @ LocatedSpan{fragment: "@", ..} => value!(LocToken::new(t, Token::At)) |
        t @ LocatedSpan{fragment: "?", ..} => value!(LocToken::new(t, Token::QuestionMark)) |
        t @ LocatedSpan{fragment: ".", ..} => value!(LocToken::new(t, Token::Dot)) |
        t @ LocatedSpan{fragment: "_", ..} => value!(LocToken::new(t, Token::Underscore)) |
        t @ LocatedSpan{fragment: "*", ..} => value!(LocToken::new(t, Token::Multiply)) |
        t @ LocatedSpan{fragment: "%", ..} => value!(LocToken::new(t, Token::Percent)) |
        t @ LocatedSpan{fragment: ",", ..} => value!(LocToken::new(t, Token::Comma)) |
        t @ LocatedSpan{fragment: ";", ..} => value!(LocToken::new(t, Token::SemiColon)) |
        t @ LocatedSpan{fragment: "(", ..} => value!(LocToken::new(t, Token::LParen)) |
        t @ LocatedSpan{fragment: ")", ..} => value!(LocToken::new(t, Token::RParen)) |
        t @ LocatedSpan{fragment: "{", ..} => value!(LocToken::new(t, Token::LBrace)) |
        t @ LocatedSpan{fragment: "}", ..} => value!(LocToken::new(t, Token::RBrace)) |
        t @ LocatedSpan{fragment: "[", ..} => value!(LocToken::new(t, Token::LBracket)) |
        t @ LocatedSpan{fragment: "]", ..} => value!(LocToken::new(t, Token::RBracket)) |

        LocatedSpan{fragment: "+", line: l, offset: o} =>
            alt!(
               do_parse!(tag!("+") >> (LocToken::new_span(l, o, "++", Token::PlusPlus))) |
               value!(LocToken::new_span(l, o, "+", Token::Plus))
            ) |
        LocatedSpan{fragment: ":", line: l, offset: o} =>
            alt!(
               do_parse!(tag!(":") >> (LocToken::new_span(l, o, "::", Token::ColonColon))) |
               value!(LocToken::new_span(l, o, ":", Token::Colon))
            ) |
        LocatedSpan{fragment: "/", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("/") >> t: not_line_ending >> (LocToken::new(t, Token::Comment(t.to_string())))) |
                value!(LocToken::new_span(l, o, "/", Token::Divide))
            ) |
        LocatedSpan{fragment: "|", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("|") >> (LocToken::new_span(l, o, "||", Token::Or))) |
                value!(LocToken::new_span(l, o, "|", Token::Bar))
            ) |
        LocatedSpan{fragment: "&", line: l, offset: o} =>
            do_parse!(tag!("&") >> (LocToken::new_span(l, o, "&&", Token::And))) |
        LocatedSpan{fragment: "-", line: l, offset: o} =>
            alt!(
                do_parse!(tag!(">") >> (LocToken::new_span(l, o, "->", Token::Arrow))) |
                value!(LocToken::new_span(l, o, "-", Token::Minus))
            ) |
        LocatedSpan{fragment: "<", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "<=", Token::LessThanEqual))) |
                value!(LocToken::new_span(l, o, "<", Token::LessThan))
            ) |
        LocatedSpan{fragment: ">", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, ">=", Token::GreaterThanEqual))) |
                value!(LocToken::new_span(l, o, ">", Token::GreaterThan))
            ) |
        LocatedSpan{fragment: "=", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "==", Token::Equal))) |
                value!(LocToken::new_span(l, o, "=", Token::Assign))
            ) |
        LocatedSpan{fragment: "!", line: l, offset: o} =>
            alt!(
                do_parse!(tag!("=") >> (LocToken::new_span(l, o, "!=", Token::NotEqual))) |
                value!(LocToken::new_span(l, o, "!", Token::Not))
            )
  )
);

// Number literals
fn lex_number<'a, E: error::ParseError<Span<'a>>>(
    input: Span<'a>,
) -> IResult<Span<'a>, LocToken, E> {
    match recognize_float::<Span, E>(input) {
        Ok((_rest, float)) => {
            if float.fragment.ends_with('.') {
                if float.fragment.starts_with('-') {
                    do_parse!(input, t: tag!("-") >> (LocToken::new(t, Token::Minus)))
                } else {
                    do_parse!(
                        input,
                        i: recognize!(digit1)
                            >> (LocToken::new(
                                i,
                                Token::IntLiteral(i.to_string().parse().unwrap())
                            ))
                    )
                }
            } else if float.fragment.starts_with('.') && !float.fragment.contains('e') {
                do_parse!(input, t: tag!(".") >> (LocToken::new(t, Token::Dot)))
            } else if let Ok(number) = float.fragment.parse::<i64>() {
                do_parse!(
                    input,
                    opt!(tag!("-"))
                        >> t: recognize!(digit1)
                        >> (LocToken::new(t, Token::IntLiteral(number)))
                )
            } else {
                do_parse!(
                    input,
                    t: recognize_float
                        >> (LocToken::new(t, Token::FloatLiteral(float.fragment.parse().unwrap())))
                )
            }
        }
        Err(nom::Err::Failure(e)) => Err(nom::Err::Error(e)),
        Err(e) => Err(e),
    }
}

// Reserved or ident
fn parse_reserved(t: Span) -> LocToken {
    let string = t.to_string();
    match string.as_ref() {
        "all" => LocToken::new(t, Token::All),
        "any" => LocToken::new(t, Token::Any),
        "as" => LocToken::new(t, Token::As),
        "async" => LocToken::new(t, Token::Async),
        "else" => LocToken::new(t, Token::Else),
        "external" => LocToken::new(t, Token::External),
        "false" => LocToken::new(t, Token::BoolLiteral(false)),
        "filter_map" => LocToken::new(t, Token::FilterMap),
        "filter" => LocToken::new(t, Token::Filter),
        "fn" => LocToken::new(t, Token::Function),
        "foreach" => LocToken::new(t, Token::ForEach),
        "fold" => LocToken::new(t, Token::Fold),
        "if" => LocToken::new(t, Token::If),
        "in" => LocToken::new(t, Token::In),
        "let" => LocToken::new(t, Token::Let),
        "map" => LocToken::new(t, Token::Map),
        "matches" => LocToken::new(t, Token::Matches),
        "Regex" => LocToken::new(t, Token::Regex),
        "return" => LocToken::new(t, Token::Return),
        "Some" => LocToken::new(t, Token::Some),
        "true" => LocToken::new(t, Token::BoolLiteral(true)),
        _ => LocToken::new(t, Token::Ident(string)),
    }
}

fn lex_label_inner<'a, E: error::ParseError<Span<'a>>>(
    input: Span<'a>,
) -> IResult<Span<'a>, LocToken, E> {
    match nom::bytes::complete::take_until("'")(input) {
        Ok((rest, span)) => {
            if let Ok(label) = span.to_string().parse::<labels::Label>() {
                Ok((rest, LocToken::new(span, Token::LabelLiteral(label))))
            } else {
                Err(nom::Err::Incomplete(nom::Needed::Unknown))
            }
        }
        Err(nom::Err::Failure(e)) => Err(nom::Err::Error(e)),
        Err(e) => Err(e),
    }
}

named!(lex_label<Span, LocToken>,
    delimited!(tag!("'"), lex_label_inner, tag!("'"))
);

// Identifiers and reserved words
fn lex_reserved_ident<'a, E: error::ParseError<Span<'a>>>(
    input: Span<'a>,
) -> IResult<Span<'a>, LocToken, E> {
    match nom::bytes::complete::take_while1(|c| char::is_alphanumeric(c) || c == '_')(input) {
        Ok((
            rest,
            LocatedSpan {
                fragment,
                line,
                offset,
            },
        )) => {
            if fragment.is_empty() || fragment.starts_with('_') {
                Err(nom::Err::Incomplete(nom::Needed::Unknown))
            } else {
                Ok((
                    rest,
                    parse_reserved(LocatedSpan {
                        fragment,
                        line,
                        offset,
                    }),
                ))
            }
        }
        Err(nom::Err::Failure(e)) => Err(nom::Err::Error(e)),
        Err(e) => Err(e),
    }
}

// String literals
named!(not_escaped<Span, Span>,
    do_parse!(peek!(not!(one_of!("\\\""))) >> t: take!(1) >> (t))
);

named!(lex_string<Span, LocToken>,
    do_parse!(
        b: alt!(tag!("b\"") | tag!("\"")) >>
        t: opt!(escaped!(call!(not_escaped), '\\', one_of!("\\\""))) >>
        tag!("\"") >>
        (LocToken::new(
            b,
            {
                let s = t.map(|s| s.to_string().replace("\\\"", "\"").replace("\\\\", "\\")).unwrap_or_default();
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
    do_parse!(t: take!(1) >> (LocToken::new(t, Token::Illegal(t.to_string()))))
);

named!(lex_token<Span, LocToken>,
    alt!(
        complete!(lex_number) |
        complete!(lex_string) |
        complete!(lex_label) |
        complete!(lex_reserved_ident) |
        complete!(lex_operator) |
        complete!(lex_illegal)
    )
);

named!(lex_tokens<Span, Vec<LocToken>>, many0!(delimited!(multispace0, lex_token, multispace0)));

pub fn lex(buf: &str) -> Vec<LocToken> {
    let (loc, mut tokens) = lex_tokens(Span::new(buf)).unwrap();
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
