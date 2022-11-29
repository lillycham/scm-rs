use std::{iter::Peekable, rc::Rc, vec::IntoIter};

use crate::types::*;

trait GentleIterator<I: Iterator> {
    fn take_until<F>(&mut self, pred: F) -> IntoIter<I::Item>
    where
        F: Fn(&I::Item) -> bool;
}

impl<I: Iterator> GentleIterator<I> for Peekable<I> {
    fn take_until<F>(&mut self, pred: F) -> IntoIter<I::Item>
    where
        F: Fn(&I::Item) -> bool,
    {
        let mut v: Vec<I::Item> = vec![];
        while self.peek().map_or(false, &pred) {
            v.push(self.next().unwrap());
        }

        v.into_iter()
    }
}

pub(crate) fn tokenise<I>(iter: &mut Peekable<I>) -> Vec<String>
where
    I: Iterator<Item = char>,
{
    let mut tokens: Vec<String> = vec![];

    while let Some(t) = tokenise_single(iter) {
        tokens.push(t);
    }

    tokens
}

pub(crate) fn tokenise_single<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    while skip_whitespace(iter) || skip_comment(iter) {
        continue;
    }

    check_lparen(iter)
        .or_else(|| check_rparen(iter))
        .or_else(|| check_quote(iter))
        .or_else(|| check_string(iter))
        .or_else(|| check_hash(iter))
        .or_else(|| check_symbol(iter))
}

fn check_quote<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    if iter.peek().map_or(false, |x| *x == '\'') {
        iter.next();
        Some("'".to_string())
    } else {
        None
    }
}

fn skip_whitespace<I>(iter: &mut Peekable<I>) -> bool
where
    I: Iterator<Item = char>,
{
    if check_char(iter, ' ') || check_char(iter, '\n') {
        iter.next();
        true
    } else {
        false
    }
}

fn skip_comment<I>(iter: &mut Peekable<I>) -> bool
where
    I: Iterator<Item = char>,
{
    if check_char(iter, ';') {
        iter.take_until(|c| *c != '\n');
        true
    } else {
        false
    }
}

fn check_lparen<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    check_for(iter, '(')
}

fn check_rparen<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    check_for(iter, ')')
}

fn check_string<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    if !check_char(iter, '"') {
        return None;
    }

    iter.next();
    let value = iter.take_until(|c| *c != '"').collect();
    iter.next();

    Some(value)
}

fn check_hash<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    if !check_char(iter, '#') {
        return None;
    }

    iter.next();
    match iter.next() {
        Some('t') => Some("#t".to_string()),
        Some('f') => Some("#f".to_string()),
        Some(c) => panic!("Expected '#t' or '#f', got: '#{}'", c),
        None => panic!("Expected char after '#', none found"),
    }
}

fn check_symbol<I>(iter: &mut Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    if !check(iter, |_| true) {
        return None;
    }

    let value: String = iter
        .take_until(|c| *c != ' ' && *c != ')' && *c != '\n')
        .collect();

    match value.parse::<f32>() {
        Ok(n) => Some(n.to_string()),
        Err(_) => Some(value),
    }
}

fn check_for<I>(iter: &mut Peekable<I>, chr: char) -> Option<String>
where
    I: Iterator<Item = char>,
{
    if !check_char(iter, chr) {
        return None;
    }

    iter.next();
    Some(chr.to_string())
}

fn check<F, I>(iter: &mut Peekable<I>, f: F) -> bool
where
    F: Fn(char) -> bool,
    I: Iterator<Item = char>,
{
    if let Some(&x) = iter.peek() {
        f(x)
    } else {
        false
    }
}

fn check_char<I>(iter: &mut Peekable<I>, chr: char) -> bool
where
    I: Iterator<Item = char>,
{
    check(iter, |x| x == chr)
}

pub(crate) fn parse<'a>(toks: &'a [String]) -> ScmResult<(Expr, &'a [String])> {
    let (tok, r) = toks
        .split_first()
        .ok_or(ScmErr::Reason("no tokens".to_string()))?;
    match tok.as_ref() {
        "(" => read_seq(r),
        ")" => Err(ScmErr::Reason("extraneous ')'".to_string())),
        "\'" => {
            let (expr, r) = parse(r)?;
            Ok((Expr::Quote(Rc::new(expr)), r))
        }
        _ => Ok((parse_atom(tok), r)),
    }
}

pub(crate) fn read_seq<'a>(toks: &'a [String]) -> ScmResult<(Expr, &'a [String])> {
    let mut res = Vec::new();
    let mut toks_ = toks;

    loop {
        let (next, rest) = toks_
            .split_first()
            .ok_or(ScmErr::Reason("No closing ')' to match '('".to_string()))?;
        if next == ")" {
            return Ok((Expr::List(res), rest));
        }

        let (exp, new_toks_) = parse(toks_)?;
        res.push(exp);
        toks_ = new_toks_;
    }
}

pub(crate) fn parse_atom(tok: &str) -> Expr {
    match tok.as_ref() {
        "#t" => Expr::Bool(true),
        "#f" => Expr::Bool(false),

        _ => {
            if let Ok(i) = tok.parse::<i128>() {
                Expr::Integral(i)
            } else if let Ok(n) = tok.parse::<f64>() {
                Expr::Floating(n)
            } else if let Ok(r) = tok.parse::<Rational>() {
                Expr::Rational(r)
            } else {
                Expr::Symbol(tok.to_string())
            }
        }
    }
}

pub(crate) fn parse_rat(exp: &Expr) -> ScmResult<Rational> {
    match exp {
        Expr::Rational(r) => Ok(r.clone()),
        Expr::Floating(n) => Ok(Rational::from_float(*n)),
        _ => Err(ScmErr::Reason(format!(
            "Expected a rational number, got {}",
            exp
        ))),
    }
}

pub(crate) fn parse_rats(exp: &[Expr]) -> ScmResult<Vec<Rational>> {
    exp.iter().map(|e| parse_rat(e)).collect()
}

pub(crate) fn parse_floats(args: &[Expr]) -> ScmResult<Vec<f64>> {
    args.iter().map(|x| parse_float(x)).collect()
}

pub(crate) fn parse_float(exp: &Expr) -> ScmResult<f64> {
    match exp {
        Expr::Floating(n) => Ok(*n),
        _ => Err(ScmErr::Reason(format!("expected number, got {}", exp))),
    }
}

pub(crate) fn parse_integral(exp: &Expr) -> ScmResult<i128> {
    match exp {
        Expr::Integral(i) => Ok(*i),
        _ => Err(ScmErr::Reason(format!("expected number, got {}", exp))),
    }
}

pub(crate) fn parse_integrals(args: &[Expr]) -> ScmResult<Vec<i128>> {
    args.iter().map(|x| parse_integral(x)).collect()
}

pub(crate) fn parse_list_of_symbols(args: Rc<Expr>) -> ScmResult<Vec<String>> {
    let list = match args.as_ref() {
        Expr::List(l) => Ok(l.clone()),
        _ => Err(ScmErr::Reason(
            "expected args form to be a list".to_string(),
        )),
    }?;
    list.iter()
        .map(|x| match x {
            Expr::Symbol(s) => Ok(s.clone()),
            _ => Err(ScmErr::Reason(
                "expected symbols in the argument list".to_string(),
            )),
        })
        .collect()
}
