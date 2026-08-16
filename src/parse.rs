#![allow(dead_code)]

use crate::type_check::{ExprType, StructTy};
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
    (Vec<Statement>, Option<Statement>),
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
                ComparisonOp::Ge => "\\ge ",
                ComparisonOp::Le => "\\le ",
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
        iters: Vec<(String, Expr)>,
        body: Vec<Statement>,
    },
    List(Vec<Expr>),
    ListR(Box<Expr>, Box<Expr>),
    Point(Box<Expr>, Box<Expr>),
    Point3(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Abs(Box<Expr>),
    Dot(Dot),
    Struct(String, Vec<(String, Expr)>),
    Action(Vec<(String, Expr)>),
    Index(Box<Expr>, Index),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    List(Vec<Expr>),
    Range(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct Dot {
    pub struct_storage: StructTy,
    pub x: String,
    pub y: String,
}

impl PartialEq for Dot {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.struct_storage == other.struct_storage
    }
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
        style: HashMap<Vec<String>, String>,
    },
    Implicit(Expr, ComparisonOp, Expr),
    Struct(String, HashMap<String, ExprType>),
    Ticker(Vec<Expr>),
}

fn bx<T>(x: T) -> Box<T> {
    Box::new(x)
}
fn st<S: ToString>(s: S) -> String {
    s.to_string()
}
