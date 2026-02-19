//! Arithmetic operators — mirrors upstream `operators/arithmetic.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn add_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    expr[1..]
        .iter()
        .try_fold(0.0f64, |acc, e| {
            Ok(util::num(&crate::evaluate(e, ctx)?) + acc)
        })
        .map(util::f64_to_jsval)
}

fn subtract_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let first = util::num(&crate::evaluate(&expr[1], ctx)?);
    expr[2..]
        .iter()
        .try_fold(first, |acc, e| {
            Ok(acc - util::num(&crate::evaluate(e, ctx)?))
        })
        .map(util::f64_to_jsval)
}

fn multiply_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    expr[1..]
        .iter()
        .try_fold(1.0f64, |acc, e| {
            Ok(util::num(&crate::evaluate(e, ctx)?) * acc)
        })
        .map(util::f64_to_jsval)
}

fn divide_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let start = util::num(&crate::evaluate(&expr[1], ctx)?);
    expr[2..]
        .iter()
        .try_fold(start, |acc, e| {
            util::slash(&util::f64_to_jsval(acc), &crate::evaluate(e, ctx)?).map(|v| util::num(&v))
        })
        .map(util::f64_to_jsval)
}

fn mod_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let start = util::num(&crate::evaluate(&expr[1], ctx)?);
    expr[2..]
        .iter()
        .try_fold(start, |acc, e| {
            util::modulo(&util::f64_to_jsval(acc), &crate::evaluate(e, ctx)?).map(|v| util::num(&v))
        })
        .map(util::f64_to_jsval)
}

fn min_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let vals: Result<Vec<f64>, JsError> = expr[1..]
        .iter()
        .map(|e| Ok(util::num(&crate::evaluate(e, ctx)?)))
        .collect();
    let vals = vals?;
    let m = vals.into_iter().fold(f64::INFINITY, f64::min);
    Ok(util::f64_to_jsval(m))
}

fn max_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let vals: Result<Vec<f64>, JsError> = expr[1..]
        .iter()
        .map(|e| Ok(util::num(&crate::evaluate(e, ctx)?)))
        .collect();
    let vals = vals?;
    let m = vals.into_iter().fold(f64::NEG_INFINITY, f64::max);
    Ok(util::f64_to_jsval(m))
}

fn round_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).round(),
    ))
}

fn ceil_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).ceil(),
    ))
}

fn floor_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).floor(),
    ))
}

fn trunc_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).trunc(),
    ))
}

fn abs_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).abs(),
    ))
}

fn sqrt_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).sqrt(),
    ))
}

fn exp_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).exp(),
    ))
}

fn ln_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).ln(),
    ))
}

fn log_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let n = util::num(&crate::evaluate(&expr[1], ctx)?);
    let base = util::num(&crate::evaluate(&expr[2], ctx)?);
    Ok(util::f64_to_jsval(n.ln() / base.ln()))
}

fn log10_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    Ok(util::f64_to_jsval(
        util::num(&crate::evaluate(&expr[1], ctx)?).log10(),
    ))
}

fn pow_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let n = util::num(&crate::evaluate(&expr[1], ctx)?);
    let base = util::num(&crate::evaluate(&expr[2], ctx)?);
    Ok(util::f64_to_jsval(n.powf(base)))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "+",
            aliases: &["add"],
            arity: Arity::Variadic,
            eval_fn: add_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "-",
            aliases: &["subtract"],
            arity: Arity::Variadic,
            eval_fn: subtract_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "*",
            aliases: &["multiply"],
            arity: Arity::Variadic,
            eval_fn: multiply_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "/",
            aliases: &["divide"],
            arity: Arity::Variadic,
            eval_fn: divide_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "%",
            aliases: &["mod"],
            arity: Arity::Variadic,
            eval_fn: mod_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "min",
            aliases: &[],
            arity: Arity::Variadic,
            eval_fn: min_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "max",
            aliases: &[],
            arity: Arity::Variadic,
            eval_fn: max_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "round",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: round_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "ceil",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: ceil_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "floor",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: floor_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "trunc",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: trunc_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "abs",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: abs_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "sqrt",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: sqrt_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "exp",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: exp_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "ln",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: ln_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "log",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: log_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "log10",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: log10_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "**",
            aliases: &["pow"],
            arity: Arity::Fixed(2),
            eval_fn: pow_eval,
            impure: false,
        }),
    ]
}
