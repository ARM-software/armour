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

use super::expressions::{Block, Expr, Pattern};
use super::headers::{Headers, THeaders};
use super::lang::Program;
use super::literals::{Literal, CPFlatLiteral, DPFlatLiteral, TFlatLiteral};
use super::parser::{As, Assoc, Infix, Pat, PolicyRegex, Precedence};
use super::types::{Typ, TFlatTyp, TTyp};
use pretty::termcolor::{Color, ColorChoice, ColorSpec, StandardStream};
use pretty::RcDoc;
use std::fmt;

fn bracket(doc: RcDoc<'_, ColorSpec>) -> RcDoc<'_, ColorSpec> {
    RcDoc::text("(").append(doc.nest(1)).append(")")
}


pub trait TPrettyLit : std::fmt::Display {
    fn literal<'a, 'b>(&'a self) -> RcDoc<'b, ColorSpec> {
        RcDoc::as_string(self).annotate(ColorSpec::new().set_fg(Some(Color::Green)).clone())
    }

    fn non_parse_literal<'a, 'b>(&'a self) -> RcDoc<'b, ColorSpec> {
        RcDoc::as_string(self).annotate(ColorSpec::new().set_fg(Some(Color::Blue)).clone())
    }

    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec>;
}

fn key<'a, 'b>(data: &'a str) -> RcDoc<'b, ColorSpec> {
    enum Class {
        Keyword,
        Control,
        Other,
    }
    let class = match data {
        "as" => Class::Keyword,
        "async" => Class::Control,
        "else" => Class::Control,
        "false" => Class::Keyword,
        "fn" => Class::Keyword,
        "if" => Class::Control,
        "in" => Class::Keyword,
        "let" => Class::Keyword,
        "matches" => Class::Control,
        "return" => Class::Control,
        "true" => Class::Keyword,
        _ => Class::Other,
    };
    match class {
        Class::Keyword => {
            RcDoc::as_string(data).annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
        }
        Class::Control => RcDoc::as_string(data)
            .annotate(ColorSpec::new().set_fg(Some(Color::Magenta)).clone()),
        Class::Other => RcDoc::as_string(data),
    }
}

impl<FlatTyp, FlatLiteral> Expr<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn vec_str_to_doc<'a, 'b>(l: &'b [String]) -> RcDoc<'a, ColorSpec> {
        if l.len() == 1 {
            RcDoc::as_string(l.get(0).unwrap())
        } else {
            bracket(
                RcDoc::intersperse(
                    l.iter().map(RcDoc::as_string),
                    RcDoc::text(",").append(RcDoc::space()),
                )
                .group(),
            )
        }
    }
    fn closure_var(&self) -> Option<&str> {
        if let Expr::Closure(v, _) = self {
            Some(&v.0)
        } else {
            None
        }
    }
    fn closure_vars<'a>(&'a self, res: &mut Vec<&'a str>) {
        if let Expr::Closure(v, e) = self {
            res.push(&v.0);
            Expr::closure_vars(e, res)
        }
    }
    fn closure_body(&self) -> &Self {
        if let Expr::Closure(_, e) = self {
            e.closure_body()
        } else {
            self
        }
    }
    fn is_if(&self) -> bool {
        match self {
            Expr::IfExpr { .. } | Expr::IfMatchExpr { .. } | Expr::IfSomeMatchExpr { .. } => true,
            _ => false,
        }
    }
    fn method<'a, 'b>(name: &'a str) -> RcDoc<'b, ColorSpec> {
        RcDoc::as_string(name).annotate(ColorSpec::new().set_fg(Some(Color::Yellow)).clone())
    }
    fn precedence(&self) -> (Precedence, Assoc) {
        if let Expr::InfixExpr(op, _, _) = self {
            Infix::precedence(op)
        } else {
            (Precedence::PDot, Assoc::Left)
        }
    }
    pub fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            Expr::Var(id) => RcDoc::as_string(id.0.clone()),
            Expr::BVar(id, _) => RcDoc::as_string(id.0.clone()),
            Expr::LitExpr(lit) => lit.to_doc(),
            Expr::Let(vs, e, b) => key("let")
                .append(
                    RcDoc::space()
                        .append(Self::vec_str_to_doc(&vs))
                        .append(" =")
                        .nest(2),
                )
                .append(RcDoc::space().append(e.to_doc()).nest(4))
                .append(RcDoc::text(";"))
                .append(RcDoc::line())
                .append(b.closure_body().to_doc())
                .group(),
            Expr::Closure(v, e) => RcDoc::text("\\")
                .append(RcDoc::text(v.0.clone()))
                .append(RcDoc::text("."))
                .append(RcDoc::space().append(e.to_doc()).nest(2))
                .group(),
            Expr::ReturnExpr(e) => key("return")
                .append(RcDoc::space())
                .append(e.to_doc())
                .group(),
            Expr::PrefixExpr(p, e) => RcDoc::as_string(p).append(e.to_doc()).group(),
            Expr::InfixExpr(op, l, r) => {
                let left = l.precedence();
                let right = r.precedence();
                let own = self.precedence();
                let left = if left.0 < own.0 || left == own && own.1 == Assoc::Right {
                    bracket(l.to_doc())
                } else {
                    l.to_doc()
                };
                let right = if right.0 < own.0 || right == own && own.1 == Assoc::Left {
                    bracket(r.to_doc())
                } else {
                    r.to_doc()
                };
                left.append(RcDoc::space())
                    .append(RcDoc::as_string(op))
                    .append(RcDoc::space())
                    .append(right)
                    .group()
            }
            Expr::Iter(op, vs, e, b, acc_opt) => {
                let tmp = RcDoc::as_string(op)
                    .annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
                    .append(
                        RcDoc::space()
                            .append(Self::vec_str_to_doc(&vs))
                            .append(RcDoc::space())
                            .append(key("in"))
                            .nest(2),
                    )
                    .append(RcDoc::space().append(e.to_doc()).nest(4))
                    .append(RcDoc::space())
                    .append("{")
                    .append(RcDoc::line().append(b.closure_body().to_doc()).nest(2))
                    .append(RcDoc::line())
                    .append("}");

                match acc_opt {
                    Some((_, acc)) =>tmp.append(RcDoc::space().append(acc.to_doc()).nest(4)) ,
                    None =>tmp 
                }.group()
            },
            Expr::BlockExpr(Block::Block, es) => RcDoc::intersperse(
                es.iter().map(|e| e.to_doc()),
                RcDoc::text(";").append(RcDoc::line()),
            ),
            Expr::BlockExpr(Block::List, es) => RcDoc::text("[")
                .append(
                    RcDoc::intersperse(
                        es.iter().map(|e| e.to_doc()),
                        RcDoc::text(",").append(RcDoc::space()),
                    )
                    .nest(1)
                    .group(),
                )
                .append("]"),
            Expr::BlockExpr(Block::Tuple, es) => bracket(
                RcDoc::intersperse(
                    es.iter().map(|e| e.to_doc()),
                    RcDoc::text(",").append(RcDoc::space()),
                )
                .group(),
            ),
            Expr::IfExpr {
                cond,
                consequence,
                alternative,
            } => {
                let doc = key("if")
                    .append(RcDoc::space())
                    .append(cond.to_doc())
                    .append(RcDoc::space())
                    .append("{")
                    .append(RcDoc::line().append(consequence.to_doc()).nest(2))
                    .append(RcDoc::line())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(RcDoc::space())
                        .append(key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(RcDoc::line().append(alt.to_doc()).nest(2))
                            .append(RcDoc::line())
                            .append("}")
                            .group()
                    }
                } else {
                    doc.group()
                }
            }
            Expr::IfSomeMatchExpr {
                expr,
                consequence,
                alternative,
            } => {
                let doc = key("if")
                    .append(" ")
                    .append(key("let"))
                    .append(" Some(")
                    .append(RcDoc::as_string(consequence.closure_var().unwrap()))
                    .append(RcDoc::text(") ="))
                    .append(RcDoc::space())
                    .append(expr.to_doc())
                    .append(RcDoc::space())
                    .append("{")
                    .append(
                        RcDoc::line()
                            .append(consequence.closure_body().to_doc())
                            .nest(2),
                    )
                    .append(RcDoc::line())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(RcDoc::space())
                        .append(key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(RcDoc::line().append(alt.to_doc()).nest(2))
                            .append(RcDoc::line())
                            .append("}")
                            .group()
                    }
                } else {
                    doc.group()
                }
            }
            Expr::IfMatchExpr {
                matches,
                consequence,
                alternative,
                ..
            } => {
                let doc = key("if")
                    .append(
                        RcDoc::space()
                            .append(RcDoc::intersperse(
                                matches.iter().map(|(e, re)| {
                                    e.to_doc()
                                        .append(RcDoc::space())
                                        .append(key("matches"))
                                        .append(RcDoc::space())
                                        .append(re.to_doc())
                                }),
                                RcDoc::space().append("&&").append(RcDoc::space()),
                            ))
                            .nest(2),
                    )
                    .append(RcDoc::space())
                    .append("{")
                    .append(
                        RcDoc::line()
                            .append(consequence.closure_body().to_doc())
                            .nest(2),
                    )
                    .append(RcDoc::line())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(RcDoc::space())
                        .append(key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(RcDoc::line().append(alt.to_doc()).nest(2))
                            .append(RcDoc::line())
                            .append("}")
                            .group()
                    }
                } else {
                    doc.group()
                }
            }
            Expr::CallExpr {
                function,
                arguments,
                is_async,
            } => {
                let doc = if *is_async {
                    key("async").append(RcDoc::space())
                } else {
                    RcDoc::nil()
                };
                if let Some(method) = Headers::<FlatTyp>::method(&function) {
                    let mut args = arguments.iter();
                    doc.append(args.next().unwrap().to_doc())
                        .append(".")
                        .append(Self::method(method))
                        .append(bracket(
                            RcDoc::intersperse(
                                args.map(|e| e.to_doc()),
                                RcDoc::text(",").append(RcDoc::space()),
                            )
                            .group(),
                        ))
                } else {
                    let f = if function == "option::Some" {
                        RcDoc::text("Some")
                    } else if let Some((module, name)) = Headers::<FlatTyp>::split(&function) {
                        RcDoc::as_string(module)
                            .append("::")
                            .append(Self::method(name))
                    } else {
                        Self::method(&function)
                    };
                    doc.append(f).append(bracket(
                        RcDoc::intersperse(
                            arguments.iter().map(|e| e.to_doc()),
                            RcDoc::text(",").append(RcDoc::space()),
                        )
                        .group(),
                    ))
                }
            },
            Expr::Phantom(_) => unreachable!()
        }
    }
    pub fn to_pretty(&self, width: usize) -> String {
        let mut w = Vec::new();
        self.to_doc().render(width, &mut w).unwrap();
        String::from_utf8(w).unwrap()
    }
    pub fn print(&self) {
        self.to_doc()
            .render_colored(80, StandardStream::stdout(ColorChoice::Auto))
            .unwrap()
    }
    pub fn print_debug(&self) {
        println!("{:#?}", self);
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> fmt::Display for Expr<FlatTyp, FlatLiteral> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_pretty(80))
    }
}

