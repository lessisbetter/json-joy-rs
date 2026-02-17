//! JSON Patch operators — mirrors upstream `operators/patch.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use json_joy_json_pointer::parse_json_pointer;
use serde_json::Value;
use std::sync::Arc;

fn validate_add_operand_count(count: usize) -> Result<(), JsError> {
    if count < 3 {
        return Err(JsError::Other(
            "Not enough operands for \"jp.add\" operand.".to_string(),
        ));
    }
    if count % 2 != 0 {
        return Err(JsError::Other(
            "Invalid number of operands for \"jp.add\" operand.".to_string(),
        ));
    }
    Ok(())
}

fn validate_add_path(path: &Value) -> Result<&str, JsError> {
    match path {
        Value::String(s) => Ok(s.as_str()),
        _ => Err(JsError::Other(
            "The \"path\" argument for \"jp.add\" must be a const string.".to_string(),
        )),
    }
}

fn jp_add_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    validate_add_operand_count(expr.len())?;
    let mut doc = util::jsvalue_to_json(crate::evaluate(&expr[1], ctx)?);
    let mut i = 2;
    while i < expr.len() {
        let path_str = validate_add_path(&expr[i])?;
        i += 1;
        let value = util::jsvalue_to_json(crate::evaluate(&expr[i], ctx)?);
        i += 1;
        let path = parse_json_pointer(path_str);
        // Apply JSON Patch "add" semantics
        doc = apply_add(doc, &path, value)?;
    }
    Ok(JsValue::Json(doc))
}

/// Apply JSON Patch "add" operation to a document.
fn apply_add(mut doc: Value, path: &[String], value: Value) -> Result<Value, JsError> {
    if path.is_empty() {
        return Ok(value);
    }
    // Navigate to the parent and set the key (destination need not exist yet)
    let parent_path = &path[..path.len() - 1];
    let key = &path[path.len() - 1];
    let parent = get_mut_at(&mut doc, parent_path)?;
    match parent {
        Value::Object(obj) => {
            obj.insert(key.clone(), value);
        }
        Value::Array(arr) => {
            if key == "-" {
                arr.push(value);
            } else {
                let idx: usize = key.parse().map_err(|_| JsError::InvalidIndex)?;
                if idx > arr.len() {
                    return Err(JsError::InvalidIndex);
                }
                arr.insert(idx, value);
            }
        }
        _ => return Err(JsError::NotContainer),
    }
    Ok(doc)
}

fn get_mut_at<'a>(val: &'a mut Value, path: &[String]) -> Result<&'a mut Value, JsError> {
    let mut current = val;
    for key in path {
        current = match current {
            Value::Object(obj) => obj.get_mut(key.as_str()).ok_or(JsError::NotFound)?,
            Value::Array(arr) => {
                let idx: usize = key.parse().map_err(|_| JsError::InvalidIndex)?;
                arr.get_mut(idx).ok_or(JsError::OutOfBounds)?
            }
            _ => return Err(JsError::NotContainer),
        };
    }
    Ok(current)
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![Arc::new(OperatorDefinition {
        name: "jp.add",
        aliases: &[],
        arity: Arity::Variadic,
        eval_fn: jp_add_eval,
        impure: false,
    })]
}
