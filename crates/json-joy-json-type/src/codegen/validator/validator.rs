//! Runtime validator — validates JSON values against TypeNode schemas.
//!
//! Upstream: ValidatorCodegen.ts (ported as runtime match dispatch, no JIT).

use serde_json::Value;

use crate::constants::ValidationError;
use crate::schema::NumFormat;
use crate::type_def::classes::*;
use crate::type_def::TypeNode;

use super::types::{ErrorMode, ValidationResult, ValidatorOptions};

/// Validate a JSON value against a TypeNode.
pub fn validate(
    value: &Value,
    type_: &TypeNode,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    validate_inner(value, type_, opts, path)
}

fn make_error(code: ValidationError, path: &[Value], opts: &ValidatorOptions) -> ValidationResult {
    match opts.errors {
        ErrorMode::Boolean => ValidationResult::BoolError,
        ErrorMode::String => {
            let mut parts: Vec<serde_json::Value> = vec![Value::String(code.name().to_string())];
            parts.extend_from_slice(path);
            ValidationResult::StringError(
                serde_json::to_string(&Value::Array(parts)).unwrap_or_default(),
            )
        }
        ErrorMode::Object => ValidationResult::ObjectError {
            code: code.name().to_string(),
            errno: code as u8,
            message: code.message().to_string(),
            path: path.to_vec(),
        },
    }
}

fn validate_inner(
    value: &Value,
    type_: &TypeNode,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    match type_ {
        TypeNode::Any(_) => ValidationResult::Ok,

        TypeNode::Bool(_) => {
            if !value.is_boolean() {
                return make_error(ValidationError::Bool, path, opts);
            }
            ValidationResult::Ok
        }

        TypeNode::Num(t) => validate_num(value, t, opts, path),

        TypeNode::Str(t) => validate_str(value, t, opts, path),

        TypeNode::Bin(t) => validate_bin(value, t, opts, path),

        TypeNode::Con(t) => {
            if !json_equal(value, &t.value) {
                return make_error(ValidationError::Const, path, opts);
            }
            // Run custom validators
            for (validator, _name) in &t.base.validators {
                if let Some(_err) = validator(value) {
                    return make_error(ValidationError::Validation, path, opts);
                }
            }
            ValidationResult::Ok
        }

        TypeNode::Arr(t) => validate_arr(value, t, opts, path),

        TypeNode::Obj(t) => validate_obj(value, t, opts, path),

        TypeNode::Map(t) => validate_map(value, t, opts, path),

        TypeNode::Ref(t) => {
            // Resolve the ref via the system module
            if let Some(system) = &t.base.system {
                match system.resolve(&t.ref_) {
                    Ok(alias) => {
                        let builder = crate::type_def::builder::TypeBuilder::new();
                        let resolved = builder.import(&alias.schema);
                        return validate_inner(value, &resolved, opts, path);
                    }
                    Err(_e) => return make_error(ValidationError::Ref, path, opts),
                }
            }
            ValidationResult::Ok
        }

        TypeNode::Or(t) => validate_or(value, t, opts, path),

        TypeNode::Fn(_) | TypeNode::FnRx(_) => ValidationResult::Ok,

        TypeNode::Key(t) => {
            let mut new_path = path.to_vec();
            new_path.push(Value::String(t.key.clone()));
            validate_inner(value, &t.val, opts, &new_path)
        }

        TypeNode::Alias(t) => validate_inner(value, &t.type_, opts, path),
    }
}

