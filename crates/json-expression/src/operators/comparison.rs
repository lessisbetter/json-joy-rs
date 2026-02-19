//! Comparison operators — mirrors upstream `operators/comparison.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use json_joy_util::deep_equal;
use serde_json::Value;
use std::sync::Arc;

fn deep_eq(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Json(av), JsValue::Json(bv)) => deep_equal(av, bv),
        (JsValue::Undefined, JsValue::Undefined) => true,
        (JsValue::Binary(ab), JsValue::Binary(bb)) => ab == bb,
        _ => false,
    }
}

fn eq_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(deep_eq(&left, &right))))
}

fn ne_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(!deep_eq(&left, &right))))
}

fn gt_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::js_gt(&left, &right))))
}

fn ge_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::js_gte(&left, &right))))
}

fn lt_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::js_lt(&left, &right))))
}

fn le_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::js_lte(&left, &right))))
}

fn cmp_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let left = crate::evaluate(&expr[1], ctx)?;
    let right = crate::evaluate(&expr[2], ctx)?;
    Ok(util::i64_to_jsval(util::cmp(&left, &right)))
}

fn between_eq_eq_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    let min = crate::evaluate(&expr[2], ctx)?;
    let max = crate::evaluate(&expr[3], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::between_eq_eq(
        &val, &min, &max,
    ))))
}

fn between_ne_ne_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    let min = crate::evaluate(&expr[2], ctx)?;
    let max = crate::evaluate(&expr[3], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::between_ne_ne(
        &val, &min, &max,
    ))))
}

fn between_eq_ne_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    let min = crate::evaluate(&expr[2], ctx)?;
    let max = crate::evaluate(&expr[3], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::between_eq_ne(
        &val, &min, &max,
    ))))
}

fn between_ne_eq_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    let min = crate::evaluate(&expr[2], ctx)?;
    let max = crate::evaluate(&expr[3], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::between_ne_eq(
        &val, &min, &max,
    ))))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "==",
            aliases: &["eq"],
            arity: Arity::Fixed(2),
            eval_fn: eq_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "!=",
            aliases: &["ne"],
            arity: Arity::Fixed(2),
            eval_fn: ne_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: ">",
            aliases: &["gt"],
            arity: Arity::Fixed(2),
            eval_fn: gt_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: ">=",
            aliases: &["ge"],
            arity: Arity::Fixed(2),
            eval_fn: ge_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "<",
            aliases: &["lt"],
            arity: Arity::Fixed(2),
            eval_fn: lt_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "<=",
            aliases: &["le"],
            arity: Arity::Fixed(2),
            eval_fn: le_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "cmp",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: cmp_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "=><=",
            aliases: &["between"],
            arity: Arity::Fixed(3),
            eval_fn: between_eq_eq_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "><",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: between_ne_ne_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "=><",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: between_eq_ne_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "><=",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: between_ne_eq_eval,
            impure: false,
        }),
    ]
}
