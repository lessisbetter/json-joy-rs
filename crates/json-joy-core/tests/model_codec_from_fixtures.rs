use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::model::Model;
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

#[test]
fn model_roundtrip_fixtures_decode_view_and_roundtrip_binary() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_roundtrip") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let model_hex = fixture["expected"]["model_binary_hex"]
            .as_str()
            .expect("expected.model_binary_hex must be string");
        let model_bytes = decode_hex(model_hex);

        let model = Model::from_binary(&model_bytes)
            .unwrap_or_else(|e| panic!("Model decode failed for {}: {e}", entry["name"]));
        let expected_view = &fixture["expected"]["view_json"];

        assert_eq!(
            model.to_binary(),
            model_bytes,
            "Model roundtrip mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            model.view(),
            expected_view,
            "Model view mismatch for fixture {}",
            entry["name"]
        );
    }

    assert!(seen >= 60, "expected at least 60 model_roundtrip fixtures");
}

#[test]
fn model_decode_error_fixtures_reject_binary_when_oracle_rejects() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_decode_error") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let model_hex = fixture["input"]["model_binary_hex"]
            .as_str()
            .expect("input.model_binary_hex must be string");
        let oracle_error = fixture["expected"]["error_message"]
            .as_str()
            .expect("expected.error_message must be string");

        let bytes = decode_hex(model_hex);
        let decoded = Model::from_binary(&bytes);

        if oracle_error == "NO_ERROR" {
            assert!(
                decoded.is_ok(),
                "oracle accepted binary but Rust rejected fixture {}",
                entry["name"]
            );
        } else {
            assert!(
                decoded.is_err(),
                "oracle rejected binary but Rust accepted fixture {}",
                entry["name"]
            );
        }
    }

    assert!(seen >= 20, "expected at least 20 model_decode_error fixtures");
}
