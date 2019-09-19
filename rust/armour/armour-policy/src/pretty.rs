use super::headers::Headers;
use super::lang::{Block, Expr, Program};
use super::literals::Literal;
use super::types::Typ;
use pretty::termcolor::{Color, ColorChoice, ColorSpec, StandardStream};
use pretty::{BoxDoc, Doc};
use std::fmt;

// TODO: regex, brackets (precedence for infix)

impl Expr {
    fn vec_str_to_doc<'a, 'b>(l: &'b [String]) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        if l.len() == 1 {
            Doc::as_string(l.get(0).unwrap())
        } else {
            Doc::text("(")
                .append(
                    Doc::intersperse(
                        l.iter().map(Doc::as_string),
                        Doc::text(",").append(Doc::space()),
                    )
                    .nest(1),
                )
                .append(Doc::text(")"))
                .group()
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
    fn closure_body(self) -> Expr {
        if let Expr::Closure(v, e) = self {
            e.subst(0, &Expr::var(&v.0)).closure_body()
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
    fn key<'a, 'b>(data: &'a str) -> Doc<'b, BoxDoc<'b, ColorSpec>, ColorSpec> {
        enum Class {
            Keyword,
            Control,
            Other,
        }
        let class = match data {
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
                Doc::as_string(data).annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
            }
            Class::Control => {
                Doc::as_string(data).annotate(ColorSpec::new().set_fg(Some(Color::Magenta)).clone())
            }
            Class::Other => Doc::as_string(data),
        }
    }
    fn method<'a, 'b>(name: &'a str) -> Doc<'b, BoxDoc<'b, ColorSpec>, ColorSpec> {
        Doc::as_string(name).annotate(ColorSpec::new().set_fg(Some(Color::Yellow)).clone())
    }
    pub fn owned_to_doc<'a>(self) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        match self {
            Expr::Var(id) => Doc::as_string(id.0),
            Expr::BVar(i) => Doc::text("BVar(").append(Doc::as_string(i)).append(")"),
            Expr::LitExpr(lit) => lit.to_doc(),
            Expr::Let(vs, e, b) => Expr::key("let")
                .append(
                    Doc::space()
                        .append(Expr::vec_str_to_doc(&vs))
                        .append(" =")
                        .nest(2),
                )
                .append(Doc::space().append(e.owned_to_doc()).nest(4))
                .append(Doc::text(";"))
                .append(Doc::newline())
                .append(b.closure_body().owned_to_doc())
                .group(),
            Expr::Closure(v, e) => Doc::text("\\")
                .append(Doc::text(v.0))
                .append(Doc::text("."))
                .append(Doc::space().append(e.owned_to_doc()).nest(2))
                .group(),
            Expr::ReturnExpr(e) => Expr::key("return")
                .append(Doc::space())
                .append(e.owned_to_doc())
                .group(),
            Expr::PrefixExpr(p, e) => Doc::as_string(p).append(e.owned_to_doc()).group(),
            Expr::InfixExpr(op, l, r) => Doc::text("(")
                .append(
                    l.owned_to_doc()
                        .append(Doc::space())
                        .append(Doc::as_string(op))
                        .append(Doc::space())
                        .append(r.owned_to_doc())
                        .nest(1),
                )
                .append(")")
                .group(),
            Expr::Iter(op, vs, e, b) => Doc::as_string(op)
                .annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
                .append(
                    Doc::space()
                        .append(Expr::vec_str_to_doc(&vs))
                        .append(Doc::space())
                        .append(Expr::key("in"))
                        .nest(2),
                )
                .append(Doc::space().append(e.owned_to_doc()).nest(4))
                .append(Doc::space())
                .append("{")
                .append(
                    Doc::newline()
                        .append(b.closure_body().owned_to_doc())
                        .nest(2),
                )
                .append(Doc::newline())
                .append("}")
                .group(),
            Expr::BlockExpr(Block::Block, es) => Doc::intersperse(
                es.into_iter().map(|e| e.owned_to_doc()),
                Doc::text(";").append(Doc::newline()),
            ),
            Expr::BlockExpr(Block::List, es) => Doc::text("[")
                .append(
                    Doc::intersperse(
                        es.into_iter().map(|e| e.owned_to_doc()),
                        Doc::text(",").append(Doc::space()),
                    )
                    .nest(1)
                    .group(),
                )
                .append("]"),
            Expr::BlockExpr(Block::Tuple, es) => Doc::text("(")
                .append(
                    Doc::intersperse(
                        es.into_iter().map(|e| e.owned_to_doc()),
                        Doc::text(",").append(Doc::space()),
                    )
                    .nest(1)
                    .group(),
                )
                .append(")"),
            Expr::IfExpr {
                cond,
                consequence,
                alternative,
            } => {
                let doc = Expr::key("if")
                    .append(Doc::space())
                    .append(cond.owned_to_doc())
                    .append(Doc::space())
                    .append("{")
                    .append(Doc::newline().append(consequence.owned_to_doc()).nest(2))
                    .append(Doc::newline())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(Doc::space())
                        .append(Expr::key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.owned_to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(Doc::newline().append(alt.owned_to_doc()).nest(2))
                            .append(Doc::newline())
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
                let doc = Expr::key("if")
                    .append(" ")
                    .append(Expr::key("let"))
                    .append(" Some(")
                    .append(Doc::as_string(consequence.closure_var().unwrap()))
                    .append(Doc::text(") ="))
                    .append(Doc::space())
                    .append(expr.owned_to_doc())
                    .append(Doc::space())
                    .append("{")
                    .append(
                        Doc::newline()
                            .append(consequence.closure_body().owned_to_doc())
                            .nest(2),
                    )
                    .append(Doc::newline())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(Doc::space())
                        .append(Expr::key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.owned_to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(Doc::newline().append(alt.owned_to_doc()).nest(2))
                            .append(Doc::newline())
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
                let doc = Expr::key("if")
                    .append(
                        Doc::space()
                            .append(Doc::intersperse(
                                matches.into_iter().map(|(e, re)| {
                                    e.owned_to_doc()
                                        .append(Doc::space())
                                        .append(Expr::key("matches"))
                                        .append(Doc::space())
                                        .append(Doc::as_string(re.as_str()).annotate(
                                            ColorSpec::new().set_fg(Some(Color::Red)).clone(),
                                        ))
                                }),
                                Doc::text("&&").append(Doc::space()),
                            ))
                            .nest(2),
                    )
                    .append(Doc::space())
                    .append("{")
                    .append(
                        Doc::newline()
                            .append(consequence.closure_body().owned_to_doc())
                            .nest(2),
                    )
                    .append(Doc::newline())
                    .append("}");
                if let Some(alt) = alternative {
                    let doc = doc
                        .append(Doc::space())
                        .append(Expr::key("else"))
                        .append(" ");
                    if alt.is_if() {
                        doc.append(alt.owned_to_doc()).group()
                    } else {
                        doc.append("{")
                            .append(Doc::newline().append(alt.owned_to_doc()).nest(2))
                            .append(Doc::newline())
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
                let doc = if is_async {
                    Expr::key("async").append(Doc::space())
                } else {
                    Doc::nil()
                };
                if let Some(method) = Headers::method(&function) {
                    let mut args = arguments.into_iter();
                    doc.append(args.next().unwrap().owned_to_doc())
                        .append(".")
                        .append(Expr::method(method))
                        .append(
                            Doc::text("(")
                                .append(
                                    Doc::intersperse(
                                        args.map(|e| e.owned_to_doc()),
                                        Doc::text(",").append(Doc::space()),
                                    )
                                    .nest(1)
                                    .group(),
                                )
                                .append(")"),
                        )
                } else {
                    let f = if function == "option::Some" {
                        Doc::text("Some")
                    } else if let Some((module, name)) = Headers::split(&function) {
                        Doc::as_string(module)
                            .append("::")
                            .append(Expr::method(name))
                    } else {
                        Expr::method(&function)
                    };
                    doc.append(f).append(
                        Doc::text("(")
                            .append(
                                Doc::intersperse(
                                    arguments.into_iter().map(|e| e.owned_to_doc()),
                                    Doc::text(",").append(Doc::space()),
                                )
                                .nest(1)
                                .group(),
                            )
                            .append(")"),
                    )
                }
            }
        }
    }
    pub fn to_doc<'a, 'b>(&'b self) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        self.to_owned().owned_to_doc()
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
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_pretty(80))
    }
}

impl Literal {
    fn literal<'a, 'b>(&'a self) -> Doc<'b, BoxDoc<'b, ColorSpec>, ColorSpec> {
        Doc::as_string(self).annotate(ColorSpec::new().set_fg(Some(Color::Green)).clone())
    }
    fn non_parse_literal<'a, 'b>(&'a self) -> Doc<'b, BoxDoc<'b, ColorSpec>, ColorSpec> {
        Doc::as_string(self).annotate(ColorSpec::new().set_fg(Some(Color::Red)).clone())
    }
    fn to_doc<'a, 'b>(&'b self) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        match self {
            Literal::Bool(b) => Expr::key(&b.to_string()),
            Literal::Data(d) => {
                if std::str::from_utf8(d).is_ok() {
                    self.literal()
                } else {
                    self.non_parse_literal()
                }
            }
            Literal::Regex(r) => Doc::text("Regex(")
                .append(
                    Doc::as_string(r.as_str())
                        .annotate(ColorSpec::new().set_fg(Some(Color::Red)).clone()),
                )
                .append(")"),
            Literal::Unit => Doc::text("()"),
            Literal::List(lits) => Doc::text("[")
                .append(
                    Doc::intersperse(
                        lits.iter().map(|l| l.to_doc()),
                        Doc::text(",").append(Doc::space()),
                    )
                    .nest(1)
                    .group(),
                )
                .append("]"),
            Literal::Tuple(lits) => match lits.len() {
                0 => Doc::text("None"),
                1 => Doc::text("Some(").append(lits[0].to_doc()).append(")"),
                _ => Doc::text("(")
                    .append(
                        Doc::intersperse(
                            lits.iter().map(|l| l.to_doc()),
                            Doc::text(",").append(Doc::space()),
                        )
                        .nest(1)
                        .group(),
                    )
                    .append(")"),
            },
            Literal::HttpRequest(_) | Literal::ID(_) | Literal::IpAddr(_) => {
                self.non_parse_literal()
            }
            _ => self.literal(),
        }
    }
}

impl Typ {
    fn internal<'a>(
        doc: Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec>,
    ) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        doc.annotate(ColorSpec::new().set_fg(Some(Color::Cyan)).clone())
    }
    fn to_doc<'a, 'b>(&'b self) -> Doc<'a, BoxDoc<'a, ColorSpec>, ColorSpec> {
        match self {
            Typ::List(t) => Typ::internal(Doc::text("List"))
                .append("<")
                .append(t.to_doc())
                .append(">"),
            Typ::Tuple(ts) => match ts.len() {
                0 => Typ::internal(Doc::text("Option")).append("<?>"),
                1 => Typ::internal(Doc::text("Option"))
                    .append("<")
                    .append(ts[0].to_doc())
                    .append(">"),
                _ => Doc::text("(")
                    .append(
                        Doc::intersperse(
                            ts.iter().map(|t| t.to_doc()),
                            Doc::text(",").append(Doc::space()),
                        )
                        .nest(1)
                        .group(),
                    )
                    .append(")"),
            },
            _ => Typ::internal(Doc::as_string(self)),
        }
    }
}

