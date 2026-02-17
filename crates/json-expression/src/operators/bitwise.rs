//! Bitwise operators — mirrors upstream `operators/bitwise.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn bit_and_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let first = util::int(&crate::evaluate(&expr[1], ctx)?);
    let result = expr[2..].iter().try_fold(first, |acc, e| {
        Ok(acc & util::int(&crate::evaluate(e, ctx)?))
    })?;
    Ok(util::i32_to_jsval(result))
}

fn bit_or_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let first = util::int(&crate::evaluate(&expr[1], ctx)?);
    let result = expr[2..].iter().try_fold(first, |acc, e| {
        Ok(acc | util::int(&crate::evaluate(e, ctx)?))
    })?;
    Ok(util::i32_to_jsval(result))
}

fn bit_xor_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let first = util::int(&crate::evaluate(&expr[1], ctx)?);
    let result = expr[2..].iter().try_fold(first, |acc, e| {
        Ok(acc ^ util::int(&crate::evaluate(e, ctx)?))
    })?;
    Ok(util::i32_to_jsval(result))
}

fn bit_not_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = util::int(&crate::evaluate(&expr[1], ctx)?);
    Ok(util::i32_to_jsval(!val))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition { name: "&", aliases: &["bitAnd"], arity: Arity::Variadic, eval_fn: bit_and_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "|", aliases: &["bitOr"], arity: Arity::Variadic, eval_fn: bit_or_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "^", aliases: &["bitXor"], arity: Arity::Variadic, eval_fn: bit_xor_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "~", aliases: &["bitNot"], arity: Arity::Fixed(1), eval_fn: bit_not_eval, impure: false }),
    ]
}
