use std::{collections::HashMap, rc::Rc, fs, io};

use crate::{
    lexer::{parse, parse_floats, parse_integrals, parse_list_of_symbols, parse_rats, tokenise, parse_integral},
    types::*,
};

macro_rules! comparison {
    ($check_fn:expr) => {{
        |args: &[Expr]| -> ScmResult<Expr> {
            let floats = parse_floats(args)?;
            let (first, rest) = floats
                .split_first()
                .ok_or(ScmErr::Reason("expected at least one number".to_string()))?;

            fn f(prev: &f64, ts: &[f64]) -> bool {
                match ts.first() {
                    Some(t) => $check_fn(prev, t) && f(t, &ts[1..]),
                    None => true,
                }
            }

            Ok(Expr::Bool(f(first, rest)))
        }
    }};
}

fn mk_lambda_env<'a>(params: Rc<Expr>, args: &[Expr], outer: &'a mut Env) -> ScmResult<Env<'a>> {
    let syms = parse_list_of_symbols(params)?;
    if syms.len() != args.len() {
        return Err(ScmErr::Reason(format!(
            "expected {} arguments, got {}",
            syms.len(),
            args.len()
        )));
    };

    let evaled_forms = eval_forms(args, outer)?;
    let mut ops: HashMap<String, Expr> = HashMap::new();

    for (k, v) in syms.iter().zip(evaled_forms.iter()) {
        ops.insert(k.clone(), v.clone());
    }

    Ok(Env {
        ops,
        parent_scope: Some(outer),
    })
}

#[derive(Debug, Clone, Copy)]
enum ScmNumber {
    Floating(f64),
    Rational(Rational),
    Integral(i128),
}

// generate try_from for ScmNumber variants
macro_rules! try_from {
    ($t:ty, $v:ident) => {
        impl TryFrom<ScmNumber> for $t {
            type Error = ScmErr;

            fn try_from(value: ScmNumber) -> Result<Self, Self::Error> {
                match value {
                    ScmNumber::$v(i) => Ok(i),
                    _ => Err(ScmErr::Reason(format!("expected {}", stringify!($v)))),
                }
            }
        }
    };
}

try_from!(f64, Floating);
try_from!(Rational, Rational);
try_from!(i128, Integral);

fn parse_nums(args: &[Expr]) -> ScmResult<Vec<ScmNumber>> {
    if let Ok(n) = parse_floats(args) {
        Ok(n.into_iter().map(|x| ScmNumber::Floating(x)).collect())
    } else if let Ok(n) = parse_rats(args) {
        Ok(n.into_iter().map(|x| ScmNumber::Rational(x)).collect())
    } else if let Ok(n) = parse_integrals(args) {
        Ok(n.into_iter().map(|x| ScmNumber::Integral(x)).collect())
    } else {
        Err(ScmErr::Reason("Expected number".into()))
    }
}

