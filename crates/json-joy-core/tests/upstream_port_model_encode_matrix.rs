use std::fs;
use std::path::{Path, PathBuf};

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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[test]
fn upstream_port_model_encode_inventory_from_apply_replay() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    let mut match_count = 0u32;
    let mut mismatch_count = 0u32;
    let mut mismatch_names = Vec::new();
    let mut mismatch_ids = Vec::new();

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_apply_replay") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));
        let name = fixture["name"].as_str().unwrap_or("unknown");

        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let patches = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("input.patches_binary_hex must be array")
            .iter()
            .map(|v| decode_hex(v.as_str().expect("patch hex must be string")))
            .collect::<Vec<_>>();
        let replay = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("input.replay_pattern must be array");

        let mut runtime =
            RuntimeModel::from_model_binary(&base).unwrap_or_else(|e| panic!("runtime decode failed for {name}: {e}"));
        for idx in replay {
            let i = idx.as_u64().expect("replay index must be u64") as usize;
            let patch = Patch::from_binary(&patches[i])
                .unwrap_or_else(|e| panic!("patch decode failed for {name}: {e}"));
            runtime
                .apply_patch(&patch)
                .unwrap_or_else(|e| panic!("runtime apply failed for {name}: {e}"));
        }

        let encoded = runtime
            .to_model_binary_like()
            .unwrap_or_else(|e| panic!("runtime encode failed for {name}: {e}"));
        let expected = decode_hex(
            fixture["expected"]["model_binary_hex"]
                .as_str()
                .expect("expected.model_binary_hex must be string"),
        );
        if encoded == expected {
            match_count += 1;
        } else {
            mismatch_count += 1;
            mismatch_ids.push(name.to_string());
            mismatch_names.push(format!(
                "{} expected={} actual={}",
                name,
                hex(&expected),
                hex(&encoded)
            ));
        }
    }

    eprintln!(
        "model encode inventory (apply_replay): total={}, match={}, mismatch={}",
        seen, match_count, mismatch_count
    );
    eprintln!("model encode mismatches: {}", mismatch_names.join(", "));

    assert!(seen >= 30, "expected at least 30 model_apply_replay fixtures");
    assert!(
        mismatch_ids.is_empty(),
        "expected exact binary parity for all apply_replay fixtures; mismatches: {}",
        mismatch_ids.join(", ")
    );
}

#[test]
fn upstream_port_model_encode_roundtrip_decode_matrix() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    let mut mismatches = Vec::new();
    let mut encode_error_ids = Vec::new();

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_roundtrip") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));
        let name = fixture["name"].as_str().unwrap_or("unknown");
        let expected = decode_hex(
            fixture["expected"]["model_binary_hex"]
                .as_str()
                .expect("expected.model_binary_hex must be string"),
        );

        let runtime = RuntimeModel::from_model_binary(&expected)
            .unwrap_or_else(|e| panic!("runtime decode failed for {name}: {e}"));
        let encoded = match runtime.to_model_binary_like() {
            Ok(v) => v,
            Err(e) => {
                encode_error_ids.push(name.to_string());
                eprintln!("roundtrip encode error for {name}: {e}");
                continue;
            }
        };
        if encoded != expected {
            mismatches.push(format!(
                "{} expected={} actual={}",
                name,
                hex(&expected),
                hex(&encoded)
            ));
        }
    }

    assert!(seen >= 60, "expected at least 60 model_roundtrip fixtures");
    assert!(
        encode_error_ids.is_empty(),
        "expected all roundtrip fixtures to encode successfully; errors: {}",
        encode_error_ids.join(", ")
    );
    assert!(
        mismatches.is_empty(),
        "roundtrip-decode model encode parity mismatches: {}",
        mismatches.join(", ")
    );

}