impl Program {
    fn decl_to_doc<'a>(
        &self,
        name: &'a str,
        e: &'a Expr,
    ) -> pretty::Doc<'a, pretty::BoxDoc<'a, ColorSpec>, ColorSpec> {
        let mut args = Vec::new();
        e.closure_vars(&mut args);
        let (tys, ty) = self.typ(name).unwrap_or_default().split();
        let tys = tys.unwrap_or_default();
        let ret = if ty == Typ::Unit {
            Doc::nil()
        } else {
            Doc::space()
                .append(Doc::text("->"))
                .append(Doc::space().append(ty.to_doc()).nest(2))
        };
        Expr::key("fn")
            .append(Doc::space())
            .append(Expr::method(name))
            .append(Doc::text("("))
            .append(Doc::intersperse(
                args.into_iter()
                    .zip(tys)
                    .map(|(arg, ty)| Doc::text(arg).append(Doc::text(": ").append(ty.to_doc()))),
                Doc::text(",").append(Doc::space()),
            ))
            .append(Doc::text(")"))
            .append(ret)
            .append(Doc::space())
            .append(Doc::text("{"))
            .append(
                Doc::newline()
                    .append(e.to_owned().closure_body().owned_to_doc())
                    .nest(2),
            )
            .append(Doc::newline())
            .append(Doc::text("}"))
            .group()
    }
    pub fn pretty<'a>(&self, name: &'a str, e: &'a Expr, width: usize) -> String {
        let mut w = Vec::new();
        self.decl_to_doc(name, e).render(width, &mut w).unwrap();
        String::from_utf8(w).unwrap()
    }
    pub fn print(&self) {
        for (name, e) in self.code.0.iter() {
            self.decl_to_doc(name, e)
                .render_colored(80, StandardStream::stdout(ColorChoice::Auto))
                .unwrap();
            println!()
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (name, e) in self.code.0.iter() {
            writeln!(f, "{}", self.pretty(name, e, 80))?;
            writeln!(f)?;
        }
        write!(f, "")
    }
}