pub(crate) fn mk_env<'a>() -> Env<'a> {
    let mut ops: HashMap<String, Expr> = HashMap::new();
    ops.insert(
        "+".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let n = parse_nums(args)?;
            Ok(match n.first().unwrap() {
                ScmNumber::Floating(_) => {
                    Expr::Floating(n.into_iter().map(|x| f64::try_from(x).unwrap()).sum())
                }
                ScmNumber::Rational(_) => {
                    Expr::Rational(n.into_iter().map(|x| Rational::try_from(x).unwrap()).sum())
                }
                ScmNumber::Integral(_) => Expr::Integral(
                    n.into_iter()
                        .map(|x| i128::try_from(x).unwrap())
                        .sum::<i128>(),
                ),
            })
        }),
    );

    ops.insert(
        "-".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let nums = parse_floats(args)?;
            let (first, rest) = nums
                .split_first()
                .ok_or(ScmErr::Reason("Expected at least one number".to_string()))?;
            let sum_rest: f64 = rest.iter().sum();
            Ok(Expr::Floating(first - sum_rest))
        }),
    );

    ops.insert(
        "*".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let nums = parse_floats(args)?;
            Ok(Expr::Floating(nums.iter().product()))
        }),
    );

    ops.insert(
        "/".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let nums = parse_floats(args)?;
            let (first, rest) = nums
                .split_first()
                .ok_or(ScmErr::Reason("Expected at least one number".to_string()))?;
            let prod_rest: f64 = rest.iter().product();
            Ok(Expr::Floating(first / prod_rest))
        }),
    );

    ops.insert(
        "empty?".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let list = match args.first() {
                Some(Expr::List(l)) => Ok(l.clone()),
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            Ok(Expr::Bool(list.is_empty()))
        }),
    );

    ops.insert(
        "length".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let list = match args.first() {
                Some(Expr::List(l)) => Ok(l.clone()),
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            Ok(Expr::Floating(list.len() as f64))
        }),
    );

    ops.insert("=".to_string(), Expr::Func(comparison!(|a, b| a == b)));

    ops.insert(">".to_string(), Expr::Func(comparison!(|a, b| a > b)));

    ops.insert("<".to_string(), Expr::Func(comparison!(|a, b| a < b)));

    ops.insert(">=".to_string(), Expr::Func(comparison!(|a, b| a >= b)));

    ops.insert("<=".to_string(), Expr::Func(comparison!(|a, b| a <= b)));

    ops.insert(
        "display".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            for arg in args {
                print!("{}", arg);
            }
            println!();
            Ok(Expr::Bool(true))
        }),
    );

    // List operations

    ops.insert(
        "cons".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let (car, cdr) = args.split_at(1);
            let car = car.first().unwrap();
            let cdr = match cdr.first() {
                Some(Expr::List(l)) => Ok(l.clone()),
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            let mut new_list = vec![car.clone()];
            new_list.extend(cdr);
            Ok(Expr::List(new_list))
        }),
    );

    ops.insert(
        "car".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let list = match args.first() {
                Some(Expr::Quote(l)) => match **l {
                    Expr::List(ref l) => Ok(l.clone()),
                    _ => Err(ScmErr::Reason("expected a list".to_string())),
                },
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            let car = list
                .first()
                .ok_or(ScmErr::Reason("expected a non-empty list".to_string()))?;
            Ok(car.clone())
        }),
    );

    ops.insert(
        "cdr".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let list = match args.first() {
                Some(Expr::Quote(r)) => match **r {
                    Expr::List(ref l) => Ok(l.clone()),
                    _ => Err(ScmErr::Reason("expected a quoted list".to_string())),
                },
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            let cdr = list
                .split_first()
                .ok_or(ScmErr::Reason("expected a non-empty list".to_string()))?
                .1;
            Ok(Expr::Quote(Rc::new(Expr::List(cdr.to_vec()))))
        }),
    );

    // Aliases for list operations
    ops.insert("first".into(), ops["car".into()].clone());
    ops.insert("rest".into(), ops["cdr".into()].clone());

    // Eval and apply
    ops.insert(
        "eval".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let expr = args.first().unwrap();
            let expr_unquoted = match expr {
                Expr::Quote(e) => match **e {
                    Expr::List(ref l) => Ok(Expr::List(l.clone())),
                    _ => Err(ScmErr::Reason("expected a quoted list".to_string())),
                },
                _ => Err(ScmErr::Reason("expected a quoted list".to_string())),
            }?;
            let mut env = mk_env();

            Ok(eval(&expr_unquoted, &mut env)?)
        }),
    );

    // (define apply (lambda (fn x) (fn x)))
    // Couldn't figure out how to implement this in Rust, so it lives in schemeland
    // ops.insert(
    //     "apply".into(),
    //     Expr::Lambda (
    //         ScmLambda {
    //         params: Rc::new(Expr::List(vec![Expr::Symbol("fn".into()), Expr::Symbol("x".into())])),
    //         body: Rc::new(Expr::List(vec![
    //             Expr::Symbol("fn".into()),
    //             Expr::Symbol("x".into()),
    //         ]))
    //     })
    // );

    //);
    Env {
        ops,
        parent_scope: None,
    }
}

