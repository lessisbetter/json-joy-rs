//! Logical operators — mirrors upstream `operators/logical.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn and_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let mut acc = crate::evaluate(&expr[1], ctx)?;
    for e in &expr[2..] {
        if !util::is_truthy(&acc) {
            return Ok(acc);
        }
        acc = crate::evaluate(e, ctx)?;
    }
    Ok(acc)
}

fn or_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let mut acc = crate::evaluate(&expr[1], ctx)?;
    for e in &expr[2..] {
        if util::is_truthy(&acc) {
            return Ok(acc);
        }
        acc = crate::evaluate(e, ctx)?;
    }
    Ok(acc)
}

fn not_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(!util::is_truthy(&val))))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "&&",
            aliases: &["and"],
            arity: Arity::Variadic,
            eval_fn: and_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "||",
            aliases: &["or"],
            arity: Arity::Variadic,
            eval_fn: or_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "!",
            aliases: &["not"],
            arity: Arity::Fixed(1),
            eval_fn: not_eval,
            impure: false,
        }),
    ]
}
