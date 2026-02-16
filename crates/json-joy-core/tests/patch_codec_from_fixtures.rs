use std::fs;
use std::path::{Path, PathBuf};

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

#[test]
fn patch_diff_apply_fixtures_decode_and_roundtrip() {
    // Contract source:
    // - fixtures are generated from json-joy@17.67.0 oracle.
    // - for patch-present scenarios, Rust must decode and round-trip bytes
    //   exactly. This preserves wire compatibility while semantic decoding is
    //   still being expanded.
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("patch_diff_apply") {
            continue;
        }
        seen += 1;
        let file = entry["file"]
            .as_str()
            .expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let patch_present = fixture["expected"]["patch_present"]
            .as_bool()
            .expect("expected.patch_present must be bool");

        if !patch_present {
            continue;
        }

        let patch_hex = fixture["expected"]["patch_binary_hex"]
            .as_str()
            .expect("expected.patch_binary_hex must be string for patch_present fixtures");
        let patch_bytes = decode_hex(patch_hex);

        let patch = Patch::from_binary(&patch_bytes)
            .unwrap_or_else(|e| panic!("Patch decode failed for {}: {e}", entry["name"]));
        let roundtrip = patch.to_binary();

        let expected_op_count = fixture["expected"]["patch_op_count"]
            .as_u64()
            .unwrap_or_else(|| panic!("expected.patch_op_count must be u64 for {}", entry["name"]));
        let expected_span = fixture["expected"]["patch_span"]
            .as_u64()
            .unwrap_or_else(|| panic!("expected.patch_span must be u64 for {}", entry["name"]));
        let expected_opcodes: Vec<u8> = fixture["expected"]["patch_opcodes"]
            .as_array()
            .unwrap_or_else(|| panic!("expected.patch_opcodes must be array for {}", entry["name"]))
            .iter()
            .map(|v| {
                let n = v.as_u64().unwrap_or_else(|| {
                    panic!(
                        "expected.patch_opcodes values must be u64 for {}",
                        entry["name"]
                    )
                });
                u8::try_from(n).unwrap_or_else(|_| {
                    panic!(
                        "expected.patch_opcodes value out of range for {}",
                        entry["name"]
                    )
                })
            })
            .collect();
        let expected_sid = fixture["expected"]["patch_id_sid"]
            .as_u64()
            .unwrap_or_else(|| panic!("expected.patch_id_sid must be u64 for {}", entry["name"]));
        let expected_time = fixture["expected"]["patch_id_time"]
            .as_u64()
            .unwrap_or_else(|| panic!("expected.patch_id_time must be u64 for {}", entry["name"]));
        let expected_next_time = fixture["expected"]["patch_next_time"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!("expected.patch_next_time must be u64 for {}", entry["name"])
            });

        assert_eq!(
            roundtrip, patch_bytes,
            "Patch roundtrip mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            patch.op_count(),
            expected_op_count,
            "Patch op_count mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            patch.span(),
            expected_span,
            "Patch span mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            patch.opcodes(),
            expected_opcodes.as_slice(),
            "Patch opcode sequence mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            patch.id(),
            Some((expected_sid, expected_time)),
            "Patch id mismatch for fixture {}",
            entry["name"]
        );
        assert_eq!(
            patch.next_time(),
            expected_next_time,
            "Patch next_time mismatch for fixture {}",
            entry["name"]
        );
    }

    assert!(seen >= 30, "expected at least 30 patch_diff_apply fixtures");
}

#[test]
fn patch_decode_error_fixtures_reject_binary_when_oracle_rejects() {
    // Deliberately fixture-driven:
    // upstream decoder accepts many malformed payloads, so we align behavior to
    // oracle expectations per fixture instead of applying stricter local rules.
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("patch_decode_error") {
            continue;
        }
        seen += 1;

        let file = entry["file"]
            .as_str()
            .expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let patch_hex = fixture["input"]["patch_binary_hex"]
            .as_str()
            .expect("input.patch_binary_hex must be string");
        let oracle_error = fixture["expected"]["error_message"]
            .as_str()
            .expect("expected.error_message must be string");

        let bytes = decode_hex(patch_hex);
        let decoded = Patch::from_binary(&bytes);

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

    assert!(
        seen >= 10,
        "expected at least 10 patch_decode_error fixtures"
    );
}