fn validate_num(
    value: &Value,
    t: &NumType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    let schema = &t.schema;

    let num = match value.as_f64() {
        Some(n) => n,
        None => return make_error(ValidationError::Num, path, opts),
    };

    if let Some(format) = schema.format {
        if format.is_integer() {
            if num.fract() != 0.0 {
                return make_error(ValidationError::Int, path, opts);
            }
            if format.is_unsigned() && num < 0.0 {
                return make_error(ValidationError::Uint, path, opts);
            }
            match format {
                NumFormat::U8 => {
                    if num > 0xFF as f64 {
                        return make_error(ValidationError::Uint, path, opts);
                    }
                }
                NumFormat::U16 => {
                    if num > 0xFFFF as f64 {
                        return make_error(ValidationError::Uint, path, opts);
                    }
                }
                NumFormat::U32 => {
                    if num > 0xFFFF_FFFFu64 as f64 {
                        return make_error(ValidationError::Uint, path, opts);
                    }
                }
                NumFormat::I8 => {
                    if !(-128.0..=127.0).contains(&num) {
                        return make_error(ValidationError::Int, path, opts);
                    }
                }
                NumFormat::I16 => {
                    if !(-32768.0..=32767.0).contains(&num) {
                        return make_error(ValidationError::Int, path, opts);
                    }
                }
                NumFormat::I32 => {
                    if !(-2147483648.0..=2147483647.0).contains(&num) {
                        return make_error(ValidationError::Int, path, opts);
                    }
                }
                // I64/U64: f64 cannot represent all i64/u64 values exactly (max safe integer
                // is 2^53-1), so boundary checks at the i64/u64 limit would be imprecise.
                // We accept any integer-valued f64 for these formats (same as upstream behavior).
                _ => {}
            }
        } else if format.is_float() && !num.is_finite() {
            return make_error(ValidationError::Num, path, opts);
        }
    }

    if let Some(gt) = schema.gt {
        if num <= gt {
            return make_error(ValidationError::Gt, path, opts);
        }
    }
    if let Some(gte) = schema.gte {
        if num < gte {
            return make_error(ValidationError::Gte, path, opts);
        }
    }
    if let Some(lt) = schema.lt {
        if num >= lt {
            return make_error(ValidationError::Lt, path, opts);
        }
    }
    if let Some(lte) = schema.lte {
        if num > lte {
            return make_error(ValidationError::Lte, path, opts);
        }
    }

    for (validator, _) in &t.base.validators {
        if validator(value).is_some() {
            return make_error(ValidationError::Validation, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_str(
    value: &Value,
    t: &StrType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    let s = match value.as_str() {
        Some(s) => s,
        None => return make_error(ValidationError::Str, path, opts),
    };
    let schema = &t.schema;
    let len = s.chars().count() as u64;
    if let Some(min) = schema.min {
        if len < min {
            return make_error(ValidationError::StrLen, path, opts);
        }
    }
    if let Some(max) = schema.max {
        if len > max {
            return make_error(ValidationError::StrLen, path, opts);
        }
    }
    // Format checks
    if let Some(fmt) = schema.format {
        match fmt {
            crate::schema::StrFormat::Ascii => {
                if !s.is_ascii() {
                    return make_error(ValidationError::Str, path, opts);
                }
            }
            crate::schema::StrFormat::Utf8 => {} // All Rust strings are valid UTF-8
        }
    } else if schema.ascii == Some(true) && !s.is_ascii() {
        return make_error(ValidationError::Str, path, opts);
    }
    for (validator, _) in &t.base.validators {
        if validator(value).is_some() {
            return make_error(ValidationError::Validation, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_bin(
    value: &Value,
    t: &BinType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    // Binary is represented as a JSON array of numbers (bytes)
    let arr = match value.as_array() {
        Some(a) => a,
        None => return make_error(ValidationError::Bin, path, opts),
    };
    if !arr
        .iter()
        .all(|v| v.as_u64().map(|n| n <= 255).unwrap_or(false))
    {
        return make_error(ValidationError::Bin, path, opts);
    }
    let schema = &t.schema;
    let len = arr.len() as u64;
    if let Some(min) = schema.min {
        if len < min {
            return make_error(ValidationError::BinLen, path, opts);
        }
    }
    if let Some(max) = schema.max {
        if len > max {
            return make_error(ValidationError::BinLen, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_arr(
    value: &Value,
    t: &ArrType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return make_error(ValidationError::Arr, path, opts),
    };

    let head_len = t.head.len();
    let tail_len = t.tail.len();

    // Validate head elements
    for (i, head_type) in t.head.iter().enumerate() {
        if i >= arr.len() {
            return make_error(ValidationError::Tup, path, opts);
        }
        let mut p = path.to_vec();
        p.push(Value::Number(i.into()));
        let r = validate_inner(&arr[i], head_type, opts, &p);
        if r.is_err() {
            return r;
        }
    }

    // Validate body elements
    if let Some(body_type) = &t.type_ {
        let schema = &t.schema;
        let body_len = arr.len().saturating_sub(head_len + tail_len);
        if let Some(min) = schema.min {
            if (body_len as u64) < min {
                return make_error(ValidationError::ArrLen, path, opts);
            }
        }
        if let Some(max) = schema.max {
            if (body_len as u64) > max {
                return make_error(ValidationError::ArrLen, path, opts);
            }
        }
        for (i, item) in arr
            .iter()
            .enumerate()
            .take(arr.len().saturating_sub(tail_len))
            .skip(head_len)
        {
            let mut p = path.to_vec();
            p.push(Value::Number(i.into()));
            let r = validate_inner(item, body_type, opts, &p);
            if r.is_err() {
                return r;
            }
        }
    }

    // Validate tail elements
    let tail_start = arr.len().saturating_sub(tail_len);
    for (i, tail_type) in t.tail.iter().enumerate() {
        let idx = tail_start + i;
        if idx >= arr.len() {
            return make_error(ValidationError::Tup, path, opts);
        }
        let mut p = path.to_vec();
        p.push(Value::Number(idx.into()));
        let r = validate_inner(&arr[idx], tail_type, opts, &p);
        if r.is_err() {
            return r;
        }
    }

    for (validator, _) in &t.base.validators {
        if validator(value).is_some() {
            return make_error(ValidationError::Validation, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_obj(
    value: &Value,
    t: &ObjType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return make_error(ValidationError::Obj, path, opts),
    };

    let schema = &t.schema;
    let check_extra = !t.keys.is_empty()
        && schema.decode_unknown_keys != Some(true)
        && !opts.skip_object_extra_fields_check;

    if check_extra {
        for key in obj.keys() {
            if !t.keys.iter().any(|k| k.key == *key) {
                let mut p = path.to_vec();
                p.push(Value::String(key.clone()));
                return make_error(ValidationError::Keys, &p, opts);
            }
        }
    }

    for field in &t.keys {
        if field.optional {
            if let Some(v) = obj.get(&field.key) {
                let mut p = path.to_vec();
                p.push(Value::String(field.key.clone()));
                let r = validate_inner(v, &field.val, opts, &p);
                if r.is_err() {
                    return r;
                }
            }
        } else {
            let v = match obj.get(&field.key) {
                Some(v) => v,
                None => {
                    let mut p = path.to_vec();
                    p.push(Value::String(field.key.clone()));
                    return make_error(ValidationError::Key, &p, opts);
                }
            };
            let mut p = path.to_vec();
            p.push(Value::String(field.key.clone()));
            let r = validate_inner(v, &field.val, opts, &p);
            if r.is_err() {
                return r;
            }
        }
    }

    for (validator, _) in &t.base.validators {
        if validator(value).is_some() {
            return make_error(ValidationError::Validation, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_map(
    value: &Value,
    t: &MapType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return make_error(ValidationError::Map, path, opts),
    };

    for (key, val) in obj {
        let mut p = path.to_vec();
        p.push(Value::String(key.clone()));
        let r = validate_inner(val, &t.value, opts, &p);
        if r.is_err() {
            return r;
        }
    }

    for (validator, _) in &t.base.validators {
        if validator(value).is_some() {
            return make_error(ValidationError::Validation, path, opts);
        }
    }
    ValidationResult::Ok
}

fn validate_or(
    value: &Value,
    t: &OrType,
    opts: &ValidatorOptions,
    path: &[Value],
) -> ValidationResult {
    if t.types.is_empty() {
        return make_error(ValidationError::Or, path, opts);
    }
    // Try each type in order — first match wins.
    // Run with the caller's opts directly: a successful result is returned as-is,
    // and we only skip to the next branch on failure.
    for type_ in &t.types {
        let r = validate_inner(value, type_, opts, path);
        if r.is_ok() {
            return r;
        }
    }
    make_error(ValidationError::Or, path, opts)
}

/// Simple deep equality check for JSON values.
pub fn json_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a
            .as_f64()
            .zip(b.as_f64())
            .map(|(a, b)| a == b)
            .unwrap_or(false),
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| json_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).map(|bv| json_equal(v, bv)).unwrap_or(false))
        }
        _ => false,
    }
}
