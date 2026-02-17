//! Branching operators — mirrors upstream `operators/branching.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn if_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let condition = crate::evaluate(&expr[1], ctx)?;
    if util::is_truthy(&condition) {
        crate::evaluate(&expr[2], ctx)
    } else {
        crate::evaluate(&expr[3], ctx)
    }
}

fn throw_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    let msg = util::str_val(&val);
    Err(JsError::Thrown(msg))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition { name: "?", aliases: &["if"], arity: Arity::Fixed(3), eval_fn: if_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "throw", aliases: &[], arity: Arity::Fixed(1), eval_fn: throw_eval, impure: false }),
    ]
}
