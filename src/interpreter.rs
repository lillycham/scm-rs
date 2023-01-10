use std::{collections::HashMap, rc::Rc, fs::read_to_string, io, path::Path};

use crate::{
    lexer::{parse, parse_floats, parse_integrals, parse_list_of_symbols, parse_rats, tokenise, parse_integral},
    types::*,
};

/// Compare numbers
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

    let evaled_forms = eval_forms(args, None, outer)?;
    let mut ops: HashMap<String, Expr> = HashMap::new();
    // let my_callstack = outer.call_stack.clone();
    // let my_stackframe = Frame {
    //     locals: HashMap::new(),
    //     return_to: Continuation {
    //         expr: Rc::new(Expr::Void),
    //         env: Rc::new(Env {
    //             ops: HashMap::new(),
    //             parent_scope: None,
    //             search_path: None,
    //             loaded_modules: Vec::new(),
    //             call_stack: my_callstack.clone()
    //         }),
    //         stack: my_callstack.clone()
    //     },
    //     params: Rc::new(Expr::Void)
    // };

    // my_callstack.push(my_stackframe);

    for (k, v) in syms.iter().zip(evaled_forms.iter()) {
        ops.insert(k.clone(), v.clone());
        
    }

    Ok(Env {
        ops,
        parent_scope: Some(outer),
        search_path: outer.search_path.clone(),
        loaded_modules: outer.loaded_modules.clone(),
        //call_stack: outer.call_stack.clone()
    })
}

#[derive(Debug, Clone, Copy)]
enum ScmNumber {
    Floating(f64),
    Rational(Rational),
    Integral(i128),
}

/// generate try_from for ScmNumber variants
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

/// Parse a list of expressions into a list of numbers
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

/// Initialise the global environment
pub(crate) fn mk_env<'a>() -> Env<'a> {
    let mut ops: HashMap<String, Expr> = HashMap::new();
    ops.insert(
        "+".to_string(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let n = parse_nums(args)?;
            let fst = n.first().ok_or(ScmErr::Reason("expected at least one number".to_string()))?;
            Ok(match fst {
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
            Ok(Expr::Void)
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

    ops.insert(
        "list".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            Ok(Expr::List(args.to_vec()))
        }),
    );

    ops.insert(
        "flip".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let (f, args) = args.split_at(1);
            let f = f.first().unwrap();
            let args = match args.first() {
                Some(Expr::List(l)) => Ok(l.clone()),
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            let mut new_args = vec![];
            for arg in args {
                new_args.push(f.clone());
                new_args.push(arg.clone());
            }
            Ok(Expr::List(new_args))
        }),
    );

    ops.insert(
        "reverse".into(), 
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let list = match args.first() {
                Some(Expr::List(l)) => Ok(l.clone()),
                _ => Err(ScmErr::Reason("expected a list".to_string())),
            }?;
            Ok(Expr::List(list.iter().rev().cloned().collect()))
        }),
    );

    // Aliases for list operations
    ops.insert("first".into(), ops["car".into()].clone());
    ops.insert("rest".into(), ops["cdr".into()].clone());

    // File operations
    

    Env {
        ops,
        parent_scope: None,
        search_path: None,
        loaded_modules: vec![],
        //call_stack: vec![],
    }
}

fn eval_begin(args: &[Expr], namespace: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
    let mut result = Expr::Void;

    for arg in args {
        let result_ = eval(arg, namespace, env)?;
        result = result_;
    }
    Ok(result)
}

/// Call a function with the current continuation
// fn eval_call_cc(args: &[Expr], namespace: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
// }

/// Define an expression in the environment
pub(crate) fn eval_define(args: &[Expr], namespace: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
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

    let value_eval = eval(value, namespace, env)?;

    env.ops.insert(name_str, value_eval);
    Ok(name.clone())
}

