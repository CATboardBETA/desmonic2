#![allow(clippy::type_complexity)]
use crate::parse::Statement;
use crate::transpile::transpile_many;
use crate::type_check::ExprType::{Num, NumList};
use crate::type_check::{ExprType, StructStorage, StructTy, STRUCTS};
use clap::{builder::Styles, Parser, Subcommand};
use convert_case::ccase;
use generate::GraphState;
use std::collections::HashMap;
use std::fs;
use std::ops::Deref;

mod generate;
mod parse;
mod transpile;
mod type_check;

pub const CLAP_STYLING: Styles = Styles::styled()
    .header(clap_cargo::style::HEADER)
    .usage(clap_cargo::style::USAGE)
    .literal(clap_cargo::style::LITERAL)
    .placeholder(clap_cargo::style::PLACEHOLDER)
    .error(clap_cargo::style::ERROR)
    .valid(clap_cargo::style::VALID)
    .invalid(clap_cargo::style::INVALID);

#[derive(Parser)]
#[command(name = "Desmonic",
    version, about = "Desmonic Parser, Transpiler, and Graphstate Generator",
    long_about = None,
    propagate_version = true,
    styles = CLAP_STYLING
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Builds a Desmonic file into a `.json` Desmos graphstate
    #[command(alias = "b")]
    Build {
        /// Input `.desmo` file to compile
        input: String,
        /// Output file for `build`. Defaults to the input file name, with `.desmo` -> `.json`
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Builds a Desmonic file into a graphstate and hosts it on a webserver
    #[command(alias = "r")]
    Run {
        /// Input `.desmo` file to run. With `--from-state`, this should be a graphstate `.json`.
        input: String,
        /// Port to host the state on. This should match with the port in the userscript.
        #[arg(short, long, default_value_t = 8000)]
        port: u16,
        /// Hosts a precompiled graphstate, without compilation
        #[arg(long, short = 's')]
        from_state: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Commands::Build { input, .. } = cli.command {
        let file_str = fs::read_to_string(input).expect("Failed to read input file.");
        let mut ast = parse::gen_ast(&file_str).unwrap();
        let mut vars = HashMap::new();
        let mut funcs = type_check::BUILTIN_FUNCS
            .clone()
            .into_iter()
            .map(|(k, (v1, v2, _))| (k, (v1, v2)))
            .collect();
        let mut errs = vec![];
        type_check(&mut ast, &mut vars, &mut funcs, &mut errs);
        let transpiled = transpile_many(ast, None, 0, None);

        if !errs.is_empty() {
            eprintln!("\nErrors:");
            for err in errs {
                eprintln!("\t{err}")
            }
        }

        println!(
            "\nState:\n{}",
            serde_json::to_string(&GraphState::from_vec(transpiled)).unwrap()
        )
    }
}

fn type_check(
    stmts: &mut Vec<Statement>,
    vars: &mut HashMap<String, ExprType>,
    funcs: &mut HashMap<String, (Vec<ExprType>, ExprType)>,
    errs: &mut Vec<String>,
) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Statement::Expr(e) => {
                type_check::check(e.clone(), vars, funcs, errs);
            }
            Statement::Def(n, e) => {
                let mut typed = type_check::check(e.clone(), vars, funcs, errs);
                if let ExprType::Struct(StructTy {
                    name,
                    index: _,
                    storage,
                }) = &mut typed
                {
                    if let Some(struc) = STRUCTS.lock().unwrap().get(name.lock().unwrap().deref()) {
                        let len = struc.len();
                        *storage.lock().unwrap() = if len == 1 {
                            StructStorage::OneVar(ccase!(
                                camel,
                                format!("{}_{}", name.lock().unwrap(), n)
                            ))
                        } else {
                            StructStorage::ManyVars(
                                struc
                                    .iter()
                                    .enumerate()
                                    .map(|(i, _)| {
                                        ccase!(
                                            camel,
                                            format!("{}_{}{}", name.lock().unwrap(), n, i)
                                        )
                                    })
                                    .collect(),
                            )
                        };
                    } else {
                        errs.push(format!("Struct `{}` does not exist", name.lock().unwrap()))
                    }
                }
                vars.insert(n.clone(), typed);
            }
            Statement::Fn { name, body, params: paramsog } => {
                let params: Vec<_> = paramsog
                    .iter_mut()
                    .map(|x1| {
                        if let ExprType::Struct(StructTy {
                            name,
                            index,
                            storage,
                        }) = &mut x1.1
                        {
                            let name: String = name.lock().unwrap().clone();
                            if let Some(struc) = STRUCTS.lock().unwrap().get(&name) {
                                let len = struc.len();
                                *index.lock().unwrap() = struc
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (k, _))| (k.clone(), i))
                                    .collect();
                                if len == 1 {
                                    let name = format!("{}_{}", name, x1.0);
                                    *storage.lock().unwrap() =
                                        StructStorage::OneVar(ccase!(camel, name.clone()));
                                } else {
                                    *storage.lock().unwrap() = StructStorage::ManyVars(
                                        struc
                                            .iter()
                                            .enumerate()
                                            .map(|(i, _)| {
                                                ccase!(camel, format!("{}{}_{}", name, i, x1.0))
                                            })
                                            .collect(),
                                    );
                                }
                            } else {
                                errs.push(format!("Struct `{}` does not exist", name));
                            }
                        }
                        x1.clone()
                    })
                    .collect();
                let (rest, last) = body.split_at(body.len() - 1);
                let mut locals = vars.clone();
                locals.extend(params.clone());
                for x in rest {
                    let Statement::Def(n, e) = x else {
                        unreachable!()
                    };
                    let typed = type_check::check(e.clone(), &mut locals, funcs, errs);
                    locals.insert(n.clone(), typed);
                }
                let Statement::Expr(last) = last[0].clone() else {
                    unreachable!()
                };
                let typed = type_check::check(last, &mut locals, funcs, errs);
                funcs.insert(
                    name.clone(),
                    (params.iter().map(|x| x.1.clone()).collect(), typed),
                );
                *paramsog =paramsog
                    .iter_mut()
                    .flat_map(|x1| {
                        if let ExprType::Struct(StructTy {
                                                    name,
                                                    index,
                                                    storage,
                                                }) = &mut x1.1
                        {
                            let name: String = name.lock().unwrap().clone();
                            if let Some(struc) = STRUCTS.lock().unwrap().get(&name) {
                                let len = struc.len();
                                *index.lock().unwrap() = struc
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (k, _))| (k.clone(), i))
                                    .collect();
                                if len == 1 {
                                    let name = format!("{}_{}", name, x1.0);
                                    *storage.lock().unwrap() = StructStorage::OneVar(ccase!(
                                        camel,
                                        name.clone()
                                    ));
                                    vec![(name, struc.iter().next().unwrap().1.clone())]
                                } else {
                                    *storage.lock().unwrap() = StructStorage::ManyVars(
                                        struc
                                            .iter()
                                            .enumerate()
                                            .map(|(i, _)| {
                                                ccase!(camel, format!("{}{}_{}", name, i, x1.0))
                                            })
                                            .collect(),
                                    );
                                    struc
                                        .iter()
                                        .enumerate()
                                        .map(|(i, (_, v))| {
                                            (ccase!(camel, format!("{}{}_{}", name, i, x1.0)), v.clone())
                                        })
                                        .collect()
                                }
                            } else {
                                errs.push(format!("Struct `{}` does not exist", name));
                                vec![]
                            }
                        } else {
                            vec![x1.clone()]
                        }
                    })
                    .collect::<Vec<_>>();
            }
            Statement::Styled { stmts, .. } => {
                type_check(stmts, vars, funcs, errs);
            }
            Statement::Implicit(e1, _cmp, e2) => {
                let e1 = type_check::check(e1.clone(), vars, funcs, errs);
                let e2 = type_check::check(e2.clone(), vars, funcs, errs);
                if e1 != e2 {
                    errs.push("Implicit should have equivalent types on each side".to_string());
                }
                if e1 != Num || e1 != NumList || e2 != Num || e2 != NumList {
                    errs.push("Implicit may only be of numbers or lists of numbers".to_string());
                }
            }
            Statement::Struct(..) => {}
        }
    }
}
