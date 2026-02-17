//! Type operators — mirrors upstream `operators/type.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn type_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::String(util::js_type(&val).to_string())))
}

fn bool_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_truthy(&val))))
}

fn num_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(util::f64_to_jsval(util::num(&val)))
}

fn str_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::String(util::str_val(&val))))
}

fn len_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(util::len(&val))
}

fn und_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(val == JsValue::Undefined)))
}

fn nil_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Json(Value::Null)))))
}

fn bool_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Json(Value::Bool(_))))))
}

fn num_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Json(Value::Number(_))))))
}

fn str_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Json(Value::String(_))))))
}

fn bin_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Binary(_)))))
}

fn arr_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(matches!(val, JsValue::Json(Value::Array(_))))))
}

fn obj_q_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::js_type(&val) == "object")))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition { name: "type", aliases: &[], arity: Arity::Fixed(1), eval_fn: type_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "bool", aliases: &[], arity: Arity::Fixed(1), eval_fn: bool_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "num", aliases: &[], arity: Arity::Fixed(1), eval_fn: num_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "str", aliases: &[], arity: Arity::Fixed(1), eval_fn: str_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "len", aliases: &[], arity: Arity::Fixed(1), eval_fn: len_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "und?", aliases: &[], arity: Arity::Fixed(1), eval_fn: und_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "nil?", aliases: &[], arity: Arity::Fixed(1), eval_fn: nil_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "bool?", aliases: &[], arity: Arity::Fixed(1), eval_fn: bool_q_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "num?", aliases: &[], arity: Arity::Fixed(1), eval_fn: num_q_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "str?", aliases: &[], arity: Arity::Fixed(1), eval_fn: str_q_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "bin?", aliases: &[], arity: Arity::Fixed(1), eval_fn: bin_q_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "arr?", aliases: &[], arity: Arity::Fixed(1), eval_fn: arr_q_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "obj?", aliases: &[], arity: Arity::Fixed(1), eval_fn: obj_q_eval, impure: false }),
    ]
}
