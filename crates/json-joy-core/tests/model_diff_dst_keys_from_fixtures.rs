use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::diff_runtime::diff_model_dst_keys_to_patch_bytes;
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
    let data =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex string must have even length"
    );
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
fn model_diff_dst_keys_fixtures_match_oracle_patch_binary_and_apply_view() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
    let mut seen = 0u32;

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_diff_dst_keys") {
            continue;
        }
        seen += 1;
        let name = entry["name"].as_str().expect("fixture name must be string");
        let file = entry["file"].as_str().expect("fixture file must be string");
        let fixture = read_json(&dir.join(file));

        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let sid = fixture["input"]["sid"]
            .as_u64()
            .expect("input.sid must be u64");
        let dst = &fixture["input"]["dst_keys_view_json"];
        let expected_present = fixture["expected"]["patch_present"]
            .as_bool()
            .expect("expected.patch_present must be bool");

        let patch = diff_model_dst_keys_to_patch_bytes(&base, dst, sid)
            .unwrap_or_else(|e| panic!("native diff-dst-keys failed for {name}: {e}"));
        assert_eq!(
            patch.is_some(),
            expected_present,
            "patch presence mismatch for fixture {name}"
        );

        if let Some(bytes) = patch {
            let expected_hex = fixture["expected"]["patch_binary_hex"]
                .as_str()
                .expect("expected.patch_binary_hex must be string when patch_present=true");
            assert_eq!(
                hex(&bytes),
                expected_hex,
                "patch bytes mismatch for fixture {name}"
            );

            let decoded = Patch::from_binary(&bytes)
                .unwrap_or_else(|e| panic!("patch decode failed for {name}: {e}"));
            let mut model = RuntimeModel::from_model_binary(&base)
                .unwrap_or_else(|e| panic!("runtime decode failed for {name}: {e}"));
            model
                .apply_patch(&decoded)
                .unwrap_or_else(|e| panic!("runtime apply failed for {name}: {e}"));
            assert_eq!(
                model.view_json(),
                fixture["expected"]["view_after_apply_json"],
                "apply-view mismatch for fixture {name}"
            );
        }
    }

    assert!(
        seen >= 20,
        "expected at least 20 model_diff_dst_keys fixtures"
    );
}
