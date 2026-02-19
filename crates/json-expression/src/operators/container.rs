//! Container operators — mirrors upstream `operators/container.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn len_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(util::len(&val))
}

fn member_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let container = crate::evaluate(&expr[1], ctx)?;
    let index = crate::evaluate(&expr[2], ctx)?;
    util::member(&container, &index)
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "[]",
            aliases: &["member"],
            arity: Arity::Fixed(2),
            eval_fn: member_eval,
            impure: false,
        }),
        // Note: 'len' is also defined in type_ops.ts; container.ts provides the same.
        // We register it here under its own key — the map will take the last one.
        // For exact parity, use "len" from type_ops which was added first.
    ]
}
