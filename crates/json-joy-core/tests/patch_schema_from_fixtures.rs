use std::fs;
use std::path::Path;

use json_joy_core::schema;
use serde_json::Value;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn from_hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex length must be even");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn read_fixture(path: &Path) -> Value {
    let data = fs::read_to_string(path).expect("fixture readable");
    serde_json::from_str(&data).expect("fixture json")
}

#[test]
fn patch_schema_fixtures_match_oracle_binary_and_metadata() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    let mut seen = 0u32;
    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("patch_schema_parity") {
            continue;
        }
        seen += 1;
        let fx = read_fixture(&dir.join(entry["file"].as_str().expect("fixture file")));
        let sid = fx["input"]["sid"].as_u64().expect("input.sid");
        let time = fx["input"]["time"].as_u64().expect("input.time");
        let value = &fx["input"]["value_json"];

        let patch = schema::json(value)
            .to_patch(sid, time)
            .expect("schema::json to_patch");
        let expected = from_hex(
            fx["expected"]["patch_binary_hex"]
                .as_str()
                .expect("expected.patch_binary_hex"),
        );
        assert_eq!(
            patch.to_binary(),
            expected,
            "schema patch binary mismatch for {}",
            fx["name"]
        );

        let expected_op_count = fx["expected"]["patch_op_count"]
            .as_u64()
            .expect("expected.patch_op_count");
        let expected_span = fx["expected"]["patch_span"]
            .as_u64()
            .expect("expected.patch_span");
        let expected_opcodes: Vec<u8> = fx["expected"]["patch_opcodes"]
            .as_array()
            .expect("expected.patch_opcodes")
            .iter()
            .map(|v| u8::try_from(v.as_u64().expect("opcode u64")).expect("opcode in range"))
            .collect();

        assert_eq!(
            patch.op_count(),
            expected_op_count,
            "op count mismatch for {}",
            fx["name"]
        );
        assert_eq!(
            patch.span(),
            expected_span,
            "span mismatch for {}",
            fx["name"]
        );
        assert_eq!(
            patch.opcodes(),
            expected_opcodes.as_slice(),
            "opcodes mismatch for {}",
            fx["name"]
        );
    }

    assert!(
        seen >= 25,
        "expected at least 25 patch_schema_parity fixtures"
    );
}