/// Evaluate an if expression
fn eval_if(args: &[Expr], namespace: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
    let crit = args
        .first()
        .ok_or(ScmErr::Reason("Expected at least one form".to_string()))?;
    match eval(crit, namespace, env)? {
        Expr::Bool(b) => {
            let branch = if b { 1 } else { 2 };
            let result = args.get(branch).ok_or(ScmErr::Reason(format!(
                "Expected branch conditional {}",
                branch
            )))?;

            eval(result, namespace, env)
        }
        _ => Err(ScmErr::Reason(format!("unexpected criteria {}", crit))),
    }
}

/// Evaluate a lambda form and produce a lambda expression that can be applied
/// to a list of arguments
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

fn eval_macro(args: &[Expr], env: &mut Env) -> ScmResult<Expr> {
    if args.len() != 3 {
        return Err(ScmErr::Reason(
            "'defmacro' only accepts three forms".to_string(),
        ));
    }

    let name = match args[0] {
        Expr::Symbol(ref name) => name.clone(),
        _ => return Err(ScmErr::Reason("expected symbol as first argument".to_string())),
    };

    let params = match args[1] {
        Expr::List(ref params) => Rc::new(Expr::List(params.clone())),
        _ => return Err(ScmErr::Reason("expected list as second argument".to_string())),
    };


    let body = match args[2] {
        Expr::List(_) => Rc::new(args[2].clone()),
        _ => return Err(ScmErr::Reason("expected list as third argument".to_string())),
    };

    let macro_fn = ScmMacro {
        params,
        body,
    };

    // Store the macro in the given environment
    env.ops.insert(name, Expr::Macro(macro_fn));

    Ok(Expr::Void)
}

fn expand_macro(macro_fn: &ScmMacro, args: &[Expr], env: &mut Env) -> ScmResult<Expr> {
    let mut bindings = HashMap::new();

    // Bind the arguments to the macro parameters
    let params = match &*macro_fn.params {
        Expr::List(params) => params,
        _ => return Err(ScmErr::Reason("expected list as macro parameters".to_string())),
    };
    for (param, arg) in params.iter().zip(args.iter()) {
        match param {
            Expr::Symbol(name) => {
                bindings.insert(name.clone(), arg.clone());
            }
            _ => return Err(ScmErr::Reason("expected symbol as macro parameter".to_string())),
        }
    }

    // Example of a macro expansion
    // (define-macro (foo x) (list 'bar x))
    // (foo 1)
    // (list 'bar 1)

    // Expand the macro body
    let mut expanded = vec![];
    let body = match &*macro_fn.body {
        Expr::List(body) => body,
        _ => return Err(ScmErr::Reason("expected list as macro body".to_string())),
    };
    for expr in body.iter() {
        match expr {
            Expr::Symbol(name) => {
                if let Some(binding) = bindings.get(name) {
                    expanded.push(binding.clone());
                } else {
                    expanded.push(expr.clone());
                }
            }
            _ => expanded.push(expr.clone()),
        }
    }
    let expanded = Expr::List(expanded.clone());

    // Evaluate the expanded macro body
    eval(&expanded, None, env)
}

/// Eval a file and load it into the environment
fn require_file(filename: &str, why_loaded: &str, env: &mut Env) -> ScmResult<()> {
    let path = env.search_path.unwrap().join(filename);
    let lib  = read_to_string(path).unwrap();
    let toks = tokenise(&mut lib.chars().peekable());
    let should_load = env.loaded_modules.clone().into_iter().filter(|m| m.name == filename).collect::<Vec<_>>().is_empty();

    if !should_load {
        return Ok(());
    } else {
        env.loaded_modules.push(Module {
            name: filename.to_string(),
            loaded_from: why_loaded.to_string(),
            //exports: vec![],
        });
        match parse_eval(toks, Some(filename), env, 1) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Test if a mod is loaded
pub(crate) fn should_load_mod(mod_name: &str, env: &Env) -> bool {
    let loaded = env.loaded_modules.clone().into_iter().filter(|m| m.name == mod_name).collect::<Vec<_>>();
    loaded.is_empty()
}

/// Add metadata for a synthetic module to the environment
fn require_synthetic(module_name: Option<&str>, why_loaded: &str, env: &mut Env) -> ScmResult<()> {
    let mod_name = module_name.unwrap_or("core").to_string() + "<synthetic>";
    if !should_load_mod(module_name.unwrap(), env) {
        return Ok(());
    } else {
        env.loaded_modules.push(Module {
            name: mod_name,
            loaded_from: why_loaded.to_string(),
            //exports: vec![],
        });        
        Ok(())
    }
}

/// Special form of require that loads the synthetic unsafe module
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

fn require_imperative_constructs(env: &mut Env) -> ScmResult<()> {
    let ops = &mut env.ops;

    ops.insert(
        "begin".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            for expr in args {
                let unpacked = match expr {
                    Expr::List(l) => Ok(l),
                    Expr::Quote(q) => match **q {
                        Expr::List(ref l) => Ok(l),
                        _ => Err(ScmErr::Reason("expected a list".to_string())),
                    },
                    _ => Err(ScmErr::Reason("expected a list".to_string())),
                }?;
                eval(&Expr::List(unpacked.to_owned()), None, &mut mk_env())?;
            };
            
            Ok(Expr::Void)
        }),
    );
    Ok(())
}


