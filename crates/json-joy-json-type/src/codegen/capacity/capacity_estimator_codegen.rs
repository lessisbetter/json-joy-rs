//! Runtime port of `CapacityEstimatorCodegen`.
//!
//! Upstream reference: `json-type/src/codegen/capacity/CapacityEstimatorCodegen.ts`.
//!
//! Rust divergence:
//! - Uses a recursive runtime estimator instead of generating JS source at runtime.
//! - Input is `serde_json::Value`; JS-side `Value<T>` wrapper objects are not represented.

use json_joy_util::json_size::MaxEncodingOverhead;
use json_joy_util::max_encoding_capacity;
use serde_json::Value;

use crate::codegen::discriminator::DiscriminatorCodegen;
use crate::type_def::{TypeBuilder, TypeNode};

pub type CompiledCapacityEstimator = Box<dyn Fn(&Value) -> usize + Send + Sync>;

pub struct CapacityEstimatorCodegen;

impl CapacityEstimatorCodegen {
    pub fn get(type_: TypeNode) -> CompiledCapacityEstimator {
        Box::new(move |value: &Value| estimate_node(value, &type_))
    }
}

fn estimate_node(value: &Value, type_: &TypeNode) -> usize {
    match type_ {
        TypeNode::Any(_) => max_encoding_capacity(value),
        TypeNode::Con(t) => max_encoding_capacity(t.literal()),
        TypeNode::Bool(_) => MaxEncodingOverhead::BOOLEAN,
        TypeNode::Num(_) => MaxEncodingOverhead::NUMBER,
        TypeNode::Str(_) => {
            let len = value.as_str().map(str::len).unwrap_or(0);
            MaxEncodingOverhead::STRING + MaxEncodingOverhead::STRING_LENGTH_MULTIPLIER * len
        }
        TypeNode::Bin(_) => {
            let len = value.as_array().map(Vec::len).unwrap_or(0);
            MaxEncodingOverhead::BINARY + MaxEncodingOverhead::BINARY_LENGTH_MULTIPLIER * len
        }
        TypeNode::Arr(t) => estimate_arr(value, t),
        TypeNode::Obj(t) => estimate_obj(value, t),
        TypeNode::Key(t) => estimate_node(value, &t.val),
        TypeNode::Map(t) => estimate_map(value, &t.value),
        TypeNode::Ref(t) => {
            let system = t.base.system.as_ref().expect("NO_SYSTEM");
            let alias = system.resolve(&t.ref_).expect("REF_RESOLVE_FAILED");
            let resolved = TypeBuilder::new().import(&alias.schema);
            estimate_node(value, &resolved)
        }
        TypeNode::Or(t) => estimate_or(value, t),
        TypeNode::Alias(t) => estimate_node(value, &t.type_),
        TypeNode::Fn(_) | TypeNode::FnRx(_) => max_encoding_capacity(value),
    }
}

fn estimate_arr(value: &Value, t: &crate::type_def::ArrType) -> usize {
    let arr: &[Value] = value.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let arr_len = arr.len();
    let head_len = t.head.len();
    let tail_len = t.tail.len();

    let mut size = MaxEncodingOverhead::ARRAY + MaxEncodingOverhead::ARRAY_ELEMENT * arr_len;

    if let Some(body_type) = &t.type_ {
        let body_len = arr_len.saturating_sub(head_len + tail_len);
        match body_type.as_ref() {
            TypeNode::Con(c) => size += body_len * max_encoding_capacity(c.literal()),
            TypeNode::Bool(_) => size += body_len * MaxEncodingOverhead::BOOLEAN,
            TypeNode::Num(_) => size += body_len * MaxEncodingOverhead::NUMBER,
            _ => {
                let start = head_len.min(arr_len);
                let end = arr_len.saturating_sub(tail_len);
                for item in arr.iter().take(end).skip(start) {
                    size += estimate_node(item, body_type);
                }
            }
        }
    }

    for (i, head_type) in t.head.iter().enumerate() {
        if let Some(item) = arr.get(i) {
            size += estimate_node(item, head_type);
        }
    }

    if tail_len > 0 {
        for (i, tail_type) in t.tail.iter().enumerate() {
            let idx = arr_len.saturating_sub(tail_len) + i;
            if let Some(item) = arr.get(idx) {
                size += estimate_node(item, tail_type);
            }
        }
    }

    size
}

fn estimate_obj(value: &Value, t: &crate::type_def::ObjType) -> usize {
    if t.schema.encode_unknown_keys == Some(true) {
        return max_encoding_capacity(value);
    }

    let obj = match value.as_object() {
        Some(obj) => obj,
        None => return MaxEncodingOverhead::OBJECT,
    };

    let mut size = MaxEncodingOverhead::OBJECT;
    for field in &t.keys {
        match obj.get(&field.key) {
            Some(field_value) => {
                size += MaxEncodingOverhead::OBJECT_ELEMENT;
                size += max_encoding_capacity(&Value::String(field.key.clone()));
                size += estimate_node(field_value, &field.val);
            }
            None if field.optional => {}
            None => {
                size += MaxEncodingOverhead::OBJECT_ELEMENT;
                size += max_encoding_capacity(&Value::String(field.key.clone()));
            }
        }
    }
    size
}

fn estimate_map(value: &Value, value_type: &TypeNode) -> usize {
    let obj = match value.as_object() {
        Some(obj) => obj,
        None => return MaxEncodingOverhead::OBJECT,
    };

    let mut size = MaxEncodingOverhead::OBJECT + MaxEncodingOverhead::OBJECT_ELEMENT * obj.len();
    for (key, value) in obj {
        size += MaxEncodingOverhead::STRING + MaxEncodingOverhead::STRING_LENGTH_MULTIPLIER * key.len();
        size += estimate_node(value, value_type);
    }
    size
}

fn estimate_or(value: &Value, t: &crate::type_def::OrType) -> usize {
    if t.types.is_empty() {
        return 0;
    }
    if t.types.len() == 1 {
        return estimate_node(value, &t.types[0]);
    }

    let Ok(discriminator) = DiscriminatorCodegen::get(t) else {
        return 0;
    };
    let Ok(index) = discriminator(value) else {
        return 0;
    };
    let Some(ty) = t.types.get(index as usize) else {
        return 0;
    };
    estimate_node(value, ty)
}
