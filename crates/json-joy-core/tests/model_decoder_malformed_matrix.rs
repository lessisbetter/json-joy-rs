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
fn decode_error_fixture_matrix_includes_required_classes() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let required = [
        "model_decode_error_clock_offset_overflow_v1",
        "model_decode_error_clock_offset_truncated_v1",
        "model_decode_error_clock_table_len_zero_v1",
        "model_decode_error_clock_table_short_tuple_v1",
        "model_decode_error_clock_table_bad_varint_v1",
        "model_decode_error_trunc_con_v1",
        "model_decode_error_trunc_obj_v1",
        "model_decode_error_trunc_vec_v1",
        "model_decode_error_trunc_str_v1",
        "model_decode_error_trunc_bin_v1",
        "model_decode_error_trunc_arr_v1",
        "model_decode_error_server_bad_preamble_v1",
        "model_decode_error_server_trunc_time_v1",
        "model_decode_error_mixed_server_logical_v1",
    ];

    for req in required {
        let present = fixtures.iter().any(|f| f["name"].as_str() == Some(req));
        assert!(present, "missing required decode-error fixture: {req}");
    }
}

#[test]
fn decode_error_matrix_matches_oracle_per_fixture() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_decode_error") {
            continue;
        }

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));
        let model_hex = fixture["input"]["model_binary_hex"]
            .as_str()
            .expect("input.model_binary_hex must be string");
        let oracle_error = fixture["expected"]["error_message"]
            .as_str()
            .expect("expected.error_message must be string");

        let decoded = Model::from_binary(&decode_hex(model_hex));
        if oracle_error == "NO_ERROR" {
            assert!(decoded.is_ok(), "oracle accepted fixture {}", entry["name"]);
        } else {
            assert!(decoded.is_err(), "oracle rejected fixture {}", entry["name"]);
        }
    }
}
