use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    // crates/json-joy-core -> repo root -> tests/compat/fixtures
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

#[test]
fn fixture_manifest_exists_and_has_entries() {
    let dir = fixtures_dir();
    let manifest_path = dir.join("manifest.json");
    assert!(manifest_path.exists(), "missing manifest: {:?}", manifest_path);

    let manifest = read_json(&manifest_path);
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");
    assert!(!fixtures.is_empty(), "manifest.fixtures must not be empty");
    assert_eq!(
        manifest["upstream_version"].as_str(),
        Some("17.67.0"),
        "manifest upstream version must remain pinned"
    );
    let fixture_count = manifest["fixture_count"]
        .as_u64()
        .expect("manifest.fixture_count must be u64");
    assert!(
        fixture_count >= 100,
        "fixture_count must be >= 100 for broad patch/model surface coverage"
    );
}

#[test]
fn every_manifest_fixture_file_exists_and_has_required_keys() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));

    for entry in manifest["fixtures"].as_array().expect("manifest.fixtures must be array") {
        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture_path = dir.join(file);
        assert!(fixture_path.exists(), "fixture file missing: {:?}", fixture_path);

        let fixture = read_json(&fixture_path);
        assert_eq!(fixture["fixture_version"].as_i64(), Some(1));
        assert!(fixture["name"].is_string(), "fixture.name must be string");
        assert!(fixture["scenario"].is_string(), "fixture.scenario must be string");
        assert!(fixture["input"].is_object(), "fixture.input must be object");
        assert!(fixture["expected"].is_object(), "fixture.expected must be object");
        assert!(fixture["meta"].is_object(), "fixture.meta must be object");
        assert_eq!(fixture["meta"]["upstream_version"].as_str(), Some("17.67.0"));

        match fixture["scenario"].as_str().expect("scenario must be string") {
            "patch_diff_apply" => {
                assert!(
                    fixture["expected"]["patch_present"].is_boolean(),
                    "patch_diff_apply fixtures must define expected.patch_present"
                );
            }
            "patch_decode_error" => {
                assert!(
                    fixture["expected"]["error_message"].is_string(),
                    "patch_decode_error fixtures must define expected.error_message"
                );
            }
            "patch_canonical_encode" => {
                assert!(
                    fixture["input"]["ops"].is_array(),
                    "patch_canonical_encode fixtures must define input.ops"
                );
                assert!(
                    fixture["expected"]["patch_binary_hex"].is_string(),
                    "patch_canonical_encode fixtures must define expected.patch_binary_hex"
                );
                assert!(
                    fixture["expected"]["patch_opcodes"].is_array(),
                    "patch_canonical_encode fixtures must define expected.patch_opcodes"
                );
            }
            "model_roundtrip" => {
                assert!(
                    fixture["expected"]["model_binary_hex"].is_string(),
                    "model_roundtrip fixtures must define expected.model_binary_hex"
                );
                assert!(
                    !fixture["expected"]["view_json"].is_null()
                        || fixture["input"]["data"].is_null(),
                    "model_roundtrip fixtures must define expected.view_json"
                );
            }
            "model_decode_error" => {
                assert!(
                    fixture["expected"]["error_message"].is_string(),
                    "model_decode_error fixtures must define expected.error_message"
                );
            }
            "model_canonical_encode" => {
                assert!(
                    fixture["input"].is_object(),
                    "model_canonical_encode fixtures must define input object"
                );
                assert!(
                    fixture["expected"]["model_binary_hex"].is_string(),
                    "model_canonical_encode fixtures must define expected.model_binary_hex"
                );
                assert!(
                    fixture["expected"].get("view_json").is_some(),
                    "model_canonical_encode fixtures must define expected.view_json"
                );
            }
            other => panic!("unexpected fixture scenario: {other}"),
        }
    }
}

#[test]
fn manifest_contains_required_scenarios() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let has_diff_apply = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_diff_apply"));
    let has_decode_error = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_decode_error"));
    let has_model_roundtrip = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_roundtrip"));
    let has_model_decode_error = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_decode_error"));
    let has_model_canonical_encode = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_canonical_encode"));
    let model_roundtrip_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_roundtrip"))
        .count();
    let model_decode_error_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_decode_error"))
        .count();
    let model_canonical_encode_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_canonical_encode"))
        .count();
    let has_patch_canonical_encode = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_canonical_encode"));

    assert!(has_diff_apply, "fixtures must include patch_diff_apply scenarios");
    assert!(has_decode_error, "fixtures must include patch_decode_error scenarios");
    assert!(
        has_patch_canonical_encode,
        "fixtures must include patch_canonical_encode scenarios"
    );
    assert!(has_model_roundtrip, "fixtures must include model_roundtrip scenarios");
    assert!(
        has_model_decode_error,
        "fixtures must include model_decode_error scenarios"
    );
    assert!(
        has_model_canonical_encode,
        "fixtures must include model_canonical_encode scenarios"
    );
    assert!(
        model_roundtrip_count >= 60,
        "fixtures must include at least 60 model_roundtrip scenarios"
    );
    assert!(
        model_decode_error_count >= 20,
        "fixtures must include at least 20 model_decode_error scenarios"
    );
    assert!(
        model_canonical_encode_count >= 6,
        "fixtures must include at least 6 model_canonical_encode scenarios"
    );
}
