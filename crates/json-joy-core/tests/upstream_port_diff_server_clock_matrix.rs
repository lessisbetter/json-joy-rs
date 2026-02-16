use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::diff_runtime::{diff_model_to_patch_bytes, DiffError};
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::Patch;
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn read_json(path: &Path) -> Value {
    let data = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex string must have even length");
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}

fn mutate_view(v: &Value) -> Value {
    match v {
        Value::Null => Value::Bool(true),
        Value::Bool(b) => Value::Bool(!b),
        Value::Number(_) => Value::String("server-mut".to_owned()),
        Value::String(s) => Value::String(format!("{s}_m")),
        Value::Array(arr) => {
            let mut out = arr.clone();
            out.push(Value::String("m".to_owned()));
            Value::Array(out)
        }
        Value::Object(map) => {
            let mut out = map.clone();
            out.insert("__m".to_owned(), Value::Number(1u64.into()));
            Value::Object(out)
        }
    }
}

#[test]
fn upstream_port_diff_server_clock_matrix_avoids_unsupported_shape() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_canonical_encode") {
            continue;
        }
        let file = entry["file"].as_str().expect("fixture file must be string");
        let fixture = read_json(&dir.join(file));
        if fixture["input"]["mode"].as_str() != Some("server") {
            continue;
        }
        seen += 1;
        let base_binary = decode_hex(
            fixture["expected"]["model_binary_hex"]
                .as_str()
                .expect("expected.model_binary_hex must be string"),
        );
        let base_view = fixture["expected"]["view_json"].clone();
        let next_view = mutate_view(&base_view);

        let patch_bytes = match diff_model_to_patch_bytes(&base_binary, &next_view, 770_001) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => panic!("server diff produced no-op for mutated view {}", fixture["name"]),
            Err(DiffError::UnsupportedShape) => {
                panic!("server diff hit UnsupportedShape for {}", fixture["name"])
            }
            Err(e) => panic!("server diff error for {}: {e}", fixture["name"]),
        };

        let patch = Patch::from_binary(&patch_bytes)
            .unwrap_or_else(|e| panic!("diff patch decode failed for {}: {e}", fixture["name"]));
        let mut runtime = RuntimeModel::from_model_binary(&base_binary)
            .unwrap_or_else(|e| panic!("runtime decode failed for {}: {e}", fixture["name"]));
        runtime
            .apply_patch(&patch)
            .unwrap_or_else(|e| panic!("runtime apply failed for {}: {e}", fixture["name"]));
        assert_eq!(
            runtime.view_json(),
            next_view,
            "server diff apply target mismatch for {}",
            fixture["name"]
        );
    }

    assert!(seen >= 4, "expected at least 4 server canonical fixtures");
}