impl<FlatTyp:TFlatTyp> Infix<FlatTyp> {
    fn precedence(&self) -> (Precedence, Assoc) {
        match self {
            Infix::Equal => (Precedence::PEquals, Assoc::Right),
            Infix::NotEqual => (Precedence::PEquals, Assoc::Right),
            Infix::Plus => (Precedence::PSum, Assoc::Left),
            Infix::Minus => (Precedence::PSum, Assoc::Left),
            Infix::Divide => (Precedence::PProduct, Assoc::Left),
            Infix::Multiply => (Precedence::PProduct, Assoc::Left),
            Infix::Remainder => (Precedence::PProduct, Assoc::Left),
            Infix::GreaterThanEqual => (Precedence::PLessGreater, Assoc::Right),
            Infix::LessThanEqual => (Precedence::PLessGreater, Assoc::Right),
            Infix::GreaterThan => (Precedence::PLessGreater, Assoc::Right),
            Infix::LessThan => (Precedence::PLessGreater, Assoc::Right),
            Infix::And => (Precedence::PAnd, Assoc::Right),
            Infix::Or => (Precedence::POr, Assoc::Right),
            Infix::Concat => (Precedence::PSum, Assoc::Right),
            Infix::ConcatStr => (Precedence::PSum, Assoc::Right),
            Infix::Module => (Precedence::PModule, Assoc::Right),
            Infix::In => (Precedence::PIn, Assoc::Left),
            Infix::Dot => (Precedence::PDot, Assoc::Left),
            Infix::Phantom(_) => unreachable!()
        }
    }
}

