use std::fs;
use std::path::Path;

use json_joy_core::patch::Patch;
use json_joy_core::patch_compaction::compact_patch;
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
fn patch_compaction_fixtures_match_oracle_binary() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    let mut seen = 0u32;
    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("patch_compaction_parity") {
            continue;
        }
        seen += 1;
        let fx = read_fixture(&dir.join(entry["file"].as_str().expect("fixture file")));
        let input = from_hex(
            fx["input"]["patch_binary_hex"]
                .as_str()
                .expect("input.patch_binary_hex"),
        );
        let patch = Patch::from_binary(&input).expect("decode patch");
        let compacted = compact_patch(&patch).expect("compact patch");
        let expected = from_hex(
            fx["expected"]["compacted_patch_binary_hex"]
                .as_str()
                .expect("expected.compacted_patch_binary_hex"),
        );
        assert_eq!(
            compacted.to_binary(),
            expected,
            "patch compaction mismatch for {}",
            fx["name"]
        );
    }

    assert!(
        seen >= 20,
        "expected at least 20 patch_compaction_parity fixtures"
    );
}
