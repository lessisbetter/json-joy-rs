//! Object operators — mirrors upstream `operators/object.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn validate_set_operand_count(count: usize) -> Result<(), JsError> {
    if count < 3 {
        return Err(JsError::Other("Not enough operands for \"o.set\".".to_string()));
    }
    if count % 2 != 0 {
        return Err(JsError::Other(
            "Invalid number of operands for \"o.set\" operand.".to_string(),
        ));
    }
    Ok(())
}

fn validate_del_operand_count(count: usize) -> Result<(), JsError> {
    if count < 3 {
        return Err(JsError::Other("Not enough operands for \"o.del\".".to_string()));
    }
    Ok(())
}

fn keys_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand = crate::evaluate(&expr[1], ctx)?;
    util::keys(&operand)
}

fn values_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand = crate::evaluate(&expr[1], ctx)?;
    util::values(&operand)
}

fn entries_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand = crate::evaluate(&expr[1], ctx)?;
    util::entries(&operand)
}

fn o_set_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    validate_set_operand_count(expr.len())?;
    let doc_val = crate::evaluate(&expr[1], ctx)?;
    // clone the object so we don't mutate shared data
    let mut obj = match doc_val {
        JsValue::Json(Value::Object(o)) => o.clone(),
        _ => return Err(JsError::NotObject),
    };
    let mut i = 2;
    while i < expr.len() {
        let key_val = crate::evaluate(&expr[i], ctx)?;
        let key = util::str_val(&key_val);
        i += 1;
        let value = util::jsvalue_to_json(crate::evaluate(&expr[i], ctx)?);
        i += 1;
        util::obj_set_raw(&mut obj, &key, value)?;
    }
    Ok(JsValue::Json(Value::Object(obj)))
}

fn o_del_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    validate_del_operand_count(expr.len())?;
    let doc_val = crate::evaluate(&expr[1], ctx)?;
    let mut obj = match doc_val {
        JsValue::Json(Value::Object(o)) => o.clone(),
        _ => return Err(JsError::NotObject),
    };
    for e in &expr[2..] {
        let key_val = crate::evaluate(e, ctx)?;
        let key = util::str_val(&key_val);
        obj.remove(&key);
    }
    Ok(JsValue::Json(Value::Object(obj)))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition { name: "keys", aliases: &[], arity: Arity::Fixed(1), eval_fn: keys_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "values", aliases: &[], arity: Arity::Fixed(1), eval_fn: values_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "entries", aliases: &[], arity: Arity::Fixed(1), eval_fn: entries_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "o.set", aliases: &[], arity: Arity::Variadic, eval_fn: o_set_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "o.del", aliases: &[], arity: Arity::Variadic, eval_fn: o_del_eval, impure: false }),
    ]
}
