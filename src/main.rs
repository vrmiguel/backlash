#![feature(impl_trait_in_bindings)]
#![allow(unused)]
#![allow(incomplete_features)]

use std::collections::HashMap;

use fehler::throw;

use color_eyre::{
    eyre::{bail, eyre},
    Report, Result,
};

use dyn_clone::DynClone;

use lasso::{Rodeo, Spur};

use maplit::hashmap;

use Expr::*;

pub mod parser;

type Error = Report;

type Symbol = Spur;

type Map<T> = HashMap<Symbol, T>;

trait FnClone: DynClone + Fn(Vec<Expr>) -> Result<Expr> {}

impl<T> FnClone for T where T: DynClone + Fn(Vec<Expr>) -> Result<Expr> {}

dyn_clone::clone_trait_object!(FnClone);

pub struct Env {
    vars: Map<Expr>,
    interner: Rodeo,
}

#[derive(Clone)]
enum Expr {
    Num(f64),
    Var(Symbol),
    App(Box<Expr>, Vec<Expr>),
    PrimOp(Box<dyn FnClone>),
}

#[fehler::throws]
fn apply<'a>(env: &Env, f: Expr, args: Vec<Expr>) -> Expr {
    if let PrimOp(op) = eval(env, f)? {
        let args = args.into_iter().map(|x| eval(env, x));

        let a: Vec<Expr> = args.map(|x| x.unwrap()).collect();

        op(a)?
    } else {
        bail!("not a primop");
    }
}

#[fehler::throws]
fn eval(env: &Env, expr: Expr) -> Expr {
    match expr {
        Var(x) => env.vars.get(&x).map(|x| (*x).clone()).unwrap_or(Var(x)),
        App(f, args) => apply(env, *f, args)?,
        x => x,
    }
}

#[fehler::throws]
fn bin_op(f: impl Fn(f64, f64) -> f64, init: f64, args: Vec<Expr>) -> Expr {
    let mut acc = init;

    for el in args {
        match el {
            Num(num) => acc = f(acc, num),
            _ => bail!("operating on a non-number"),
        }
    }

    Num(acc)
}

fn env() -> Env {
    let mut rodeo = Rodeo::new();

    let mut key = |s: &'static str| rodeo.get_or_intern_static(s);

    let map = hashmap![
        key("+") => PrimOp(Box::new(|args| bin_op(|a, b| a + b, 0.0, args))),
        key("*") => PrimOp(Box::new(|args| bin_op(|a, b| a * b, 1.0, args))),

        key("e") => Num(std::f64::consts::E),
        key("pi") => Num(std::f64::consts::PI),
    ];

    Env {
        vars: map,
        interner: rodeo,
    }
}

#[test]
fn test_add() {
    let env = env();

    let add = env.vars[&env.interner.get("+").unwrap()].clone();

    let expr = App(
        Box::new(add),
        vec![Num(1.0), Var(env.interner.get("e").unwrap()), Num(0.5)],
    );

    match eval(&env, expr).unwrap() {
        Num(val) if val == 1.0 + std::f64::consts::E + 0.5 => (),
        _ => unreachable!(),
    };
}

#[test]
#[should_panic]
fn test_mul() {
    let env = env();

    let mul = env.vars[&env.interner.get("*").unwrap()].clone();

    let expr = App(
        Box::new(mul),
        vec![Num(0.75), Var(env.interner.get("pi").unwrap()), Num(10.0)],
    );

    match eval(&env, expr).unwrap() {
        Num(val) if val == 0.76 * std::f64::consts::PI * 10.0 => (),
        _ => panic!("they should indeed be different, 0.75 != 0.76"),
    };
}

fn main() {
    use rustyline::error::ReadlineError::*;
    use rustyline::Editor;

    let xdg_dirs = xdg::BaseDirectories::with_prefix("backlash").unwrap();

    let history_path = xdg_dirs
        .place_config_file("history")
        .expect("cannot create configuration directory");

    let mut rl = Editor::<()>::new();

    rl.load_history(&history_path).unwrap();

    let mut env = env();

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(&line);

                use combine::*;
                let parsed = crate::parser::parse_expr().easy_parse(line.as_str());

                match &parsed {
                    Ok(s) => println!("Parsed: {:?}", &parsed),
                    Err(e) => println!("Error: {}", e),
                }
            }
            Err(Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
        rl.save_history(&history_path).unwrap();
    }
}