fn include_file(filename: &str, env: &mut Env) -> ScmResult<()> {
    let lib = fs::read_to_string(filename).expect("Lib not found!");
    let toks = tokenise(&mut lib.chars().peekable());
    match parse_eval(toks, env, 1) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub(crate) fn eval_define(args: &[Expr], env: &mut Env) -> ScmResult<Expr> {
    if args.len() > 2 {
        return Err(ScmErr::Reason(
            "'define' only accepts two forms".to_string(),
        ));
    };

    let (name, rest) = args
        .split_first()
        .ok_or(ScmErr::Reason("Expected at least one argument".to_string()))?;

    let name_str = match name {
        Expr::Symbol(s) => Ok(s.clone()),
        _ => Err(ScmErr::Reason("Expected symbol".to_string())),
    }?;

    if env.ops.contains_key(&name_str) {
        return Err(ScmErr::Reason(
            format!("can not overwrite a reserved operation or redefine an existing variable ({}) at global scope", name_str),
        ));
    }

    let value = rest
        .get(0)
        .ok_or(ScmErr::Reason("Expected at least one form".to_string()))?;

    let value_eval = eval(value, env)?;

    env.ops.insert(name_str, value_eval);
    Ok(name.clone())
}

fn eval_if(args: &[Expr], env: &mut Env) -> ScmResult<Expr> {
    let crit = args
        .first()
        .ok_or(ScmErr::Reason("Expected at least one form".to_string()))?;
    match eval(crit, env)? {
        Expr::Bool(b) => {
            let branch = if b { 1 } else { 2 };
            let result = args.get(branch).ok_or(ScmErr::Reason(format!(
                "Expected branch conditional {}",
                branch
            )))?;

            eval(result, env)
        }
        _ => Err(ScmErr::Reason(format!("unexpected criteria {}", crit))),
    }
}

fn eval_lambda(args: &[Expr]) -> ScmResult<Expr> {
    if args.len() > 2 {
        return Err(ScmErr::Reason(
            "'lambda' only accepts two forms".to_string(),
        ));
    };

    let params = args.first().ok_or(ScmErr::Reason(
        "expected at least one argument in lambda form".to_string(),
    ))?;

    let body = args
        .get(1)
        .ok_or(ScmErr::Reason("expected body in lambda form".to_string()))?;

    // println!("#<proc> lambda ({:?}) ({:?})", params, body);

    Ok(Expr::Lambda(ScmLambda {
        params: Rc::new(params.clone()),
        body: Rc::new(body.clone()),
    }))
}

fn require_unsafe(env: &mut Env) -> ScmResult<()> {
    let ops = &mut env.ops;

    ops.insert(
        "alloc-box".into(),
        Expr::Func(|x: &[Expr]| -> ScmResult<Expr> {
            let val = x.first().unwrap();
            let box_ = Box::new(val.clone());
            let ptr = Box::into_raw(box_);
            Ok(Expr::Ptr(ptr))
        }),
    );

    ops.insert(
        "deref-box".into(),
        Expr::Func(|x: &[Expr]| -> ScmResult<Expr> {
            let ptr = x.first().unwrap();
            let ptr = match ptr {
                Expr::Ptr(p) => Ok(p),
                _ => Err(ScmErr::Reason("expected a pointer".to_string())),
            }?;
            let box_ = unsafe { Box::from_raw(*ptr) };
            Ok(*box_)
        }),
    );

    Ok(())
}

// Special `require` form for loading the `sys` library
fn require_sys(env: &mut Env) -> ScmResult<()> {
    let ops = &mut env.ops;

    // Insert sys ops into the environment
    ops.insert(
        "println".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let arg = args.first().unwrap();
            println!("{}", arg);
            Ok(Expr::Bool(true))
        }),
    );

    ops.insert(
        "readln".into(),
        Expr::Func(|_args: &[Expr]| -> ScmResult<Expr> {
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            Ok(Expr::Symbol(input))
        }),
    );

    ops.insert(
        "die".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let arg = args.first().unwrap();
            println!("{}", arg);
            std::process::exit(1);
        }),
    );

    ops.insert(
        "exit".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let code = parse_integral(&args[0]).expect("Expected an integral value!") as i32;
            std::process::exit(code);
        }),
    );


    Ok(())
}

