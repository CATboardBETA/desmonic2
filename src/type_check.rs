use crate::parse::{Dot, Elif, Expr, Statement};
use std::collections::HashMap;
use std::mem::discriminant;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex};

pub static BUILTIN_FUNCS: LazyLock<HashMap<String, (Vec<ExprType>, ExprType, String)>> =
    LazyLock::new(|| {
        use ExprType::*;
        macro_rules! i {
        ($h:ident, $($n:expr, $($tys:expr)*, $ty:expr, $al:expr);*) => {
            $($h.insert($n.to_string(), (vec![$($tys),*], $ty, $al.to_string()));)*
        };
    }
        let mut h = HashMap::new();
        i! { h,
            "mod",      Num Num, Num,       "mod";
            "modl",     NumList Num, NumList, "mod";
            "modl2",    Num NumList, NumList, "mod";
            "modll",    NumList NumList, NumList, "mod";
            "sgn",      Num, Num,           "sgn";
            "sign",     Num, Num,           "sgn";
            "signum",   Num, Num,           "sgn";
            "sgnl",     NumList, NumList,   "sgn";
            "signl",    NumList, NumList,   "sgn";
            "signuml",  NumList, NumList,   "sgn";
            "cos",      Num, Num,           "cos";
            "cosl",     NumList, NumList,   "cos";
            "abs",      Num, Num,           "abs";
            "absl",     NumList, NumList,   "abs";
            "abspl",    PointList, Num,     "abs";
            "min",      NumList, Num,       "min";
            "max",      NumList, Num,       "max"
        }
        h
    });

pub static STRUCTS: LazyLock<Mutex<HashMap<String, HashMap<String, ExprType>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

macro_rules! naction {
    ($errs:expr; $expr:expr; $($v:expr),+) => {{
        if $($v == Et::Action ||)* false {
            $errs.push("Expression cannot be an action.".to_string());
            return Et::Conflict;
        }
        return $expr;
    }};
}

#[derive(Debug, Clone)]
pub enum ExprType {
    Conflict,

    Num,
    Action,
    Point,
    Point3,

    NumList,
    PointList,
    Point3List,
    /// If there is exactly one type, it is transparent. If there is more than one type, and all
    /// types are non-list, than it is stored in a list. Otherwise, each element is stored in its
    /// own variable.
    Struct(StructTy),
    StructList(StructTy),
}

