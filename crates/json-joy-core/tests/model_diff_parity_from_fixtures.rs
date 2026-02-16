use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
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

fn load_diff_fixtures() -> Vec<(String, Value)> {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let mut out = Vec::new();
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_diff_parity") {
            continue;
        }
        let name = entry["name"]
            .as_str()
            .expect("fixture entry name must be string");
        let file = entry["file"]
            .as_str()
            .expect("fixture entry file must be string");
        out.push((name.to_string(), read_json(&dir.join(file))));
    }
    out
}

#[test]
fn model_diff_parity_fixtures_match_oracle_patch_binary() {
    let fixtures = load_diff_fixtures();
    assert!(
        fixtures.len() >= 100,
        "expected at least 100 model_diff_parity fixtures"
    );

    for (name, fixture) in fixtures {
        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let sid = fixture["input"]["sid"]
            .as_u64()
            .expect("input.sid must be u64");
        let next_view = &fixture["input"]["next_view_json"];

        let generated = diff_model_to_patch_bytes(&base_bytes, next_view, sid)
            .unwrap_or_else(|e| panic!("diff runtime failed for {name}: {e}"));

        let expected_patch_present = fixture["expected"]["patch_present"]
            .as_bool()
            .expect("expected.patch_present must be bool");

        if !expected_patch_present {
            assert!(generated.is_none(), "expected no patch for fixture {name}");
            continue;
        }

        let generated = generated.expect("expected Some(patch bytes)");
        let expected = decode_hex(
            fixture["expected"]["patch_binary_hex"]
                .as_str()
                .expect("expected.patch_binary_hex must be string"),
        );

        assert_eq!(
            generated, expected,
            "patch bytes mismatch for fixture {name}"
        );

        let patch = Patch::from_binary(&generated)
            .unwrap_or_else(|e| panic!("generated patch decode failed for {name}: {e}"));

        let expected_op_count = fixture["expected"]["patch_op_count"]
            .as_u64()
            .expect("expected.patch_op_count must be u64");
        let expected_span = fixture["expected"]["patch_span"]
            .as_u64()
            .expect("expected.patch_span must be u64");
        let expected_sid = fixture["expected"]["patch_id_sid"]
            .as_u64()
            .expect("expected.patch_id_sid must be u64");
        let expected_time = fixture["expected"]["patch_id_time"]
            .as_u64()
            .expect("expected.patch_id_time must be u64");
        let expected_next_time = fixture["expected"]["patch_next_time"]
            .as_u64()
            .expect("expected.patch_next_time must be u64");
        let expected_opcodes: Vec<u8> = fixture["expected"]["patch_opcodes"]
            .as_array()
            .expect("expected.patch_opcodes must be array")
            .iter()
            .map(|v| {
                let n = v.as_u64().expect("patch opcode must be u64");
                u8::try_from(n).expect("patch opcode out of range")
            })
            .collect();

        assert_eq!(
            patch.op_count(),
            expected_op_count,
            "op_count mismatch for fixture {name}"
        );
        assert_eq!(
            patch.span(),
            expected_span,
            "span mismatch for fixture {name}"
        );
        assert_eq!(
            patch.id(),
            Some((expected_sid, expected_time)),
            "id mismatch for fixture {name}"
        );
        assert_eq!(
            patch.next_time(),
            expected_next_time,
            "next_time mismatch for fixture {name}"
        );
        assert_eq!(
            patch.opcodes(),
            expected_opcodes.as_slice(),
            "opcodes mismatch for fixture {name}"
        );
    }
}

#[test]
fn model_diff_parity_apply_matches_oracle_view() {
    let fixtures = load_diff_fixtures();

    for (name, fixture) in fixtures {
        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let sid = fixture["input"]["sid"]
            .as_u64()
            .expect("input.sid must be u64");
        let next_view = &fixture["input"]["next_view_json"];

        let generated = diff_model_to_patch_bytes(&base_bytes, next_view, sid)
            .unwrap_or_else(|e| panic!("diff runtime failed for {name}: {e}"));

        let mut runtime = RuntimeModel::from_model_binary(&base_bytes)
            .unwrap_or_else(|e| panic!("runtime decode failed for {name}: {e}"));

        if let Some(patch_bytes) = generated {
            let patch = Patch::from_binary(&patch_bytes)
                .unwrap_or_else(|e| panic!("generated patch decode failed for {name}: {e}"));
            runtime
                .apply_patch(&patch)
                .unwrap_or_else(|e| panic!("runtime apply failed for {name}: {e}"));
        }

        assert_eq!(
            runtime.view_json(),
            fixture["expected"]["view_after_apply_json"],
            "view_after_apply mismatch for fixture {name}"
        );
    }
}

#[test]
fn model_diff_noop_fixtures_return_none() {
    let fixtures = load_diff_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        let expected_patch_present = fixture["expected"]["patch_present"]
            .as_bool()
            .expect("expected.patch_present must be bool");
        if expected_patch_present {
            continue;
        }
        seen += 1;

        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let sid = fixture["input"]["sid"]
            .as_u64()
            .expect("input.sid must be u64");
        let next_view = &fixture["input"]["next_view_json"];

        let generated = diff_model_to_patch_bytes(&base_bytes, next_view, sid)
            .unwrap_or_else(|e| panic!("diff runtime failed for {name}: {e}"));

        assert!(
            generated.is_none(),
            "expected no patch for no-op fixture {name}"
        );
    }

    assert!(seen >= 5, "expected at least 5 no-op diff fixtures");
}
