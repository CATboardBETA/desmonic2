#![allow(dead_code)]

use crate::type_check::ExprType;
use lalrpop_util::lalrpop_mod;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

lalrpop_mod!(
    #[rustfmt::skip] #[allow(clippy::all)] #[allow(unused_braces)] #[allow(unused_mut)] grammar
);

//noinspection RsUnresolvedPath
pub fn gen_ast<'s>(
    input: &'s str,
) -> Result<
    Vec<Statement>,
    lalrpop_util::ParseError<usize, lalrpop_util::lexer::Token<'s>, &'static str>,
> {
    grammar::ProgramParser::new().parse(input.as_ref())
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

impl FromStr for ComparisonOp {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ComparisonOp::*;
        // Nested returns truly beautiful
        Ok(match s {
            "==" => Eq,
            "!=" => Ne,
            ">" => Gt,
            "<" => Lt,
            ">=" => Ge,
            "<=" => Le,
            _ => return Err(()),
        })
    }
}

impl ComparisonOp {
    fn from_str_single_eq(s: &str) -> Self {
        use ComparisonOp::*;
        match s {
            "=" => Eq,
            "!=" => Ne,
            ">" => Gt,
            "<" => Lt,
            ">=" => Ge,
            "<=" => Le,
            _ => unreachable!(),
        }
    }
}

impl Display for ComparisonOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ComparisonOp::Eq => "=",
                ComparisonOp::Ne => todo!(),
                ComparisonOp::Gt => ">",
                ComparisonOp::Lt => "<",
                ComparisonOp::Ge => "\\ge",
                ComparisonOp::Le => "\\le",
            }
        )
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Num(f32),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Exp(Box<Expr>, Box<Expr>),
    Var(String),
    If {
        cmp: Comparison,
        body: Box<Expr>,
        elifs: Vec<Elif>,
        elsee: Option<Box<Expr>>,
    },
    For {
        over: Box<Expr>,
        ident: String,
        body: Vec<Statement>,
    },
    List(Vec<Expr>),
    Point(Box<Expr>, Box<Expr>),
    Point3(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Abs(Box<Expr>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Comparison(
    pub Box<Expr>,
    pub ComparisonOp,
    pub Box<Expr>,
    pub Option<(ComparisonOp, Box<Expr>)>,
);

#[derive(Debug, PartialEq, Clone)]
pub struct Elif {
    pub cmp: Comparison,
    pub body: Box<Expr>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Expr(Expr),
    Def(String, Expr),
    Fn {
        name: String,
        params: Vec<(String, ExprType)>,
        body: Vec<Statement>,
    },
    Styled {
        stmts: Vec<Statement>,
        style: HashMap<String, String>,
    },
    Implicit(Expr, ComparisonOp, Expr),
}

fn bx<T>(x: T) -> Box<T> {
    Box::new(x)
}
fn st<S: ToString>(s: S) -> String {
    s.to_string()
}

//noinspection RsUnresolvedPath
#[cfg(test)]
mod test {
    use super::*;
    use grammar::ProgramParser as PP;

    #[test]
    fn empty() {
        assert_eq!(PP::new().parse(""), Ok(vec![]));
        assert_eq!(PP::new().parse("  "), Ok(vec![]));
        assert_eq!(PP::new().parse("\t"), Ok(vec![]));
        assert_eq!(PP::new().parse("\n"), Ok(vec![]));
        assert_eq!(PP::new().parse(" \t\n\r"), Ok(vec![]));
    }

    #[test]
    fn precedence() {
        use Expr::*;
        use Statement as St;
        assert_eq!(
            PP::new().parse("0+1*2;"),
            Ok(vec![St::Expr(Add(
                bx(Num(0.)),
                bx(Mul(bx(Num(1.)), bx(Num(2.))))
            ))])
        );
        assert_eq!(
            PP::new().parse("(0+1)*2;"),
            Ok(vec![St::Expr(Mul(
                bx(Add(bx(Num(0.)), bx(Num(1.)))),
                bx(Num(2.))
            ))])
        );
        // TODO: add more precedence tests
    }
    #[test]
    fn associativity() {
        use Expr::*;
        use Statement as St;
        // Right-assoc
        assert_eq!(
            PP::new().parse("x^x^x;"),
            Ok(vec![St::Expr(Exp(
                bx(Var(st("x"))),
                bx(Exp(bx(Var(st("x"))), bx(Var(st("x")))))
            ))])
        );

        // Left-assoc
        assert_eq!(
            PP::new().parse("1*1*1;"),
            Ok(vec![St::Expr(Mul(
                bx(Mul(bx(Num(1.)), bx(Num(1.)))),
                bx(Num(1.))
            ))])
        );
    }

    #[test]
    fn var_def() {
        use Expr::*;
        use Statement as St;
        assert_eq!(
            PP::new().parse("let x1=26.667;"),
            Ok(vec![St::Def(st("x1"), Num(26.667))])
        );
        assert_eq!(
            PP::new().parse("let x1=x^2-1.5;"),
            Ok(vec![St::Def(
                st("x1"),
                Sub(bx(Exp(bx(Var(st("x"))), bx(Num(2.)))), bx(Num(1.5)))
            )])
        );
    }

    #[test]
    fn if_simple() {
        use ComparisonOp as Co;
        use Expr::*;
        use Statement as St;
        assert_eq!(
            PP::new().parse("if 0<1 { 1 };"),
            Ok(vec![St::Expr(If {
                cmp: Comparison(bx(Num(0.)), Co::Lt, bx(Num(1.)), None),
                body: bx(Num(1.)),
                elifs: vec![],
                elsee: None,
            })])
        )
    }

    #[test]
    fn if_else() {
        use ComparisonOp as Co;
        use Expr::*;
        use Statement as St;
        assert_eq!(
            PP::new().parse("if -1>=11.275 { 1 } else { -1 };"),
            Ok(vec![St::Expr(If {
                cmp: Comparison(bx(Neg(bx(Num(1.)))), Co::Ge, bx(Num(11.275)), None),
                body: bx(Num(1.)),
                elifs: vec![],
                elsee: Some(bx(Neg(bx(Num(1.))))),
            })])
        )
    }
    #[test]
    fn if_elif() {
        use ComparisonOp as Co;
        use Expr::*;
        use Statement as St;
        assert_eq!(
            PP::new().parse("if -1 >= 11.275 { 1 } elif 2 == 2 < 3 { 2 };"),
            Ok(vec![St::Expr(If {
                cmp: Comparison(bx(Neg(bx(Num(1.)))), Co::Ge, bx(Num(11.275)), None),
                body: bx(Num(1.)),
                elifs: vec![Elif {
                    cmp: Comparison(
                        bx(Num(2.)),
                        Co::Eq,
                        bx(Num(2.)),
                        Some((Co::Lt, bx(Num(3.))))
                    ),
                    body: bx(Num(2.)),
                }],
                elsee: None,
            })])
        )
    }
}
