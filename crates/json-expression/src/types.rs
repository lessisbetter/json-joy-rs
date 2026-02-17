use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Represents any JavaScript value, including `undefined` (no JSON equivalent)
/// and binary data (`Uint8Array` equivalent).
#[derive(Debug, Clone)]
pub enum JsValue {
    /// JavaScript `undefined`.
    Undefined,
    /// Any JSON-compatible value.
    Json(Value),
    /// Binary data (`Uint8Array` equivalent).
    Binary(Vec<u8>),
}

impl PartialEq for JsValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (JsValue::Undefined, JsValue::Undefined) => true,
            (JsValue::Json(a), JsValue::Json(b)) => a == b,
            (JsValue::Binary(a), JsValue::Binary(b)) => a == b,
            _ => false,
        }
    }
}

impl From<Value> for JsValue {
    fn from(v: Value) -> Self {
        JsValue::Json(v)
    }
}

impl From<bool> for JsValue {
    fn from(b: bool) -> Self {
        JsValue::Json(Value::Bool(b))
    }
}

impl From<f64> for JsValue {
    fn from(n: f64) -> Self {
        JsValue::Json(serde_json::json!(n))
    }
}

impl From<i64> for JsValue {
    fn from(n: i64) -> Self {
        JsValue::Json(Value::Number(serde_json::Number::from(n)))
    }
}

impl From<String> for JsValue {
    fn from(s: String) -> Self {
        JsValue::Json(Value::String(s))
    }
}

impl From<&str> for JsValue {
    fn from(s: &str) -> Self {
        JsValue::Json(Value::String(s.to_string()))
    }
}

/// Operator arity.
#[derive(Debug, Clone, PartialEq)]
pub enum Arity {
    /// Arity 0: skip arity check.
    Any,
    /// Fixed arity: exactly `n` operands.
    Fixed(usize),
    /// Variadic: at least 2 operands (arity = -1).
    Variadic,
    /// Range: between `min` and `max` operands. `None` for max = unlimited.
    Range(usize, Option<usize>),
}

/// The type of an operator evaluation function.
///
/// `expr` is the full expression array (including the operator name at index 0).
/// Operands are at `expr[1..]`.
pub type EvalFn = for<'a> fn(&[Value], &mut EvalCtx<'a>) -> Result<JsValue, JsError>;

/// An operator definition, mirroring upstream's `OperatorDefinition` tuple.
pub struct OperatorDefinition {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub arity: Arity,
    pub eval_fn: EvalFn,
    pub impure: bool,
}

/// Map of operator name/alias -> definition.
pub type OperatorMap = HashMap<String, Arc<OperatorDefinition>>;

/// Asserts that an expression has the correct arity.
pub fn assert_arity(operator: &str, arity: &Arity, expr_len: usize) -> Result<(), JsError> {
    match arity {
        Arity::Any => Ok(()),
        Arity::Fixed(n) => {
            if expr_len != n + 1 {
                Err(JsError::ArityError(format!(
                    "\"{}\" operator expects {} operands.",
                    operator, n
                )))
            } else {
                Ok(())
            }
        }
        Arity::Variadic => {
            if expr_len < 3 {
                Err(JsError::ArityError(format!(
                    "\"{}\" operator expects at least two operands.",
                    operator
                )))
            } else {
                Ok(())
            }
        }
        Arity::Range(min, max) => {
            if expr_len < min + 1 {
                Err(JsError::ArityError(format!(
                    "\"{}\" operator expects at least {} operands.",
                    operator, min
                )))
            } else if let Some(max) = max {
                if expr_len > max + 1 {
                    return Err(JsError::ArityError(format!(
                        "\"{}\" operator expects at most {} operands.",
                        operator, max
                    )));
                }
                Ok(())
            } else {
                Ok(())
            }
        }
    }
}

/// Builds an `OperatorMap` from a list of operator definitions.
pub fn operators_to_map(operators: Vec<Arc<OperatorDefinition>>) -> OperatorMap {
    let mut map = HashMap::new();
    for op in operators {
        map.insert(op.name.to_string(), Arc::clone(&op));
        for alias in op.aliases {
            map.insert(alias.to_string(), Arc::clone(&op));
        }
    }
    map
}
