use std::io::{self, Write};

use crate::{interpreter::parse_eval, lexer::tokenise, types};

pub(crate) fn run(env: &mut types::Env, prompt: String) {
    loop {
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        let read_size = io::stdin().read_line(&mut input).unwrap();

        if read_size == 0 {
            break;
        }

        if let "exit" | "quit" | "q" = input.trim() {
            break;
        }

        let toks = tokenise(&mut input.chars().peekable());

        match parse_eval(toks, Some("<repl context>"), env, 1) {
            Ok(types::Expr::Void) => (),
            Ok(r) => println!("=> {}", r),
            Err(e) => println!("\x1b[31;m=> Error: {}\x1b[0m", e),
        }
    }
}