impl PartialEq for ExprType {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum StructStorage {
    Unknown,
    OneVar(String),
    // List(String),
    ManyVars(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct StructTy {
    pub(crate) name: Arc<Mutex<String>>,
    pub(crate) index: Arc<Mutex<HashMap<String, usize>>>,
    pub(crate) storage: Arc<Mutex<StructStorage>>,
}

impl PartialEq for StructTy {
    fn eq(&self, other: &Self) -> bool {
        self.name.lock().unwrap().deref() == other.name.lock().unwrap().deref()
            && self.storage.lock().unwrap().deref() == other.storage.lock().unwrap().deref()
    }
}

pub fn check(
    ast: Expr,
    vars: &mut HashMap<String, ExprType>,
    funcs: &mut HashMap<String, (Vec<ExprType>, ExprType)>,
    errs: &mut Vec<String>,
) -> ExprType {
    use ExprType as Et;
    match ast {
        Expr::Num(_) => Et::Num,
        Expr::Neg(a) => {
            let a = check(*a, vars, funcs, errs);
            naction!(errs; a; a);
        }
        Expr::Add(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            naction!(errs; if a == b { a } else { Et::Conflict }; a, b);
        }
        Expr::Sub(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            naction!(errs; if a == b { a } else { Et::Conflict }; a, b)
        }
        Expr::Mul(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            naction!(errs; if a == b { a } else {
                return if a == Et::Num {
                    b
                } else if b == Et::Num{
                    a
                } else {
                    Et::Conflict
                }
            }; a, b)
        }
        Expr::Div(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            naction!(errs; if a == b { a } else { Et::Conflict }; a, b)
        }
        Expr::Exp(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            naction!(errs; if a == b { a } else { Et::Conflict }; a, b)
        }
        Expr::Var(v) => match vars.get(&v) {
            None => {
                errs.push(format!("Variable {v} does not exist"));
                Et::Conflict
            }
            Some(v) => v.clone(),
        },
        Expr::If {
            cmp: _,
            body,
            elifs,
            elsee,
        } => {
            let body = check(*body, vars, funcs, errs);
            let mut extra_tys = vec![];
            for elif in elifs {
                let Elif { cmp: _, body } = elif;
                extra_tys.push(check(*body, vars, funcs, errs));
            }
            if let Some(elsee) = elsee {
                extra_tys.push(check(*elsee, vars, funcs, errs));
            }
            if extra_tys.array_windows().all(|[a, b]| a == b)
                && body == *extra_tys.first().unwrap_or(&body)
            {
                body
            } else {
                errs.push("All `if` bodies must have the same type".to_string());
                Et::Conflict
            }
        }
        Expr::List(a) => {
            let mut tys = vec![];
            for x in a {
                tys.push(check(x, vars, funcs, errs))
            }
            if tys
                .array_windows()
                .into_iter()
                .all(|[a, b]| a.clone() == b.clone() && a.clone() != Et::Action)
            {
                let ty = tys.first().unwrap_or(&Et::Num).clone();
                if ty == Et::NumList
                    || ty == Et::PointList
                    || ty == Et::Point3List
                    || matches!(ty, Et::StructList(..))
                {
                    errs.push("Cannot have a list in a list".to_string());
                    Et::Conflict
                } else {
                    match ty {
                        ExprType::Conflict => {
                            errs.push("Cannot have a type conflict in a list".to_string());
                            Et::Conflict
                        }
                        ExprType::Num => Et::NumList,
                        ExprType::Action => {
                            errs.push("Cannot have a list of actions in a list".to_string());
                            Et::Conflict
                        }
                        ExprType::Point => Et::PointList,
                        ExprType::Point3 => Et::Point3List,
                        ExprType::NumList => {
                            errs.push("Cannot have a list of numbers in a list".to_string());
                            Et::Conflict
                        }
                        ExprType::PointList => {
                            errs.push("Cannot have a list of points in a list".to_string());
                            Et::Conflict
                        }
                        ExprType::Point3List => {
                            errs.push("Cannot have a list of 3D points in a list".to_string());
                            Et::Conflict
                        }
                        ExprType::Struct(StructTy {
                            index,
                            storage,
                            name,
                        }) => Et::StructList(StructTy {
                            storage,
                            name,
                            index,
                        }),
                        ExprType::StructList(..) => {
                            errs.push("Cannot have a list of structs in a list".to_string());
                            Et::Conflict
                        }
                    }
                }
            } else {
                errs.push("All list elements must be of the same type".to_string());

                Et::Conflict
            }
        }
        Expr::Point(a, b) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            if a != b
                || a == Et::Action
                || b == Et::Action
                || a == Et::Point3List
                || b == Et::Point3List
                || a == Et::PointList
                || b == Et::PointList
                || a == Et::Point3
                || b == Et::Point3
                || a == Et::Point
                || b == Et::Point
            {
                errs.push("Cannot store a point in a point".to_string());
                Et::Conflict
            } else if a == Et::NumList {
                Et::PointList
            } else {
                Et::Point
            }
        }
        Expr::Point3(a, b, c) => {
            let a = check(*a, vars, funcs, errs);
            let b = check(*b, vars, funcs, errs);
            let c = check(*c, vars, funcs, errs);
            if (a != b && a != c && b != c)
                || a == Et::Action
                || b == Et::Action
                || c == Et::Action
                || a == Et::Point3List
                || b == Et::Point3List
                || c == Et::Point3List
                || a == Et::PointList
                || b == Et::PointList
                || c == Et::PointList
                || a == Et::Point3
                || b == Et::Point3
                || c == Et::Point3
                || a == Et::Point
                || b == Et::Point
                || c == Et::Point
            {
                errs.push("Cannot store a point in a point".to_string());
                Et::Conflict
            } else if a == Et::NumList {
                Et::Point3List
            } else {
                Et::Point3
            }
        }
        Expr::Call(name, params_found) => {
            let (params, func) = funcs.get(&name).cloned().unwrap_or_else(|| {
                errs.push(format!("Function `{name}` not found"));
                (vec![], Et::Conflict)
            });
            if params.len() == params_found.len() {
                for (i, (par_found, par_real)) in params_found.into_iter().zip(params).enumerate() {
                    let par_found = check(par_found, vars, funcs, errs);
                    if par_found != par_real {
                        errs.push(format!("Param {i} of function {name} should have type {par_real:?}, but has type {par_found:?}"));
                    }
                }
            } else {
                errs.push(format!(
                    "Call to function `{}` should have {} params, got {} params",
                    name,
                    params.len(),
                    params_found.len()
                ))
            }
            func
        }
        Expr::For { over, ident, body } => {
            let over_ty = check(*over, vars, funcs, errs);
            match over_ty {
                ExprType::Conflict => errs.push("Cannot iterate over a type conflict".to_string()),
                ExprType::Num
                | ExprType::Action
                | ExprType::Point
                | ExprType::Point3
                | ExprType::Struct(_) => {
                    errs.push(format!("Cannot iterate over type `{over_ty:?}`"))
                }
                ExprType::NumList
                | ExprType::PointList
                | ExprType::Point3List
                | ExprType::StructList(_) => {}
            }
            let mut vars = HashMap::new();
            vars.insert(ident, over_ty);
            let (rest, last) = body.split_at(body.len() - 1);
            for x in rest {
                let Statement::Def(n, e) = x else {
                    unreachable!()
                };
                let typed = check(e.clone(), &mut vars, funcs, errs);
                vars.insert(n.clone(), typed);
            }
            let Statement::Expr(last) = last[0].clone() else {
                unreachable!()
            };
            check(last, &mut vars, funcs, errs)
        }
        Expr::Abs(e) => {
            let inner_ty = check(*e, vars, funcs, errs);
            match inner_ty {
                ExprType::Conflict => ExprType::Conflict,
                ExprType::Num => ExprType::Num,
                ExprType::Action => ExprType::Conflict,
                ExprType::Point => ExprType::Num,
                ExprType::Point3 => ExprType::Num,
                ExprType::NumList => ExprType::NumList,
                ExprType::PointList => ExprType::NumList,
                ExprType::Point3List => ExprType::NumList,
                ExprType::Struct(_) => ExprType::Conflict,
                ExprType::StructList(_) => ExprType::Conflict,
            }
        }
        Expr::Dot(Dot {
            struct_storage,
            x,
            y,
        }) => {
            let ExprType::Struct(x_ty) = vars[&x].clone() else {
                unreachable!()
            };
            let x_ty_name = x_ty.name.lock().unwrap().clone();
            if let Some(ty) = STRUCTS.lock().unwrap().deref()[&x_ty_name].get(&y) {
                *struct_storage.storage.lock().unwrap() = x_ty.storage.lock().unwrap().clone();
                *struct_storage.name.lock().unwrap() = x_ty_name;
                *struct_storage.index.lock().unwrap() = x_ty.index.lock().unwrap().clone();
                ty.clone()
            } else {
                errs.push(format!(
                    "Struct `{}` (named `{}`) has no field `{}`",
                    struct_storage.name.lock().unwrap(),
                    x,
                    y
                ));
                ExprType::Conflict
            }
        }
    }
}
