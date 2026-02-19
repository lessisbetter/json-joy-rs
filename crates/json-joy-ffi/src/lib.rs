use json_joy::json_crdt::codec::structural::binary as structural_binary;
use json_joy::json_crdt::model::util::random_session_id;
use json_joy::json_crdt::model::Model;
use json_joy::json_crdt::nodes::IndexExt;
use json_joy::json_crdt_diff::JsonCrdtDiff;
use json_joy::json_crdt_patch::patch::Patch;
use json_joy::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;
use serde_json::Value;

const MIN_SESSION_ID: u64 = 65_536;
const PATCH_LOG_VERSION: u8 = 1;
const MAX_PATCH_SIZE: usize = 10 * 1024 * 1024;

fn decode_model(data: &[u8], context: &str) -> Model {
    structural_binary::decode(data).unwrap_or_else(|e| panic!("{context}: {e:?}"))
}

fn is_logical_model_binary(data: &[u8]) -> bool {
    data.first().is_some_and(|b| (b & 0x80) == 0)
}

fn json_to_pack(v: &Value) -> PackValue {
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                PackValue::Float(f)
            } else {
                PackValue::Null
            }
        }
        Value::String(s) => PackValue::Str(s.clone()),
        Value::Array(_) | Value::Object(_) => PackValue::Null,
    }
}

fn build_json(builder: &mut PatchBuilder, v: &Value) -> json_joy::json_crdt_patch::clock::Ts {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => builder.con_val(json_to_pack(v)),
        Value::String(s) => {
            let str_id = builder.str_node();
            if !s.is_empty() {
                builder.ins_str(str_id, str_id, s.clone());
            }
            str_id
        }
        Value::Array(items) => {
            let arr_id = builder.arr();
            if !items.is_empty() {
                let ids = items
                    .iter()
                    .map(|item| build_json(builder, item))
                    .collect::<Vec<_>>();
                builder.ins_arr(arr_id, arr_id, ids);
            }
            arr_id
        }
        Value::Object(map) => {
            let obj_id = builder.obj();
            if !map.is_empty() {
                let entries = map
                    .iter()
                    .map(|(k, v)| (k.clone(), build_json(builder, v)))
                    .collect::<Vec<_>>();
                builder.ins_obj(obj_id, entries);
            }
            obj_id
        }
    }
}

fn serialize_patch_log(patches: &[Patch]) -> Vec<u8> {
    if patches.is_empty() {
        return Vec::new();
    }
    let binaries = patches.iter().map(Patch::to_binary).collect::<Vec<_>>();
    let total_len = 1usize + binaries.iter().map(|b| 4 + b.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total_len);
    out.push(PATCH_LOG_VERSION);
    for bin in binaries {
        out.extend_from_slice(&(bin.len() as u32).to_be_bytes());
        out.extend_from_slice(&bin);
    }
    out
}

fn deserialize_patch_log(data: &[u8]) -> Result<Vec<Patch>, String> {
    if data.is_empty() {
        return Ok(vec![]);
    }
    if data[0] != PATCH_LOG_VERSION {
        return Err(format!("unsupported patch log version: {}", data[0]));
    }
    let mut patches = Vec::new();
    let mut offset = 1usize;
    while offset < data.len() {
        if offset + 4 > data.len() {
            return Err("corrupt pending patches: truncated length header".to_string());
        }
        let len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if len > MAX_PATCH_SIZE {
            return Err(format!(
                "corrupt pending patches: patch size {} exceeds max",
                len
            ));
        }
        if len > data.len().saturating_sub(offset) {
            return Err("corrupt pending patches: truncated patch data".to_string());
        }
        let patch = Patch::from_binary(&data[offset..offset + len])
            .map_err(|e| format!("patch decode failed: {e:?}"))?;
        patches.push(patch);
        offset += len;
    }
    Ok(patches)
}

fn diff_patch_from_json(model: &Model, sid: u64, next: &Value) -> Patch {
    let mut differ = JsonCrdtDiff::new(sid, model.clock.time, &model.index);
    match IndexExt::get(&model.index, &model.root.val) {
        Some(node) => differ.diff(node, next),
        None => {
            let mut builder = PatchBuilder::new(sid, model.clock.time);
            let id = build_json(&mut builder, next);
            builder.root(id);
            builder.flush()
        }
    }
}

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

pub fn generate_session_id() -> u64 {
    let sid = random_session_id();
    sid.max(MIN_SESSION_ID)
}

pub fn is_valid_session_id(sid: u64) -> bool {
    sid >= MIN_SESSION_ID
}

pub fn compat_model_create(data_json: String, sid: u64) -> Vec<u8> {
    assert!(is_valid_session_id(sid), "compat_model_create failed");
    let value: Value = serde_json::from_str(&data_json).expect("invalid JSON input");
    let mut model = Model::new(sid);
    let mut builder = PatchBuilder::new(sid, model.clock.time);
    let root = build_json(&mut builder, &value);
    builder.root(root);
    let patch = builder.flush();
    if !patch.ops.is_empty() {
        model.apply_patch(&patch);
    }
    structural_binary::encode(&model)
}

