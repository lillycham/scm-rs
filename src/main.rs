use std::{env::args, fs, rc::Rc};

use interpreter::{eval_require, mk_env};
use repl::run;
use types::Expr;

use crate::{interpreter::parse_eval, lexer::tokenise};

mod interpreter;
mod lexer;
mod repl;
mod types;

const PROMPT: &str = "scm> ";

fn main() {
    let env = &mut mk_env();
    let flags = args().collect::<Vec<_>>();

    // load library
    eval_require(
        &[Expr::Quote(Rc::new(Expr::Symbol(
            "libscm/lib.p.scm".into(),
        )))],
        env,
    )
    .expect("failed to load base library lib.p.scm");

    if flags.len() == 1 {
        run(env, PROMPT.to_string());
    } else if flags.len() == 2 {
        let file = &flags[1];
        let input = fs::read_to_string(file).expect("Can't read file");
        let toks = tokenise(&mut input.chars().peekable());
        match parse_eval(toks, env, 1) {
            Ok(r) => println!("{}", r),
            Err(e) => println!("ERROR! {}", e),
        }
    }
}