impl Pat {
    fn is_alt(&self) -> bool {
        if let Pat::Alt(_) = self {
            true
        } else {
            false
        }
    }
    fn is_alt_or_seq(&self) -> bool {
        match self {
            Pat::Alt(_) | Pat::Seq(_) => true,
            _ => false,
        }
    }
    fn postfix<'a>(&self, s: &'static str) -> RcDoc<'a, ColorSpec> {
        (if self.is_alt_or_seq() {
            bracket(self.to_doc())
        } else {
            self.to_doc()
        })
        .append(s)
    }
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            Pat::Any => RcDoc::text("."),
            Pat::Lit(s) => RcDoc::text("\"")
                .append(RcDoc::as_string(s))
                .append("\"")
                .annotate(ColorSpec::new().set_fg(Some(Color::Green)).clone()),
            Pat::Class(s) => RcDoc::as_string(format!(":{}:", s))
                .annotate(ColorSpec::new().set_fg(Some(Color::Green)).clone()),
            Pat::Alt(ps) => RcDoc::intersperse(
                ps.iter().map(|p| p.to_doc()),
                RcDoc::space()
                    .append(RcDoc::text("|"))
                    .append(RcDoc::space()),
            )
            .group(),
            Pat::Seq(ps) => RcDoc::intersperse(
                ps.iter().map(|p| {
                    if p.is_alt() {
                        bracket(p.to_doc())
                    } else {
                        p.to_doc()
                    }
                }),
                RcDoc::space(),
            )
            .group(),
            Pat::As(id, As::Str) => RcDoc::text("[")
                .append(RcDoc::as_string(id.0.clone()))
                .append("]"),
            Pat::As(id, As::I64) => RcDoc::text("[")
                .append(RcDoc::as_string(id.0.clone()))
                .append(" ")
                .append(key("as"))
                .append(" i64]"),
            Pat::As(id, As::Base64) => RcDoc::text("[")
                .append(RcDoc::as_string(id.0.clone()))
                .append(" ")
                .append(key("as"))
                .append(" base64]"),
            Pat::Opt(p) => p.postfix("?"),
            Pat::Star(p) => p.postfix("*"),
            Pat::Plus(p) => p.postfix("+"),
            Pat::CaseInsensitive(p) => p.postfix("!"),
            Pat::IgnoreWhitespace(p) => p.postfix("%"),
        }
    }
}