pub(crate) fn eval_require(args: &[Expr], env: &mut Env) -> ScmResult<Expr> {
    let filename = args.first().ok_or(ScmErr::Reason(
        "expected at least one argument in require form".to_string(),
    ))?;

    Ok(match filename {
        Expr::Quote(q) => {
            match q.to_owned().as_ref() {
                Expr::Symbol(s) => {
                    match &s[..] {
                        "sys" => {
                            require_sys(env)?;
                            Ok(Expr::Bool(true))
                        }
                        "unsafe" => {
                            require_unsafe(env)?;
                            Ok(Expr::Bool(true))
                        }
                        _ => {
                            include_file(s, env)?;
                            Ok(Expr::Bool(true))
                        }
                    }
                },
                _ => return Err(ScmErr::Reason("expected a quoted symbol".to_string())),
            }
        }
        _ => Err(ScmErr::Reason(format!(
            "expected symbol in require form, got {}",
            filename
        ))),
    }?)
}

fn eval_builtin(expr: &Expr, args: &[Expr], env: &mut Env) -> Option<ScmResult<Expr>> {
    match expr {
        Expr::Symbol(s) => match s.as_str() {
            "define" => Some(eval_define(args, env)),
            "if" => Some(eval_if(args, env)),
            "lambda" => Some(eval_lambda(args)),
            "require" => Some(eval_require(args, env)),
            _ => None,
        },
        _ => None,
    }
}

fn eval_forms(args: &[Expr], env: &mut Env) -> ScmResult<Vec<Expr>> {
    args.iter().map(|x| eval(x, env)).collect()
}

fn env_get(s: &str, env: &Env) -> Option<Expr> {
    match env.ops.get(s) {
        Some(expr) => Some(expr.clone()),
        None => match &env.parent_scope {
            Some(outer) => env_get(s, &outer),
            None => None,
        },
    }
}

pub(crate) fn eval(exp: &Expr, env: &mut Env) -> ScmResult<Expr> {
    match exp {
        Expr::Bool(b) => Ok(Expr::Bool(*b)),
        Expr::Quote(b) => Ok(Expr::Quote(b.clone())),
        Expr::Symbol(s) => {
            env_get(s, env).ok_or(ScmErr::Reason(format!("unexpected symbol {}", s)))
        }
        Expr::Floating(_) => Ok(exp.clone()),
        Expr::Rational(_) => Ok(exp.clone()),
        Expr::Integral(_) => Ok(exp.clone()),
        Expr::List(l) => {
            let (first, args) = l.split_first().ok_or(ScmErr::Reason(
                format!("missing procedure in expression \n   probably (), which is an empty function application\n   given: {}", exp),
            ))?;
            match eval_builtin(first, args, env) {
                Some(result) => result,
                None => {
                    let first_eval = eval(first, env)?;
                    match first_eval {
                        Expr::Func(f) => f(&eval_forms(args, env)?),

                        Expr::Lambda(l) => {
                            // println!("env: {:?}", env.ops);
                            let new_env = &mut mk_lambda_env(l.params, args, env)?;                           
                            eval(&l.body, new_env)
                        }

                        _ => Err(ScmErr::Reason(
                            format!("not a procedure\n   expected a procedure or function as first form in unquoted list\n   given: {}", exp),
                        )),
                    }
                }
            }
        }
        Expr::Func(_) => Err(ScmErr::Reason("unexpected form".to_string())),
        Expr::Lambda(_) => Err(ScmErr::Reason("unexpected form".to_string())),
        Expr::Ptr(_) => Err(ScmErr::Reason("unexpected pointer at the top level".to_string())),
    }
}

pub(crate) fn parse_eval(input: Vec<String>, env: &mut Env, line_num: i32) -> ScmResult<Expr> {
    let (parsed, unparsed) = match parse(&input) {
        Ok((parsed, unparsed)) => (parsed, unparsed),
        Err(e) => {
            return Err(ScmErr::Reason(format!(
                "on line ({}): parse error: {}",
                line_num, e
            )))
        }
    };
    let evaluated = match eval(&parsed, env) {
        Ok(evaluated) => evaluated,
        Err(e) => {
            return Err(ScmErr::Reason(format!(
                "on line ({}): eval error: {}",
                line_num, e
            )))
        }
    };

    if !unparsed.is_empty() {
        parse_eval(unparsed.to_vec(), env, line_num + 1)
    } else {
        Ok(evaluated)
    }
}
