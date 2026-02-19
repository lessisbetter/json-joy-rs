//! Input/variable access operators — mirrors upstream `operators/input.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use json_joy_json_pointer::{get, parse_json_pointer, validate_json_pointer};
use serde_json::Value;
use std::sync::Arc;

fn resolve_var(vars: &crate::vars::Vars, varname_val: &JsValue) -> Result<JsValue, JsError> {
    let varname = match varname_val {
        JsValue::Json(Value::String(s)) => s.as_str(),
        _ => return Err(JsError::VarnameMustBeString),
    };
    let (name, pointer) = util::parse_var(varname);
    validate_json_pointer(pointer).map_err(|e| JsError::Other(e.to_string()))?;
    let data = vars.get(name);
    if pointer.is_empty() {
        return Ok(data);
    }
    let path = parse_json_pointer(pointer);
    match &data {
        JsValue::Json(v) => Ok(get(v, &path)
            .map(|v| JsValue::Json(v.clone()))
            .unwrap_or(JsValue::Undefined)),
        _ => Ok(JsValue::Undefined),
    }
}

fn get_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let varname_val = crate::evaluate(&expr[1], ctx)?;
    let defval = if expr.len() >= 3 {
        Some(crate::evaluate(&expr[2], ctx)?)
    } else {
        None
    };
    let value = resolve_var(ctx.vars, &varname_val)?;
    util::throw_on_undef(value, defval)
}

fn defined_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let varname_val = crate::evaluate(&expr[1], ctx)?;
    let value = resolve_var(ctx.vars, &varname_val)?;
    Ok(JsValue::Json(Value::Bool(value != JsValue::Undefined)))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "$",
            aliases: &["get"],
            arity: Arity::Range(1, Some(2)),
            eval_fn: get_eval,
            impure: true,
        }),
        Arc::new(OperatorDefinition {
            name: "$?",
            aliases: &["get?"],
            arity: Arity::Fixed(1),
            eval_fn: defined_eval,
            impure: true,
        }),
    ]
}
