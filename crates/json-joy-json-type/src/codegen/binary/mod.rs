//! Binary codegen adapters (CBOR, MessagePack, JSON text bytes).
//!
//! Upstream reference:
//! - `json-type/src/codegen/binary/`
//!
//! This port keeps upstream naming and exposes runtime encoder builders backed
//! by the Rust `json-pack` encoders.

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use crate::codegen::discriminator::DiscriminatorCodegen;
use crate::codegen::json::{JsonTextCodegen, JsonTextCodegenError};
use crate::codegen::validator::{validate, ErrorMode, ValidatorOptions};
use crate::type_def::{ArrType, MapType, ObjType, OrType, TypeBuilder, TypeNode};
use json_joy_json_pack::cbor::CborEncoder;
use json_joy_json_pack::msgpack::MsgPackEncoderFast;
use json_joy_json_pack::PackValue;

/// A compiled binary encoder function.
pub type BinaryEncoderFn = Arc<dyn Fn(&Value) -> Result<Vec<u8>, BinaryCodegenError> + Send + Sync>;

/// Binary codegen errors.
#[derive(Debug, Error)]
pub enum BinaryCodegenError {
    #[error("{0}")]
    JsonText(#[from] JsonTextCodegenError),
    #[error("Failed to resolve reference: {0}")]
    ResolveRef(String),
}

/// Runtime equivalent of upstream `CborCodegen`.
pub struct CborCodegen;

impl CborCodegen {
    pub fn get(type_: &TypeNode) -> BinaryEncoderFn {
        let type_ = type_.clone();
        Arc::new(move |value: &Value| {
            let pack = to_pack_value(&type_, value)?;
            let mut encoder = CborEncoder::new();
            Ok(encoder.encode(&pack))
        })
    }
}

/// Runtime equivalent of upstream `MsgPackCodegen`.
pub struct MsgPackCodegen;

impl MsgPackCodegen {
    pub fn get(type_: &TypeNode) -> BinaryEncoderFn {
        let type_ = type_.clone();
        Arc::new(move |value: &Value| {
            let pack = to_pack_value(&type_, value)?;
            let mut encoder = MsgPackEncoderFast::new();
            Ok(encoder.encode(&pack))
        })
    }
}

/// Runtime equivalent of upstream `JsonCodegen`.
pub struct JsonCodegen;

impl JsonCodegen {
    pub fn get(type_: &TypeNode) -> BinaryEncoderFn {
        let encode_json = JsonTextCodegen::get(type_);
        Arc::new(move |value: &Value| {
            let text = encode_json(value)?;
            Ok(text.into_bytes())
        })
    }
}

fn to_pack_value(type_: &TypeNode, value: &Value) -> Result<PackValue, BinaryCodegenError> {
    match type_ {
        TypeNode::Any(_) => Ok(PackValue::from(value.clone())),
        TypeNode::Bool(_) => Ok(PackValue::Bool(js_truthy(value))),
        TypeNode::Num(_) => Ok(js_number(value)),
        TypeNode::Str(_) => Ok(PackValue::Str(js_to_string(value))),
        TypeNode::Bin(_) => Ok(PackValue::Bytes(to_bytes(value))),
        TypeNode::Con(t) => Ok(PackValue::from(t.literal().clone())),
        TypeNode::Arr(t) => to_pack_arr(t, value),
        TypeNode::Obj(t) => to_pack_obj(t, value),
        TypeNode::Map(t) => to_pack_map(t, value),
        TypeNode::Ref(t) => {
            let Some(system) = &t.base.system else {
                return Err(BinaryCodegenError::ResolveRef("NO_SYSTEM".to_string()));
            };
            let alias = system
                .resolve(&t.ref_)
                .map_err(BinaryCodegenError::ResolveRef)?;
            let builder = TypeBuilder::with_system(Arc::clone(system));
            let resolved = builder.import(&alias.schema);
            to_pack_value(&resolved, value)
        }
        TypeNode::Or(t) => to_pack_or(t, value),
        TypeNode::Fn(_) | TypeNode::FnRx(_) => Ok(PackValue::Null),
        TypeNode::Key(t) => to_pack_value(&t.val, value),
        TypeNode::Alias(t) => to_pack_value(&t.type_, value),
    }
}

fn to_pack_arr(t: &ArrType, value: &Value) -> Result<PackValue, BinaryCodegenError> {
    let Some(items) = value.as_array() else {
        return Ok(PackValue::Array(Vec::new()));
    };

    let mut out: Vec<PackValue> = Vec::with_capacity(items.len());
    let len = items.len();
    let tail_len = t.tail.len();

    for (i, item) in items.iter().enumerate() {
        if i < t.head.len() {
            out.push(to_pack_value(&t.head[i], item)?);
            continue;
        }
        if tail_len > 0 && i >= len.saturating_sub(tail_len) {
            let tail_index = i - (len - tail_len);
            out.push(to_pack_value(&t.tail[tail_index], item)?);
            continue;
        }
        if let Some(body) = &t.type_ {
            out.push(to_pack_value(body, item)?);
        } else {
            out.push(PackValue::from(item.clone()));
        }
    }

    Ok(PackValue::Array(out))
}

fn to_pack_obj(t: &ObjType, value: &Value) -> Result<PackValue, BinaryCodegenError> {
    let map = value.as_object();
    let mut out: Vec<(String, PackValue)> = Vec::new();
    let mut known_keys: HashSet<&str> = HashSet::new();

    for field in &t.keys {
        known_keys.insert(field.key.as_str());
        if field.optional {
            let Some(obj) = map else {
                continue;
            };
            let Some(field_value) = obj.get(&field.key) else {
                continue;
            };
            out.push((field.key.clone(), to_pack_value(&field.val, field_value)?));
            continue;
        }

        let field_value = map
            .and_then(|obj| obj.get(&field.key))
            .unwrap_or(&Value::Null);
        out.push((field.key.clone(), to_pack_value(&field.val, field_value)?));
    }

    if t.schema.encode_unknown_keys == Some(true) {
        if let Some(obj) = map {
            for (key, val) in obj {
                if known_keys.contains(key.as_str()) {
                    continue;
                }
                out.push((key.clone(), PackValue::from(val.clone())));
            }
        }
    }

    Ok(PackValue::Object(out))
}

fn to_pack_map(t: &MapType, value: &Value) -> Result<PackValue, BinaryCodegenError> {
    let Some(map) = value.as_object() else {
        return Ok(PackValue::Object(Vec::new()));
    };
    let mut out: Vec<(String, PackValue)> = Vec::with_capacity(map.len());
    for (key, val) in map {
        out.push((key.clone(), to_pack_value(&t.value, val)?));
    }
    Ok(PackValue::Object(out))
}

fn to_pack_or(t: &OrType, value: &Value) -> Result<PackValue, BinaryCodegenError> {
    let index = if let Ok(discriminator) = DiscriminatorCodegen::get(t) {
        let idx = discriminator(value);
        if idx >= 0 && (idx as usize) < t.types.len() {
            idx as usize
        } else {
            first_matching_or_index(t, value)
        }
    } else {
        first_matching_or_index(t, value)
    };
    to_pack_value(&t.types[index], value)
}

fn first_matching_or_index(t: &OrType, value: &Value) -> usize {
    let opts = ValidatorOptions {
        errors: ErrorMode::Boolean,
        ..Default::default()
    };
    for (i, child) in t.types.iter().enumerate() {
        if validate(value, child, &opts, &[]).is_ok() {
            return i;
        }
    }
    0
}

fn to_bytes(value: &Value) -> Vec<u8> {
    value.as_array().map_or_else(Vec::new, |items| {
        items
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()).unwrap_or(0))
            .collect()
    })
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn js_number(value: &Value) -> PackValue {
    let n = match value {
        Value::Null => 0.0,
        Value::Bool(true) => 1.0,
        Value::Bool(false) => 0.0,
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        Value::Array(_) | Value::Object(_) => 0.0,
    };
    if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        PackValue::Integer(n as i64)
    } else {
        PackValue::Float(n)
    }
}

fn js_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}
