use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    fmt,
    iter::{Peekable, Sum},
    ops::{Add, Div, Mul, Sub},
    rc::Rc,
    str::FromStr,
};

#[allow(dead_code)]
#[derive(Debug)]
pub enum ScmErr {
    Reason(String),
}

impl std::fmt::Display for ScmErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ScmErr::Reason(ref s) => write!(f, "{}", s),
        }
    }
}

pub type ScmResult<T> = Result<T, ScmErr>;

#[derive(Clone)]
pub struct ScmLambda {
    pub(crate) params: Rc<Expr>,
    pub(crate) body: Rc<Expr>,
}

#[derive(Clone)]
pub enum Expr {
    Floating(f64),
    Rational(Rational),
    Integral(i128),
    Symbol(String),
    List(Vec<Expr>),
    Func(fn(&[Expr]) -> ScmResult<Expr>),
    Lambda(ScmLambda),
    Bool(bool),
    Quote(Rc<Expr>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum Token {
    LParen,
    RParen,
    Symbol(String),
    Floating(f64),
    Integral(i64),
    Bool(bool),
    Rational(Rational),
    Char(char),
    Str(Rc<RefCell<String>>),
}

#[allow(dead_code)]
impl Token {
    pub fn get(ch: char) -> Token {
        match ch {
            '(' => Token::LParen,
            ')' => Token::RParen,
            x => Token::Char(x),
        }
    }
}
#[allow(dead_code)]
pub struct TokenIterator<I: Iterator<Item = char>> {
    inner: Peekable<I>,
}
#[allow(dead_code)]
impl<I: Iterator<Item = char>> TokenIterator<I> {
    pub fn new(inner: I) -> Self {
        TokenIterator {
            inner: inner.peekable(),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = match self {
            Expr::Symbol(s) => s.clone(),
            Expr::Floating(n) => n.to_string(),
            Expr::List(l) => {
                let chars: Vec<String> = l.iter().map(|x| x.to_string()).collect();
                format!("({})", chars.join(" "))
            }
            Expr::Func(f) => format!("#<proc> function at {:p} {{}}", &f).to_string(),
            Expr::Lambda(l) => format!("#<proc> lambda at {:p} {{}}", &l).to_string(),
            Expr::Bool(b) => {
                if *b {
                    "#t".to_string()
                } else {
                    "#f".to_string()
                }
            }
            Expr::Rational(r) => r.to_string(),
            Expr::Integral(i) => i.to_string(),
            Expr::Quote(b) => format!("'{}", b),
        };
        write!(f, "{}", str)
    }
}

pub struct Env<'a> {
    pub(crate) ops: HashMap<String, Expr>,
    pub(crate) parent_scope: Option<&'a Env<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

#[allow(dead_code)]
impl Rational {
    pub fn new(num: i32, den: i32) -> Rational {
        Rational { num, den }
    }

    // pub fn add(&self, other: &Rational) -> Rational {
    //     Rational::new(
    //         self.num * other.den + self.den * other.num,
    //         self.den * other.den,
    //     )
    //     .pure_reduce()
    // }

    // pub fn sub(&self, other: &Rational) -> Rational {
    //     Rational {
    //         num: self.num * other.den - self.den * other.num,
    //         den: self.den * other.den,
    //     }
    // }

    // pub fn mul(&self, other: &Rational) -> Rational {
    //     Rational {
    //         num: self.num * other.num,
    //         den: self.den * other.den,
    //     }
    // }

    // pub fn div(&self, other: &Rational) -> Rational {
    //     Rational {
    //         num: self.num * other.den,
    //         den: self.den * other.num,
    //     }
    // }

    pub fn to_float(&self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn to_string(&self) -> String {
        format!("{}%{}", self.num, self.den)
    }

    pub fn gcd(a: i32, b: i32) -> i32 {
        if b == 0 {
            a
        } else {
            Rational::gcd(b, a % b)
        }
    }

    pub fn reduce(&mut self) {
        let gcd = Rational::gcd(self.num, self.den);
        self.num /= gcd;
        self.den /= gcd;
    }

    pub fn pure_reduce(&self) -> Rational {
        let mut r = self.clone();
        r.reduce();
        r
    }

    pub fn from_float(f: f64) -> Rational {
        let mut num = f as i32;
        let mut den = 1;
        while num as f64 / den as f64 != f {
            num *= 10;
            den *= 10;
        }
        Rational { num, den }
    }

    pub fn from_string(s: &str) -> Result<Rational, &'static str> {
        let mut parts = s.split("%");
        let num = parts.next().ok_or("no parse")?.parse::<i32>();
        let den = parts.next().ok_or("no parse")?.parse::<i32>();

        match (num, den) {
            (Ok(_), Ok(0)) => Err("Cannot divide by zero"),
            (Ok(n), Ok(d)) => Ok(Rational { num: n, den: d }),
            _ => Err("no parse"),
        }
    }

    pub fn from_int(i: i32) -> Rational {
        Rational { num: i, den: 1 }
    }
}

impl Add for Rational {
    type Output = Rational;

    fn add(self, other: Rational) -> Rational {
        Rational {
            num: self.num * other.den + self.den * other.num,
            den: self.den * other.den,
        }
        .pure_reduce()
    }
}

impl Sub for Rational {
    type Output = Rational;

    fn sub(self, other: Rational) -> Rational {
        Rational {
            num: self.num * other.den - self.den * other.num,
            den: self.den * other.den,
        }
        .pure_reduce()
    }
}

impl Mul for Rational {
    type Output = Rational;

    fn mul(self, other: Rational) -> Rational {
        Rational {
            num: self.num * other.num,
            den: self.den * other.den,
        }
        .pure_reduce()
    }
}

impl Div for Rational {
    type Output = Rational;

    fn div(self, other: Rational) -> Rational {
        Rational {
            num: self.num * other.den,
            den: self.den * other.num,
        }
        .pure_reduce()
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Rational) -> bool {
        self.num == other.num && self.den == other.den
    }
}

impl Eq for Rational {}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Rational) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Rational) -> Ordering {
        let a = self.num * other.den;
        let b = self.den * other.num;
        a.cmp(&b)
    }
}

impl FromStr for Rational {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Rational::from_string(s)
    }
}

impl Sum for Rational {
    fn sum<I: Iterator<Item = Rational>>(iter: I) -> Rational {
        iter.fold(Rational::from_int(0), |acc, x| acc.add(x))
    }
}

// impl<'a> Sum for &'a Rational {
//     fn sum<I: Iterator<Item = &'a Rational>>(iter: I) -> &'a Rational {
//         iter.fold(&Rational::from_int(0), |acc, x| &acc.add(x))
//     }
// }
