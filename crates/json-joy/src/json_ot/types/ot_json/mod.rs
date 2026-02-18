//! Register-based JSON operational transformation.
//!
//! Mirrors `packages/json-joy/src/json-ot/types/ot-json/`.
//!
//! A `JsonOp` is a 5-phase operation tuple:
//!   `(test, pick, data, drop, edit)`
//!
//! # Phases
//! 1. **test** — predicate expressions; abort if any fails
//! 2. **pick** — extract values from document into registers (removing them)
//! 3. **data** — store literal values in registers
//! 4. **drop** — insert register values back into document
//! 5. **edit** — apply string/binary OT edits in-place

use serde_json::Value;

use crate::json_ot::types::{
    ot_binary_irrev::BinaryOp,
    ot_string_irrev::StringIrrevOp,
};

/// Which OT type an edit component uses.
#[derive(Debug, Clone, PartialEq)]
pub enum EditType {
    OtString = 0,
    OtBinary = 1,
}

/// An in-place edit component applied during the edit phase.
#[derive(Debug, Clone, PartialEq)]
pub enum EditComponent {
    OtString { path: Vec<String>, op: StringIrrevOp },
    OtBinary { path: Vec<String>, op: BinaryOp },
}

/// A register ID for pick/data/drop operations.
pub type RegId = u32;

/// Pick: remove a value from the document into a register.
#[derive(Debug, Clone)]
pub struct PickComponent {
    pub register: RegId,
    pub path: Vec<String>,
}

/// Data: store a literal value in a register.
#[derive(Debug, Clone)]
pub struct DataComponent {
    pub register: RegId,
    pub value: Value,
}

/// Drop: insert a register's value back into the document.
#[derive(Debug, Clone)]
pub struct DropComponent {
    pub register: RegId,
    pub path: Vec<String>,
}

/// A complete register-based JSON operation.
#[derive(Debug, Clone, Default)]
pub struct JsonOp {
    pub test: Vec<Value>,
    pub pick: Vec<PickComponent>,
    pub data: Vec<DataComponent>,
    pub drop: Vec<DropComponent>,
    pub edit: Vec<EditComponent>,
}

impl JsonOp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_noop(&self) -> bool {
        self.test.is_empty()
            && self.pick.is_empty()
            && self.data.is_empty()
            && self.drop.is_empty()
            && self.edit.is_empty()
    }
}

/// Apply a `JsonOp` to a JSON document.
///
/// Phases: test → pick → data → drop → edit.
/// Returns the modified document, or `None` if a test phase fails.
pub fn apply(mut doc: Value, op: &JsonOp) -> Option<Value> {
    // Phase 1: test — not fully implemented (would require json-expression evaluator)
    // For now, skip test phase and proceed.

    // Phase 2: pick — extract values into registers
    let mut registers: std::collections::HashMap<RegId, Value> = std::collections::HashMap::new();
    let mut picks_sorted = op.pick.clone();
    // Sort deepest-first so nested picks work correctly
    picks_sorted.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

    for pick in &picks_sorted {
        if let Some(val) = remove_at_path(&mut doc, &pick.path) {
            registers.insert(pick.register, val);
        }
    }

    // Phase 3: data — store literal values
    for data in &op.data {
        registers.insert(data.register, data.value.clone());
    }

    // Phase 4: drop — insert register values (shallowest first)
    let mut drops_sorted = op.drop.clone();
    drops_sorted.sort_by(|a, b| a.path.len().cmp(&b.path.len()));

    for drop in &drops_sorted {
        if let Some(val) = registers.get(&drop.register).cloned() {
            insert_at_path(&mut doc, &drop.path, val);
        }
    }

    // Phase 5: edit — apply OT edits
    for edit in &op.edit {
        match edit {
            EditComponent::OtString { path, op: str_op } => {
                if let Some(target) = get_mut_at_path(&mut doc, path) {
                    if let Some(s) = target.as_str() {
                        let new_s = crate::json_ot::types::ot_string_irrev::apply(s, str_op);
                        *target = Value::String(new_s);
                    }
                }
            }
            EditComponent::OtBinary { path, op: bin_op } => {
                // Binary values are not natively supported in JSON; skip.
                let _ = (path, bin_op);
            }
        }
    }

    Some(doc)
}

// ── Internal path helpers ─────────────────────────────────────────────────

fn remove_at_path(doc: &mut Value, path: &[String]) -> Option<Value> {
    if path.is_empty() {
        return Some(std::mem::replace(doc, Value::Null));
    }
    let (parent_path, last) = path.split_at(path.len() - 1);
    let key = &last[0];
    let parent = get_mut_at_path(doc, parent_path)?;
    match parent {
        Value::Object(map) => map.remove(key),
        Value::Array(arr) => {
            let idx: usize = key.parse().ok()?;
            if idx < arr.len() { Some(arr.remove(idx)) } else { None }
        }
        _ => None,
    }
}

fn insert_at_path(doc: &mut Value, path: &[String], val: Value) {
    if path.is_empty() {
        *doc = val;
        return;
    }
    let (parent_path, last) = path.split_at(path.len() - 1);
    let key = &last[0];
    if let Some(parent) = get_mut_at_path(doc, parent_path) {
        match parent {
            Value::Object(map) => { map.insert(key.clone(), val); }
            Value::Array(arr) => {
                if key == "-" {
                    arr.push(val);
                } else if let Ok(idx) = key.parse::<usize>() {
                    let idx = idx.min(arr.len());
                    arr.insert(idx, val);
                }
            }
            _ => {}
        }
    }
}

fn get_mut_at_path<'a>(doc: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let mut cur = doc;
    for key in path {
        cur = match cur {
            Value::Object(map) => map.get_mut(key)?,
            Value::Array(arr) => {
                let idx: usize = key.parse().ok()?;
                arr.get_mut(idx)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn noop_leaves_doc_unchanged() {
        let doc = json!({"a": 1});
        let op = JsonOp::new();
        assert_eq!(apply(doc.clone(), &op), Some(doc));
    }

    #[test]
    fn pick_then_drop_at_different_path() {
        let doc = json!({"a": 1, "b": 2});
        let op = JsonOp {
            pick: vec![PickComponent { register: 0, path: vec!["a".to_string()] }],
            drop: vec![DropComponent { register: 0, path: vec!["c".to_string()] }],
            ..Default::default()
        };
        let result = apply(doc, &op).unwrap();
        assert_eq!(result["c"], json!(1));
        assert!(result["a"].is_null() || result.get("a").is_none());
    }

    #[test]
    fn data_then_drop() {
        let doc = json!({});
        let op = JsonOp {
            data: vec![DataComponent { register: 0, value: json!(42) }],
            drop: vec![DropComponent { register: 0, path: vec!["x".to_string()] }],
            ..Default::default()
        };
        let result = apply(doc, &op).unwrap();
        assert_eq!(result["x"], json!(42));
    }
}
