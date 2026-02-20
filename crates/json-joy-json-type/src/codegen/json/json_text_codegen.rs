//! Runtime port of `JsonTextCodegen`.
//!
//! Upstream reference: `json-type/src/codegen/json/JsonTextCodegen.ts`.
//!
//! Rust divergence:
//! - Uses recursive runtime encoding instead of generating JS source at runtime.
//! - JS-side `Value<T>` wrappers are not represented by `serde_json::Value`.
//! - Object unknown-key emission keeps valid JSON when there are no declared keys.

use std::collections::HashSet;

use json_joy_json_pack::{json_binary, PackValue};
use serde_json::{Number, Value};

use crate::codegen::discriminator::DiscriminatorCodegen;
use crate::type_def::{ObjType, OrType, TypeBuilder, TypeNode};

pub type JsonEncoderFn = Box<dyn Fn(&Value) -> Result<String, String> + Send + Sync>;

pub struct JsonTextCodegen;

impl JsonTextCodegen {
    pub fn get(type_: TypeNode) -> JsonEncoderFn {
        Box::new(move |value: &Value| {
            let mut out = String::new();
            encode_node(value, &type_, &mut out)?;
            Ok(out)
        })
    }
}

fn encode_node(value: &Value, type_: &TypeNode, out: &mut String) -> Result<(), String> {
    match type_ {
        TypeNode::Any(_) => {
            out.push_str(&stringify_any(value)?);
            Ok(())
        }
        TypeNode::Bool(_) => {
            out.push_str(if is_truthy(value) { "true" } else { "false" });
            Ok(())
        }
        TypeNode::Num(_) => {
            out.push_str(&coerce_to_string(value));
            Ok(())
        }
        TypeNode::Str(t) => {
            let s = coerce_to_string(value);
            if t.schema.no_json_escape == Some(true) {
                out.push('"');
                out.push_str(&s);
                out.push('"');
            } else {
                out.push_str(&serde_json::to_string(&s).map_err(|e| e.to_string())?);
            }
            Ok(())
        }
        TypeNode::Bin(_) => encode_bin(value, out),
        TypeNode::Con(t) => {
            out.push_str(&serde_json::to_string(t.literal()).map_err(|e| e.to_string())?);
            Ok(())
        }
        TypeNode::Arr(t) => encode_arr(value, t, out),
        TypeNode::Obj(t) => encode_obj(value, t, out),
        TypeNode::Map(t) => {
            out.push('{');
            if let Some(obj) = value.as_object() {
                for (i, (key, item)) in obj.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(key).map_err(|e| e.to_string())?);
                    out.push(':');
                    encode_node(item, &t.value, out)?;
                }
            }
            out.push('}');
            Ok(())
        }
        TypeNode::Ref(t) => {
            let system = t
                .base
                .system
                .as_ref()
                .ok_or_else(|| "NO_SYSTEM".to_string())?;
            let alias = system.resolve(&t.ref_).map_err(|e| e.to_string())?;
            let resolved = TypeBuilder::with_system(system.clone()).import(&alias.schema);
            encode_node(value, &resolved, out)
        }
        TypeNode::Or(t) => encode_or(value, t, out),
        TypeNode::Alias(t) => encode_node(value, &t.type_, out),
        TypeNode::Key(t) => encode_node(value, &t.val, out),
        TypeNode::Fn(_) | TypeNode::FnRx(_) => {
            out.push_str(&stringify_any(value)?);
            Ok(())
        }
    }
}

fn encode_bin(value: &Value, out: &mut String) -> Result<(), String> {
    let mut bytes = Vec::new();
    if let Some(arr) = value.as_array() {
        bytes.reserve(arr.len());
        for item in arr {
            let n = item
                .as_u64()
                .filter(|n| *n <= 255)
                .ok_or_else(|| "BIN_EXPECTS_BYTE_ARRAY".to_string())?;
            bytes.push(n as u8);
        }
    }
    out.push('"');
    out.push_str(&json_binary::stringify_binary(&bytes));
    out.push('"');
    Ok(())
}

fn encode_arr(value: &Value, t: &crate::type_def::ArrType, out: &mut String) -> Result<(), String> {
    let arr: &[Value] = value.as_array().map(Vec::as_slice).unwrap_or(&[]);
    out.push('[');
    for (i, item) in arr.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if let Some(item_type) = array_item_type(t, i, arr.len()) {
            encode_node(item, item_type, out)?;
        } else {
            out.push_str(&stringify_any(item)?);
        }
    }
    out.push(']');
    Ok(())
}

fn array_item_type<'a>(
    t: &'a crate::type_def::ArrType,
    index: usize,
    len: usize,
) -> Option<&'a TypeNode> {
    if let Some(body) = t.type_.as_deref() {
        return Some(body);
    }
    if index < t.head.len() {
        return t.head.get(index);
    }
    let tail_len = t.tail.len();
    if tail_len > 0 && index >= len.saturating_sub(tail_len) {
        let tail_index = index.saturating_sub(len.saturating_sub(tail_len));
        return t.tail.get(tail_index);
    }
    None
}

fn encode_obj(value: &Value, t: &ObjType, out: &mut String) -> Result<(), String> {
    let obj = value.as_object();
    let mut known_keys: HashSet<String> = HashSet::new();
    let mut wrote_any = false;

    out.push('{');

    for field in t.keys.iter().filter(|field| !field.optional) {
        if wrote_any {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(&field.key).map_err(|e| e.to_string())?);
        out.push(':');
        let field_value = obj.and_then(|o| o.get(&field.key)).unwrap_or(&Value::Null);
        encode_node(field_value, &field.val, out)?;
        known_keys.insert(field.key.clone());
        wrote_any = true;
    }

    for field in t.keys.iter().filter(|field| field.optional) {
        known_keys.insert(field.key.clone());
        if let Some(field_value) = obj.and_then(|o| o.get(&field.key)) {
            if wrote_any {
                out.push(',');
            }
            out.push_str(&serde_json::to_string(&field.key).map_err(|e| e.to_string())?);
            out.push(':');
            encode_node(field_value, &field.val, out)?;
            wrote_any = true;
        }
    }

    if t.schema.encode_unknown_keys == Some(true) {
        if let Some(map) = obj {
            for (key, item) in map {
                if known_keys.contains(key) {
                    continue;
                }
                if wrote_any {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).map_err(|e| e.to_string())?);
                out.push(':');
                out.push_str(&stringify_any(item)?);
                wrote_any = true;
            }
        }
    }

    out.push('}');
    Ok(())
}

fn encode_or(value: &Value, t: &OrType, out: &mut String) -> Result<(), String> {
    if t.types.is_empty() {
        return Ok(());
    }
    if t.types.len() == 1 {
        return encode_node(value, &t.types[0], out);
    }

    let discriminator = DiscriminatorCodegen::get(t)?;
    let index = discriminator(value)?;
    if let Some(child) = t.types.get(index as usize) {
        encode_node(value, child, out)?;
    }
    Ok(())
}

fn stringify_any(value: &Value) -> Result<String, String> {
    json_binary::stringify(PackValue::from(value.clone())).map_err(|e| e.to_string())
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => !is_zero_number(n),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn is_zero_number(num: &Number) -> bool {
    if let Some(i) = num.as_i64() {
        return i == 0;
    }
    if let Some(u) = num.as_u64() {
        return u == 0;
    }
    if let Some(f) = num.as_f64() {
        return f == 0.0;
    }
    false
}

fn coerce_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}
