//! Binary (Uint8Array) operators — mirrors upstream `operators/binary.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn u8_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let bin = crate::evaluate(&expr[1], ctx)?;
    let index = crate::evaluate(&expr[2], ctx)?;
    util::u8_val(&bin, &index)
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "u8",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: u8_eval,
            impure: false,
        }),
    ]
}