impl PolicyRegex {
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        self.0.to_doc()
    }
    fn to_pretty(&self, width: usize) -> String {
        let mut w = Vec::new();
        self.to_doc().render(width, &mut w).unwrap();
        String::from_utf8(w).unwrap()
    }
}

impl fmt::Display for PolicyRegex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_pretty(80))
    }
}

impl Pattern {
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            Pattern::Regex(r) => r.to_doc(),
            Pattern::Label(l) => RcDoc::text(l.to_string()),
        }
    }
}

impl<FlatTyp, FlatLiteral> TPrettyLit for  Literal<FlatTyp, FlatLiteral>
where 
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp> 
{
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            Literal::FlatLiteral(fl) => fl.to_doc(),
            Literal::List(lits) => RcDoc::text("[")
                .append(
                    RcDoc::intersperse(
                        lits.iter().map(|l| l.to_doc()),
                        RcDoc::text(",").append(RcDoc::space()),
                    )
                    .nest(1)
                    .group(),
                )
                .append("]"),
            Literal::Tuple(lits) => match lits.len() {
                0 => RcDoc::text("None"),
                1 => RcDoc::text("Some(").append(lits[0].to_doc()).append(")"),
                _ => bracket(
                    RcDoc::intersperse(
                        lits.iter().map(|l| l.to_doc()),
                        RcDoc::text(",").append(RcDoc::space()),
                    )
                    .group(),
                ),
            },
            Literal::Phantom(_) => unreachable!()
        }
    }
}

