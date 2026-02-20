#![allow(dead_code)]

use json_joy::json_crdt_patch::enums::JsonCrdtPatchOpcode;
use json_joy::json_crdt_patch::operations::Op;
use serde_json::Value;

pub fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("hex length must be even".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn op_to_opcode(op: &Op) -> u8 {
    match op {
        Op::NewCon { .. } => JsonCrdtPatchOpcode::NewCon as u8,
        Op::NewVal { .. } => JsonCrdtPatchOpcode::NewVal as u8,
        Op::NewObj { .. } => JsonCrdtPatchOpcode::NewObj as u8,
        Op::NewVec { .. } => JsonCrdtPatchOpcode::NewVec as u8,
        Op::NewStr { .. } => JsonCrdtPatchOpcode::NewStr as u8,
        Op::NewBin { .. } => JsonCrdtPatchOpcode::NewBin as u8,
        Op::NewArr { .. } => JsonCrdtPatchOpcode::NewArr as u8,
        Op::InsVal { .. } => JsonCrdtPatchOpcode::InsVal as u8,
        Op::InsObj { .. } => JsonCrdtPatchOpcode::InsObj as u8,
        Op::InsVec { .. } => JsonCrdtPatchOpcode::InsVec as u8,
        Op::InsStr { .. } => JsonCrdtPatchOpcode::InsStr as u8,
        Op::InsBin { .. } => JsonCrdtPatchOpcode::InsBin as u8,
        Op::InsArr { .. } => JsonCrdtPatchOpcode::InsArr as u8,
        Op::UpdArr { .. } => JsonCrdtPatchOpcode::UpdArr as u8,
        Op::Del { .. } => JsonCrdtPatchOpcode::Del as u8,
        Op::Nop { .. } => JsonCrdtPatchOpcode::Nop as u8,
    }
}

pub fn compare_expected_fields(expected: &Value, actual: &Value) -> Vec<String> {
    let mut diffs = Vec::<String>::new();
    compare_value("expected", expected, actual, &mut diffs);
    diffs
}

fn compare_value(path: &str, expected: &Value, actual: &Value, diffs: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(eobj), Value::Object(aobj)) => {
            for (k, ev) in eobj {
                let child_path = format!("{path}.{k}");
                match aobj.get(k) {
                    Some(av) => compare_field(&child_path, k, ev, av, diffs),
                    None => diffs.push(format!("missing field {child_path}")),
                }
            }
        }
        _ => {
            if expected != actual {
                diffs.push(format!("{path}: expected {expected:?}, got {actual:?}"));
            }
        }
    }
}

fn compare_field(path: &str, key: &str, expected: &Value, actual: &Value, diffs: &mut Vec<String>) {
    if key.ends_with("_hex") {
        if let (Value::Object(eobj), Value::Object(aobj)) = (expected, actual) {
            for (child_key, ev) in eobj {
                let child_path = format!("{path}.{child_key}");
                match aobj.get(child_key) {
                    Some(av) => compare_field(&child_path, child_key, ev, av, diffs),
                    None => diffs.push(format!("missing field {child_path}")),
                }
            }
            return;
        }
        let ehex = expected.as_str();
        let ahex = actual.as_str();
        match (ehex, ahex) {
            (Some(e), Some(a)) => match (decode_hex(e), decode_hex(a)) {
                (Ok(eb), Ok(ab)) => {
                    if eb != ab {
                        diffs.push(format!("{path}: hex bytes differ"));
                    }
                }
                (Err(err), _) => diffs.push(format!("{path}: invalid expected hex: {err}")),
                (_, Err(err)) => diffs.push(format!("{path}: invalid actual hex: {err}")),
            },
            _ => diffs.push(format!("{path}: expected both values to be hex strings")),
        }
        return;
    }

    if key.ends_with("_json") {
        if expected != actual {
            diffs.push(format!("{path}: json mismatch"));
        }
        return;
    }

    match (expected, actual) {
        (Value::Object(_), Value::Object(_)) => compare_value(path, expected, actual, diffs),
        _ => {
            if expected != actual {
                diffs.push(format!("{path}: expected {expected:?}, got {actual:?}"));
            }
        }
    }
}