/// Special `require` form for loading the `sys` library
fn require_sys(env: &mut Env) -> ScmResult<()> {
    let ops = &mut env.ops;

    // Insert sys ops into the environment
    ops.insert(
        "println".into(),
        Expr::Func(|args: &[Expr]| -> ScmResult<Expr> {
            let arg = args.first().unwrap();
            println!("{}", arg);
            Ok(Expr::Void)
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

/// Read a *module* and evaluate it, loading the contents into the environment
/// Example: (require 'foo) loads the module *foo*
/// Modules are loaded from the search path by require_file().
/// If the module is already loaded, it is not reloaded, as loaded modules are
/// tracked in the environment.
/// 
/// Synthetically loaded modules are modules that are loaded by the interpreter
/// itself, and are not loaded from a file. They are loaded by a specialised
/// function for each module, e.g. require_sys() for the sys module. The
/// require_synthetic() function is used to load the metadata into the environment.
/// A synthetic module path will be of the form `modname<synthetic>`.
pub(crate) fn eval_require(args: &[Expr], why_loaded: &str, env: &mut Env) -> ScmResult<Expr> {
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
                            require_synthetic(Some("sys"), why_loaded, env)?;
                            Ok(Expr::Bool(true))
                        }

                        "unsafe" => {
                            require_unsafe(env)?;
                            require_synthetic(Some("unsafe"), why_loaded, env)?;
                            Ok(Expr::Bool(true))
                        }

                        "imperative/constructs" => {
                            require_imperative_constructs(env)?;
                            require_synthetic(Some("imperative/constructs"), why_loaded, env)?;
                            Ok(Expr::Bool(true))
                        }
                        _ => {
                            let search_path = env.search_path.clone().unwrap();
                            let file = search_path.join(s.to_string());
                            require_file(&file.to_str().unwrap(), why_loaded, env)?;
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

fn eval_printenv(env: &Env, namespace: Option<&str>) -> ScmResult<Expr>{
    use itertools::Itertools;
    println!("op table (current scope):");
    for key in env.ops.clone().keys().into_iter().sorted() {
        println!("{: <10}: {:?}", key, env.ops[key]);
    }
    println!("loaded modules: {:#?}", env.loaded_modules.clone().into_iter().map(|m| (m.name,m.loaded_from)).collect::<Vec<_>>());
    println!("search path: {:?}", env.search_path.unwrap_or(Path::new("")));
    println!("in addition to the above ops, there are also the following builtin syntax forms:
    - `define`
    - `eval`
    - `'`
    - `lambda`
    - `if`
    - `require`
    - `print-env`
    ");

    println!("namespace: {:?}", namespace.unwrap_or("???"));

    Ok(Expr::Void)
}

fn eval_eval(args: &[Expr], module_name: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
    let new_modname = module_name.unwrap_or("<eval>").to_string() + "::<dynamic eval>";
    let expr = args.first().unwrap();
    let unpacked = match expr {
        Expr::Quote(q) => match **q {
            Expr::List(ref l) => Ok(l),
            _ => Err(ScmErr::Reason("expected a list".to_string())),
        },
        _ => Err(ScmErr::Reason("expected a quoted list".to_string())),
    }?;
    eval(&Expr::List(unpacked.to_owned()), Some(&new_modname), env)
}

fn eval_builtin(expr: &Expr, args: &[Expr], module_name: Option<&str>, env: &mut Env) -> Option<ScmResult<Expr>> {
    match expr {
        Expr::Symbol(s) => match s.as_str() {
            "define" => Some(eval_define(args, module_name, env)),
            "if" => Some(eval_if(args, module_name, env)),
            "lambda" => Some(eval_lambda(args)),
            "require" => Some(eval_require(args,
                &format!("loaded by require from {}",
                module_name.unwrap_or("unknown")),
                env
            )),
            "eval" => Some(eval_eval(args, module_name, env)),
            "print-env" => Some(eval_printenv(env, module_name)),
            "define-macro" | "defmacro" => Some(eval_macro(args, env)),
            "begin" => Some(eval_begin(args, module_name, env)),
            _ => None,
        },
        _ => None,
    }
}

fn eval_forms(args: &[Expr], namespace: Option<&str>, env: &mut Env) -> ScmResult<Vec<Expr>> {
    args.iter().map(|x| eval(x, namespace, env)).collect()
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

pub(crate) fn eval(exp: &Expr, namespace: Option<&str>, env: &mut Env) -> ScmResult<Expr> {
    match exp {
        Expr::Bool(b) => Ok(Expr::Bool(*b)),
        Expr::Quote(b) => Ok(Expr::Quote(b.clone())),
        Expr::Symbol(s) => {
            env_get(s, env).ok_or(ScmErr::Reason(format!("unexpected symbol {}", s)))
        }
        Expr::String(s) => Ok(Expr::String(s.clone())),
        Expr::Floating(_) => Ok(exp.clone()),
        Expr::Rational(_) => Ok(exp.clone()),
        Expr::Integral(_) => Ok(exp.clone()),
        Expr::List(l) => {
            let (first, args) = l.split_first().ok_or(ScmErr::Reason(
                format!("missing procedure in expression \n   probably (), which is an empty function application\n   given: {}", exp),
            ))?;
            match eval_builtin(first, args, namespace, env) {
                Some(result) => result,
                None => {
                    let first_eval = eval(first, namespace, env)?;
                    match first_eval {
                        Expr::Func(f) => f(&eval_forms(args, namespace, env)?),

                        Expr::Lambda(l) => {
                            // println!("env: {:?}", env.ops);
                            let new_env = &mut mk_lambda_env(l.params, args, env)?;                           
                            eval(&l.body, namespace, new_env)
                        }

                        Expr::Macro(m) => {
                            let params = m.params.clone();
                            let new_env = &mut mk_lambda_env(params, args, env)?;
                            let expanded = expand_macro(&m, args, new_env)?;
                            eval(&expanded, namespace, new_env)
                        },

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
        Expr::Void => Err(ScmErr::Reason("unexpected void".to_string())),
        Expr::Macro(m) => Err(ScmErr::Reason(format!("unexpected macro: {:?}", m))),
    }
}

pub(crate) fn parse_eval(input: Vec<String>, namespace: Option<&str> ,env: &mut Env, line_num: i32) -> ScmResult<Expr> {
    let (parsed, unparsed) = match parse(&input) {
        Ok((parsed, unparsed)) => (parsed, unparsed),
        Err(e) => {
            return Err(ScmErr::Reason(format!(
                "on line ({}): parse error: {}",
                line_num, e
            )))
        }
    };
    let evaluated = match eval(&parsed, namespace, env) {
        Ok(evaluated) => evaluated,
        Err(e) => {
            return Err(ScmErr::Reason(format!(
                "on line ({}): eval error: {}",
                line_num, e
            )))
        }
    };

    if !unparsed.is_empty() {
        parse_eval(unparsed.to_vec(), namespace, env, line_num + 1)
    } else {
        Ok(evaluated)
    }
}
