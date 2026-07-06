use crate::parse::Statement;
use crate::transpile::transpile_many;
use crate::type_check::ExprType;
use clap::{Parser, Subcommand, builder::Styles};
use generate::GraphState;
use std::collections::HashMap;
use std::fs;

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
        let ast = parse::gen_ast(&file_str).unwrap();
        let mut vars = HashMap::<String, ExprType>::new();
        let mut funcs = HashMap::<String, ExprType>::new();
        let mut errs = vec![];
        for stmt in ast.clone() {
            type_check(stmt, &mut vars, &mut funcs, &mut errs);
        }
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

fn type_check(stmt: Statement, vars: &mut HashMap<String, ExprType>, funcs: &mut HashMap<String, ExprType>, errs: &mut Vec<String>) {
    match stmt {
        Statement::Expr(e) => {
            type_check::check(e, vars, funcs, errs);
        }
        Statement::Def(n, e) => {
            let typed = type_check::check(e, vars, funcs, errs);
            vars.insert(n, typed);
        }
        Statement::Fn { name, body, params } => {
            let (rest, last) = body.split_at(body.len() - 1);
            let mut locals = vars.clone();
            locals.extend(params);
            for x in rest {
                let Statement::Def(n, e) = x else {
                    unreachable!()
                };
                let typed =
                    type_check::check(e.clone(), &mut locals,  funcs,  errs);
                locals.insert(n.clone(), typed);
            }
            let Statement::Expr(last) = last[0].clone() else {
                unreachable!()
            };
            let typed = type_check::check(last, &mut locals,  funcs,  errs);
            funcs.insert(name, typed);
        }
        Statement::Styled { stmts, .. } => {
            for stmt in stmts {
                type_check(stmt, vars, funcs, errs);
            }
        }
    }
}