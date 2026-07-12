use crate::parse::{Comparison, Dot, Expr, Statement};
use crate::type_check::{BUILTIN_FUNCS, ExprType, STRUCTS, StructStorage};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};
use convert_case::ccase;

static FOR_ID: AtomicI32 = AtomicI32::new(0);

/// This really should be passed as an argument, but there's already so many on `tr`...
static ERRS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Debug)]
pub struct DesmoExpr {
    pub id: i32,
    pub folder_id: Option<i32>,
    pub content: String,
    pub other: HashMap<String, Value>,
}

pub fn transpile_many(
    stmts: Vec<Statement>,
    fn_name: Option<(&String, &Vec<(String, ExprType)>, &Vec<String>)>,
    init_id: i32,
    fold_id: Option<i32>,
) -> Vec<DesmoExpr> {
    let mut exprs = vec![];
    let mut id = init_id;
    for stmt in stmts {
        match stmt {
            Statement::Expr(e) => {
                let e = tr(&e, fn_name, &mut exprs, &mut (id, fold_id));
                exprs.push(DesmoExpr {
                    id,
                    folder_id: fold_id,
                    content: e,
                    other: hm(),
                });
                id += 1;
            }
            Statement::Def(n, e) => {
                let n = {
                    let replaced = n.replace('_', "");
                    let replaced = if let Some((fn_name, _params, rename_only)) = fn_name
                        && (rename_only.contains(&n) || rename_only.is_empty())
                    {
                        format!("{fn_name}{replaced}")
                    } else {
                        replaced
                    };
                    let (first, rest) = replaced.split_at(1);
                    let rest = if rest.is_empty() {
                        String::new()
                    } else {
                        format!("_{{{rest}}}")
                    };
                    format!("{first}{rest}")
                };
                let val = format!("{}={}", n, tr(&e, fn_name, &mut exprs, &mut (id, fold_id)));
                exprs.push(DesmoExpr {
                    id,
                    folder_id: fold_id,
                    content: val,
                    other: hm(),
                });
                id += 1;
            }
            Statement::Fn {
                ref name,
                ref params,
                body,
            } => {
                let Some((last, body)) = body.split_last() else {
                    unreachable!()
                };
                let f_id = id;
                let mut other = hm();
                other.insert("collapsed".to_string(), Value::Bool(true));
                exprs.push(DesmoExpr {
                    id: f_id,
                    folder_id: None,
                    content: format!("\\folder Function `{name}`"),
                    other,
                });
                id += 1;
                let mut other = hm();
                other.insert("hidden".to_string(), Value::Bool(true));

                exprs.append(
                    &mut transpile_many(body.into(), Some((name, params, &vec![])), id, Some(f_id))
                        .into_iter()
                        .map(|mut e| {
                            e.other = other.clone();
                            e
                        })
                        .collect(),
                );
                id += 1;
                let Statement::Expr(e) = last else {
                    unreachable!()
                };
                let params_fmt = params
                    .iter()
                    .map(|s| {
                        let replaced = s.0.replace('_', "");
                        let (first, rest) = replaced.split_at(1);
                        let rest = if rest.is_empty() {
                            String::new()
                        } else {
                            format!("_{{{rest}}}")
                        };
                        format!("{first}{rest}")
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let name_fmt = {
                    let replaced = name.replace('_', "");
                    let (first, rest) = replaced.split_at(1);
                    let rest = if rest.is_empty() {
                        String::new()
                    } else {
                        format!("_{{{rest}}}")
                    };
                    format!("{first}{rest}")
                };
                let val = tr(
                    e,
                    Some((name, params, &vec![])),
                    &mut exprs,
                    &mut (id, fold_id),
                );
                exprs.push(DesmoExpr {
                    id,
                    folder_id: Some(f_id),
                    content: format!("{}\\left({}\\right)={}", name_fmt, params_fmt, val),
                    other,
                })
            }
            Statement::Styled { stmts, style } => {
                let new = transpile_many(stmts, fn_name, id, fold_id);
                exprs.extend(new.into_iter().map(|mut x| {
                    x.other.extend(
                        style
                            .iter()
                            .map(|(k, v)| (k.clone(), Value::String(v.clone()))),
                    );
                    x
                }))
            }
            Statement::Implicit(ref lhs, cmp, ref rhs) => {
                let lhs = tr(lhs, fn_name, &mut exprs, &mut (id, fold_id));

                let rhs = tr(rhs, fn_name, &mut exprs, &mut (id, fold_id));
                exprs.push(DesmoExpr {
                    id,
                    folder_id: fold_id,
                    content: format!("{}{}{}", lhs, cmp, rhs),
                    other: hm(),
                })
            }
            Statement::Struct(_, _) => {
                // Struct definitions don't transpile into anything. They are only used by the Desmonic
                // transpiler.
            }
        }
    }
    exprs
}

fn tr(
    e: &Expr,
    fn_name: Option<(&String, &Vec<(String, ExprType)>, &Vec<String>)>,
    exprs: &mut Vec<DesmoExpr>,
    ids: &mut (i32, Option<i32>),
) -> String {
    match e {
        Expr::Num(n) => n.to_string(),
        Expr::Neg(x) => format!("-{}", tr(x, fn_name, exprs, ids)),
        Expr::Add(a, b) => format!(
            "{}+{}",
            tr(a, fn_name, exprs, ids),
            tr(b, fn_name, exprs, ids)
        ),
        Expr::Sub(a, b) => format!(
            "{}-{}",
            tr(a, fn_name, exprs, ids),
            tr(b, fn_name, exprs, ids)
        ),
        Expr::Mul(a, b) => format!(
            "\\left({}\\right)\\left({}\\right)",
            tr(a, fn_name, exprs, ids),
            tr(b, fn_name, exprs, ids)
        ),
        Expr::Div(n, d) => format!(
            "\\frac{{{}}}{{{}}}",
            tr(n, fn_name, exprs, ids),
            tr(d, fn_name, exprs, ids)
        ),
        Expr::Exp(b, e) => format!(
            "\\left({}\\right)^{{{}}}",
            tr(b, fn_name, exprs, ids),
            tr(e, fn_name, exprs, ids)
        ),
        Expr::Var(s) => {
            let replaced = s.replace('_', "");
            let replaced = if let Some((fn_name, params, rename_only)) = fn_name
                && params.iter().all(|x| x.0 != *s)
                && (rename_only.contains(s) || rename_only.is_empty())
            {
                format!("{fn_name}{replaced}")
            } else {
                replaced
            };
            let (first, rest) = replaced.split_at(1);
            let rest = if rest.is_empty() {
                String::new()
            } else {
                format!("_{{{rest}}}")
            };

            format!("{first}{rest}")
        }
        Expr::If {
            cmp,
            body,
            elifs,
            elsee,
        } => {
            let cmp_match = |Comparison(lhs, c1, mhs, rhs): &Comparison,
                             exprs: &mut Vec<DesmoExpr>,
                             ids: &mut (i32, Option<i32>)|
             -> String {
                let rhs = if let Some((c2, rhs)) = rhs {
                    format!("{}{}", c2, tr(rhs, fn_name, exprs, ids))
                } else {
                    String::new()
                };
                format!(
                    "{}{}{}{}",
                    tr(lhs, fn_name, exprs, ids),
                    c1,
                    tr(mhs, fn_name, exprs, ids),
                    rhs
                )
            };
            let elifs = elifs
                .iter()
                .map(|el| {
                    format!(
                        ",{}:{}",
                        cmp_match(&el.cmp, exprs, ids),
                        tr(&el.body, fn_name, exprs, ids)
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            let elsee = elsee.as_ref().map_or_else(String::new, |body| {
                format!(",{}", tr(body, fn_name, exprs, ids))
            });
            format!(
                "\\left\\{{{}:{}{}{}\\right\\}}",
                cmp_match(cmp, exprs, ids),
                tr(body, fn_name, exprs, ids),
                elifs,
                elsee
            )
        }
        Expr::List(l) => {
            let inner = l
                .iter()
                .map(|x| tr(x, fn_name, exprs, ids))
                .collect::<Vec<_>>()
                .join(",");
            format!("\\left[{inner}\\right]")
        }
        Expr::Point(x, y) => {
            format!(
                "\\left({},{}\\right)",
                tr(x, fn_name, exprs, ids),
                tr(y, fn_name, exprs, ids)
            )
        }
        Expr::Point3(x, y, z) => {
            format!(
                "\\left({},{},{}\\right)",
                tr(x, fn_name, exprs, ids),
                tr(y, fn_name, exprs, ids),
                tr(z, fn_name, exprs, ids)
            )
        }
        Expr::Call(name, params) => {
            let params = params
                .iter()
                .map(|x| tr(x, fn_name, exprs, ids))
                .collect::<Vec<_>>()
                .join(",");
            if !BUILTIN_FUNCS.contains_key(name) {
                // User defined function
                let name = ident_ify(name);
                format!("{name}\\left({params}\\right)")
            } else {
                let name = BUILTIN_FUNCS[name].2.clone();
                // Builtin function
                format!("\\operatorname{{{name}}}\\left({params}\\right)")
            }
        }
        Expr::For { over, ident, body } => {
            let Some((last, rest)) = body.split_last() else {
                unreachable!()
            };
            let Statement::Expr(last) = last else {
                unreachable!()
            };
            // incredible var names, I know
            let fn_name_pt3 = rest
                .iter()
                .map(|def| {
                    let Statement::Def(n, _e) = def else {
                        unreachable!()
                    };
                    n.clone()
                })
                .collect::<Vec<String>>();
            let fn_name2 = if let Some(fn_name) = fn_name {
                (
                    &format!("{}for{}", fn_name.0, FOR_ID.load(Ordering::Relaxed)),
                    fn_name.1,
                    &fn_name_pt3,
                )
            } else {
                (
                    &format!("for{}", FOR_ID.load(Ordering::Relaxed)),
                    &vec![],
                    &fn_name_pt3,
                )
            };
            let mut additional = rest
                .iter()
                .map(|def| {
                    let Statement::Def(n, e) = def else {
                        unreachable!()
                    };
                    let n = ident_ify(&format!(
                        "{}for{}{}",
                        fn_name.map(|x| x.0).unwrap_or(&"".to_string()),
                        FOR_ID.load(Ordering::Relaxed),
                        n
                    ));
                    let content = format!("{}={}", n, tr(e, Some(fn_name2), exprs, ids));
                    ids.0 += 1;
                    let mut other = hm();
                    other.insert("hidden".to_string(), Value::Bool(true));
                    DesmoExpr {
                        id: ids.0,
                        folder_id: ids.1,
                        content,
                        other,
                    }
                })
                .collect::<Vec<_>>();
            exprs.append(&mut additional);
            FOR_ID.fetch_add(1, Ordering::Relaxed);
            format!(
                "\\left({}\\operatorname{{for}}{}={}\\right)",
                tr(last, Some(fn_name2), exprs, ids),
                ident_ify(ident),
                tr(over, fn_name, exprs, ids)
            )
        }
        Expr::Abs(e) => format!("\\left|{}\\right|", tr(e, fn_name, exprs, ids)),
        Expr::Dot(Dot {
            struct_storage,
            x: _,
            y,
        }) => match struct_storage.storage.lock().unwrap().deref() {
            StructStorage::Unknown => unreachable!(),
            StructStorage::OneVar(v) => {
                ident_ify(&ccase!(camel, format!("{}_{}", struct_storage.name.lock().unwrap(), y)))
            }
            StructStorage::ManyVars(vs) => {
                let v = vs[struct_storage.index.lock().unwrap()[y]].clone();
                ident_ify(&ccase!(camel, v))
            }
        },
    }
}

fn hm<K, V>() -> HashMap<K, V> {
    HashMap::new()
}

fn ident_ify(name: &str) -> String {
    let replaced = name.replace('_', "");
    let (first, rest) = replaced.split_at(1);
    let rest = if rest.is_empty() {
        String::new()
    } else {
        format!("_{{{rest}}}")
    };
    format!("{first}{rest}")
}
