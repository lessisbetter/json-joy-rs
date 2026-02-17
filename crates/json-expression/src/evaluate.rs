//! The main `evaluate` function — mirrors upstream `evaluate.ts` / `createEvaluate.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{assert_arity, JsValue};
use serde_json::Value;

/// Evaluates a JSON expression against an execution context.
///
/// - Non-array values are returned as literals.
/// - Single-element arrays `[x]` return `x` as a literal.
/// - Multi-element arrays `[operator, ...operands]` dispatch to the matching operator.
///
/// Mirrors upstream `evaluate(expr, ctx)` from `createEvaluate.ts`.
pub fn evaluate(expr: &Value, ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    match expr {
        Value::Array(arr) => {
            if arr.is_empty() {
                // Empty array: treat as empty literal
                return Ok(JsValue::Json(Value::Array(vec![])));
            }
            if arr.len() == 1 {
                // Single-element array: it's a literal wrapper
                return Ok(JsValue::Json(arr[0].clone()));
            }

            // Look up operator
            let op_key = match &arr[0] {
                Value::String(s) => s.as_str(),
                _ => {
                    return Err(JsError::UnknownExpression(format!(
                        "Unknown expression: {}",
                        serde_json::to_string(expr).unwrap_or_default()
                    )))
                }
            };

            let def = ctx.operators.get(op_key).cloned().ok_or_else(|| {
                JsError::UnknownExpression(format!(
                    "Unknown expression: {}",
                    serde_json::to_string(expr).unwrap_or_default()
                ))
            })?;

            assert_arity(def.name, &def.arity, arr.len())?;

            match (def.eval_fn)(arr, ctx) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        other => Ok(JsValue::Json(other.clone())),
    }
}
