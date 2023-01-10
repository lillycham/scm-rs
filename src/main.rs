#![feature(string_remove_matches)]
use std::{fs, io::Read, path::Path, rc::Rc};

use clap::{Parser, Subcommand};
use interpreter::{eval_require, mk_env};
use repl::run;
use types::Expr;

use crate::{interpreter::parse_eval, lexer::tokenise};

mod interpreter;
mod lexer;
mod package_manage;
mod repl;
mod types;

const PROMPT: &str = "scm> ";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Run a file")]
    Run { file: String },
    #[command(about = "Start a REPL")]
    Repl,
    #[command(about = "Install a package")]
    Install { package: String },
}

fn main() {
    let env = &mut mk_env();
    //let flags = args().collect::<Vec<_>>();
    let flags = Cli::parse();

    // Ensure that the interpreter library directory exists
    let home = dirs::home_dir().unwrap();
    let lib_dir = home.join(".pscm-rs");
    if !lib_dir.exists() {
        fs::create_dir(&lib_dir).unwrap();
    }
    let search_path = lib_dir.join(Path::new("lib"));
    if !search_path.exists() {
        fs::create_dir(&search_path).unwrap();
    }

    // Insert the library directory into the search path
    env.search_path = Some(&(search_path));

    // load library
    eval_require(
        &[Expr::Quote(Rc::new(Expr::Symbol("base".into())))],
        "<default>",
        env,
    )
    .expect("failed to load base library");

    match flags.command {
        Some(c) => match c {
            Commands::Run { file } => {
                let mut file_ = if file == "-" {
                    Box::new(std::io::stdin()) as Box<dyn Read>
                } else {
                    Box::new(fs::File::open(&file).unwrap()) as Box<dyn Read>
                };
                let mut input = String::new();
                file_.read_to_string(&mut input).unwrap();
                let toks = tokenise(&mut input.chars().peekable());
                match parse_eval(
                    toks,
                    Some(&format!(
                        "<file context: {}>",
                        if file == "-" { "STDIN" } else { &file }
                    )),
                    env,
                    1,
                ) {
                    Ok(r) => println!("=> {}", r),
                    Err(e) => println!("\x1b[31;m=> Error: {}\x1b[0m", e),
                }
            }
            Commands::Install { package } => {
                let result = package_manage::install_package_git_interface(package);
                match result {
                    Ok(_) => {}
                    Err(e) => println!("\x1b[31;mError: {}\x1b[0m", e),
                }
            }
            Commands::Repl => run(env, PROMPT.to_string()),
        },
        None => run(env, PROMPT.to_string()),
    }

    // match &flags.command[..] {
    //     "" => run(env, PROMPT.to_string()),
    //     file => {
    //         let file_ = Path::new(file);
    //         let contents = fs::read_to_string(file_).expect("failed to read file");
    //         let toks = tokenise(&mut contents.chars().peekable());
    //         match parse_eval(toks, Some(&file), env, 1) {
    //             Ok(r) => println!("=> {}", r),
    //             Err(e) => println!("\x1b[31;m=> Error: {}\x1b[0m", e),
    //         }
    //     }
    // }
}
