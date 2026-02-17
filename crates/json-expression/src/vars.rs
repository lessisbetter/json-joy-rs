use crate::error::JsError;
use crate::types::JsValue;
use json_joy_json_pointer::{get, parse_json_pointer, validate_json_pointer};
use serde_json::Value;
use std::collections::HashMap;

/// Variable store — mirrors upstream `Vars` class.
///
/// Holds the root environment value and named variable bindings.
pub struct Vars {
    /// The root "environment" value (accessed via empty-string name or pointer starting with `/`).
    pub env: JsValue,
    /// Named variable bindings.
    vars: HashMap<String, JsValue>,
}

impl Vars {
    pub fn new(env: Value) -> Self {
        Vars {
            env: JsValue::Json(env),
            vars: HashMap::new(),
        }
    }

    /// Returns the value for `name`. Returns `JsValue::Undefined` if not found.
    ///
    /// Empty string returns `self.env`.
    pub fn get(&self, name: &str) -> JsValue {
        if name.is_empty() {
            return self.env.clone();
        }
        self.vars.get(name).cloned().unwrap_or(JsValue::Undefined)
    }

    /// Sets a named variable. Panics/errors if name is empty.
    pub fn set(&mut self, name: &str, value: JsValue) -> Result<(), JsError> {
        if name.is_empty() {
            return Err(JsError::InvalidVarname);
        }
        self.vars.insert(name.to_string(), value);
        Ok(())
    }

    /// Returns true if the variable is defined (empty string → always true).
    pub fn has(&self, name: &str) -> bool {
        if name.is_empty() {
            return true;
        }
        self.vars.contains_key(name)
    }

    /// Deletes a named variable. Returns false if name is empty (error in TS).
    pub fn del(&mut self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        self.vars.remove(name).is_some()
    }

    /// Resolves a variable name and JSON Pointer to a value.
    ///
    /// Mirrors upstream `find(name, pointer)`.
    pub fn find(&self, name: &str, pointer: &str) -> Result<JsValue, JsError> {
        let data = self.get(name);
        validate_json_pointer(pointer).map_err(|e| JsError::Other(e.to_string()))?;
        let path = parse_json_pointer(pointer);
        match &data {
            JsValue::Json(val) => {
                let result = get(val, &path);
                Ok(result.map(|v| JsValue::Json(v.clone())).unwrap_or(JsValue::Undefined))
            }
            _ => Ok(JsValue::Undefined),
        }
    }
}
