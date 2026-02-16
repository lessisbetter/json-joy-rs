use std::fs;
use std::path::Path;

use json_joy_core::patch::Patch;
use json_joy_core::patch_compact_binary_codec::{
    decode_patch_compact_binary, encode_patch_compact_binary,
};
use json_joy_core::patch_compact_codec::{decode_patch_compact, encode_patch_compact};
use json_joy_core::patch_verbose_codec::{decode_patch_verbose, encode_patch_verbose};
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
fn patch_alt_codecs_fixtures_match_oracle_outputs() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    let mut seen = 0u32;
    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("patch_alt_codecs") {
            continue;
        }
        seen += 1;
        let fx = read_fixture(&dir.join(entry["file"].as_str().expect("fixture file")));
        let patch_binary = from_hex(
            fx["input"]["patch_binary_hex"]
                .as_str()
                .expect("input.patch_binary_hex"),
        );
        let patch = Patch::from_binary(&patch_binary).expect("decode patch");

        let compact = encode_patch_compact(&patch).expect("encode compact");
        assert_eq!(
            compact, fx["expected"]["compact_json"],
            "compact json mismatch for {}",
            fx["name"]
        );
        let compact_roundtrip = decode_patch_compact(&compact).expect("decode compact");
        assert_eq!(
            compact_roundtrip.to_binary(),
            patch_binary,
            "compact roundtrip mismatch for {}",
            fx["name"]
        );

        let verbose = encode_patch_verbose(&patch).expect("encode verbose");
        assert_eq!(
            verbose, fx["expected"]["verbose_json"],
            "verbose json mismatch for {}",
            fx["name"]
        );
        let verbose_roundtrip = decode_patch_verbose(&verbose).expect("decode verbose");
        assert_eq!(
            verbose_roundtrip.to_binary(),
            patch_binary,
            "verbose roundtrip mismatch for {}",
            fx["name"]
        );

        let compact_binary = encode_patch_compact_binary(&patch).expect("encode compact binary");
        let expected_compact_binary = from_hex(
            fx["expected"]["compact_binary_hex"]
                .as_str()
                .expect("expected.compact_binary_hex"),
        );
        assert_eq!(
            compact_binary, expected_compact_binary,
            "compact binary mismatch for {}",
            fx["name"]
        );
        let compact_binary_roundtrip =
            decode_patch_compact_binary(&compact_binary).expect("decode compact binary");
        assert_eq!(
            compact_binary_roundtrip.to_binary(),
            patch_binary,
            "compact binary roundtrip mismatch for {}",
            fx["name"]
        );
    }

    assert!(seen >= 20, "expected at least 20 patch_alt_codecs fixtures");
}