pub fn compat_model_from_binary(model_binary: Vec<u8>) -> Vec<u8> {
    let model = decode_model(&model_binary, "compat_model_from_binary failed");
    structural_binary::encode(&model)
}

pub fn compat_model_load(model_binary: Vec<u8>, sid: u64) -> Vec<u8> {
    assert!(is_valid_session_id(sid), "compat_model_load failed");
    let mut model = decode_model(&model_binary, "compat_model_load failed");
    if is_logical_model_binary(&model_binary) {
        model.clock.sid = sid;
    }
    structural_binary::encode(&model)
}

pub fn compat_model_view(model_binary: Vec<u8>) -> String {
    let model = decode_model(&model_binary, "compat_model_view failed");
    serde_json::to_string(&model.view()).expect("serialize view json failed")
}

pub fn compat_model_diff(model_binary: Vec<u8>, sid: u64, next_json: String) -> Vec<u8> {
    assert!(is_valid_session_id(sid), "compat_model_diff load failed");
    let mut model = decode_model(&model_binary, "compat_model_diff load failed");
    if is_logical_model_binary(&model_binary) {
        model.clock.sid = sid;
    }
    let next: Value = serde_json::from_str(&next_json).expect("invalid next JSON");
    let patch = diff_patch_from_json(&model, sid, &next);
    if patch.ops.is_empty() {
        Vec::new()
    } else {
        patch.to_binary()
    }
}

pub fn compat_model_apply(model_binary: Vec<u8>, patch_binary: Vec<u8>) -> Vec<u8> {
    let mut model = decode_model(&model_binary, "compat_model_apply load failed");
    if patch_binary.is_empty() {
        return structural_binary::encode(&model);
    }
    let patch = Patch::from_binary(&patch_binary).expect("compat_model_apply failed");
    if patch.ops.is_empty() {
        return structural_binary::encode(&model);
    }
    model.apply_patch(&patch);
    structural_binary::encode(&model)
}

pub fn compat_model_fork(model_binary: Vec<u8>, sid: i64) -> Vec<u8> {
    let mut model = decode_model(&model_binary, "compat_model_fork load failed");
    let fork_sid = if sid < 0 {
        let mut generated = generate_session_id();
        while generated == model.clock.sid {
            generated = generate_session_id();
        }
        generated
    } else {
        let specified = sid as u64;
        assert!(is_valid_session_id(specified), "compat_model_fork failed");
        specified
    };
    model.clock.sid = fork_sid;
    structural_binary::encode(&model)
}

pub fn compat_patch_log_serialize(patch_binaries: Vec<Vec<u8>>) -> Vec<u8> {
    let patches = patch_binaries
        .iter()
        .map(|b| Patch::from_binary(b).expect("invalid patch binary"))
        .collect::<Vec<_>>();
    serialize_patch_log(&patches)
}

pub fn compat_patch_log_deserialize(log_binary: Vec<u8>) -> Vec<Vec<u8>> {
    deserialize_patch_log(&log_binary)
        .expect("patch log deserialize failed")
        .into_iter()
        .map(|p| p.to_binary())
        .collect()
}

pub fn compat_patch_log_append(existing: Vec<u8>, patch_binary: Vec<u8>) -> Vec<u8> {
    let patch = Patch::from_binary(&patch_binary).expect("invalid patch binary");
    let patch_bin = patch.to_binary();

    if existing.is_empty() {
        let mut out = Vec::with_capacity(1 + 4 + patch_bin.len());
        out.push(PATCH_LOG_VERSION);
        out.extend_from_slice(&(patch_bin.len() as u32).to_be_bytes());
        out.extend_from_slice(&patch_bin);
        return out;
    }

    let mut out = Vec::with_capacity(existing.len() + 4 + patch_bin.len());
    out.extend_from_slice(&existing);
    out.extend_from_slice(&(patch_bin.len() as u32).to_be_bytes());
    out.extend_from_slice(&patch_bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_compat_create_diff_apply_view_roundtrip() {
        let model = compat_model_create("{\"title\":\"a\"}".to_string(), 79501);
        let patch = compat_model_diff(model.clone(), 79501, "{\"title\":\"A\"}".to_string());
        assert!(!patch.is_empty(), "diff should produce a patch");

        let applied = compat_model_apply(model, patch);
        let view = compat_model_view(applied);
        assert_eq!(view, "{\"title\":\"A\"}");
    }

    #[test]
    fn ffi_compat_patch_log_append_and_deserialize() {
        let model = compat_model_create("{\"n\":1}".to_string(), 79502);
        let patch = compat_model_diff(model, 79502, "{\"n\":2}".to_string());
        assert!(!patch.is_empty(), "diff should produce a patch");

        let log = compat_patch_log_append(Vec::new(), patch.clone());
        let patches = compat_patch_log_deserialize(log);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0], patch);
    }
}

uniffi::include_scaffolding!("json_joy");