impl TPrettyLit for DPFlatLiteral{
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            dpflatlit!(Bool(b)) => key(&b.to_string()),
            dpflatlit!(Data(d)) => {
                if std::str::from_utf8(d).is_ok() {
                    self.literal()
                } else {
                    self.non_parse_literal()
                }
            }
            dpflatlit!(Regex(r)) => RcDoc::text("Regex(").append(r.to_doc()).append(")"),
            DPFlatLiteral::Unit => RcDoc::text("()"),
            dpflatlit!(HttpRequest(_))
            | dpflatlit!(ID(_))
            | dpflatlit!(Connection(_))
            | dpflatlit!(IpAddr(_)) => self.non_parse_literal(),
            _ => self.literal(),
        }
    }
}
impl TPrettyLit for CPFlatLiteral{
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            cpdpflatlit!(Bool(b)) => key(&b.to_string()),
            cpdpflatlit!(Data(d)) => {
                if std::str::from_utf8(d).is_ok() {
                    self.literal()
                } else {
                    self.non_parse_literal()
                }
            }
            cpdpflatlit!(Regex(r)) => RcDoc::text("Regex(").append(r.to_doc()).append(")"),
            CPFlatLiteral::DPFlatLiteral(DPFlatLiteral::Unit) => RcDoc::text("()"),
            cpdpflatlit!(HttpRequest(_))
            | cpdpflatlit!(ID(_))
            | cpdpflatlit!(Connection(_))
            | cpdpflatlit!(IpAddr(_)) => self.non_parse_literal(),
            _ => self.literal(),
        }
    }
}

impl<FlatTyp:TFlatTyp> Typ<FlatTyp> {
    fn internal(doc: RcDoc<'_, ColorSpec>) -> RcDoc<'_, ColorSpec> {
        doc.annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
    }
    fn to_doc<'a>(&self) -> RcDoc<'a, ColorSpec> {
        match self {
            Typ::List(t) => <Typ<FlatTyp>>::internal(RcDoc::text("List"))
                .append("<")
                .append(t.to_doc())
                .append(">"),
            Typ::Tuple(ts) => match ts.len() {
                0 => <Typ<FlatTyp>>::internal(RcDoc::text("Option")).append("<?>"),
                1 => <Typ<FlatTyp>>::internal(RcDoc::text("Option"))
                    .append("<")
                    .append(ts[0].to_doc())
                    .append(">"),
                _ => bracket(
                    RcDoc::intersperse(
                        ts.iter().map(|t| t.to_doc()),
                        RcDoc::text(",").append(RcDoc::space()),
                    )
                    .group(),
                ),
            },
            _ => <Typ<FlatTyp>>::internal(RcDoc::as_string(self)),
        }
    }
}

impl<FlatTyp, FlatLiteral> Program<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn decl_to_doc<'a>(&self, name: &'a str, e: &'a Expr<FlatTyp, FlatLiteral>) -> RcDoc<'a, ColorSpec> {
        let mut args = Vec::new();
        e.closure_vars(&mut args);
        let (tys, ty) = self.typ(name).unwrap_or_default().split();
        let tys = tys.unwrap_or_default();
        let ret = if ty == Typ::unit() {
            RcDoc::nil()
        } else {
            RcDoc::space()
                .append(RcDoc::text("->"))
                .append(RcDoc::space().append(ty.to_doc()).nest(2))
        };
        key("fn")
            .append(RcDoc::space())
            .append(<Expr<FlatTyp, FlatLiteral>>::method(name))
            .append(RcDoc::text("("))
            .append(RcDoc::intersperse(
                args.into_iter().zip(tys).map(|(arg, ty)| {
                    RcDoc::text(arg).append(RcDoc::text(": ").append(ty.to_doc()))
                }),
                RcDoc::text(",").append(RcDoc::space()),
            ))
            .append(RcDoc::text(")"))
            .append(ret)
            .append(RcDoc::space())
            .append(RcDoc::text("{"))
            .append(
                RcDoc::line()
                    .append(e.to_owned().closure_body().to_doc())
                    .nest(2),
            )
            .append(RcDoc::line())
            .append(RcDoc::text("}"))
            .group()
    }
    pub fn pretty<'a>(&self, name: &'a str, e: &'a Expr<FlatTyp, FlatLiteral>, width: usize) -> String {
        let mut w = Vec::new();
        self.decl_to_doc(name, e).render(width, &mut w).unwrap();
        String::from_utf8(w).unwrap()
    }
    pub fn print(&self) {
        // println!("protocol: {}", self.protocol());
        for (name, e) in self.code.0.iter() {
            self.decl_to_doc(name, e)
                .render_colored(80, StandardStream::stdout(ColorChoice::Auto))
                .unwrap();
            println!()
        }
    }
}

impl<FlatTyp , FlatLiteral> fmt::Display for Program<FlatTyp, FlatLiteral> 
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (name, e) in self.code.0.iter() {
            writeln!(f, "{}", self.pretty(name, e, 80))?;
        }
        Ok(())
    }
}
