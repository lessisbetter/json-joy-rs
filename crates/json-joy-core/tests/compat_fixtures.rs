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
    let data =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

#[test]
fn fixture_manifest_exists_and_has_entries() {
    let dir = fixtures_dir();
    let manifest_path = dir.join("manifest.json");
    assert!(
        manifest_path.exists(),
        "missing manifest: {:?}",
        manifest_path
    );

    let manifest = read_json(&manifest_path);
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
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

    for entry in manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array")
    {
        let file = entry["file"]
            .as_str()
            .expect("fixture entry file must be string");
        let fixture_path = dir.join(file);
        assert!(
            fixture_path.exists(),
            "fixture file missing: {:?}",
            fixture_path
        );

        let fixture = read_json(&fixture_path);
        assert_eq!(fixture["fixture_version"].as_i64(), Some(1));
        assert!(fixture["name"].is_string(), "fixture.name must be string");
        assert!(
            fixture["scenario"].is_string(),
            "fixture.scenario must be string"
        );
        assert!(fixture["input"].is_object(), "fixture.input must be object");
        assert!(
            fixture["expected"].is_object(),
            "fixture.expected must be object"
        );
        assert!(fixture["meta"].is_object(), "fixture.meta must be object");
        assert_eq!(
            fixture["meta"]["upstream_version"].as_str(),
            Some("17.67.0")
        );

        match fixture["scenario"]
            .as_str()
            .expect("scenario must be string")
        {
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
            "patch_alt_codecs" => {
                assert!(
                    fixture["input"]["patch_binary_hex"].is_string(),
                    "patch_alt_codecs fixtures must define input.patch_binary_hex"
                );
                assert!(
                    fixture["expected"]["compact_json"].is_array(),
                    "patch_alt_codecs fixtures must define expected.compact_json"
                );
                assert!(
                    fixture["expected"]["verbose_json"].is_object(),
                    "patch_alt_codecs fixtures must define expected.verbose_json"
                );
                assert!(
                    fixture["expected"]["compact_binary_hex"].is_string(),
                    "patch_alt_codecs fixtures must define expected.compact_binary_hex"
                );
            }
            "patch_compaction_parity" => {
                assert!(
                    fixture["input"]["patch_binary_hex"].is_string(),
                    "patch_compaction_parity fixtures must define input.patch_binary_hex"
                );
                assert!(
                    fixture["expected"]["compacted_patch_binary_hex"].is_string(),
                    "patch_compaction_parity fixtures must define expected.compacted_patch_binary_hex"
                );
                assert!(
                    fixture["expected"]["changed"].is_boolean(),
                    "patch_compaction_parity fixtures must define expected.changed"
                );
            }
            "patch_schema_parity" => {
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "patch_schema_parity fixtures must define input.sid"
                );
                assert!(
                    fixture["input"]["time"].is_u64(),
                    "patch_schema_parity fixtures must define input.time"
                );
                assert!(
                    fixture["input"].get("value_json").is_some(),
                    "patch_schema_parity fixtures must define input.value_json"
                );
                assert!(
                    fixture["expected"]["patch_binary_hex"].is_string(),
                    "patch_schema_parity fixtures must define expected.patch_binary_hex"
                );
                assert!(
                    fixture["expected"]["patch_opcodes"].is_array(),
                    "patch_schema_parity fixtures must define expected.patch_opcodes"
                );
                assert!(
                    fixture["expected"]["patch_op_count"].is_u64(),
                    "patch_schema_parity fixtures must define expected.patch_op_count"
                );
                assert!(
                    fixture["expected"]["patch_span"].is_u64(),
                    "patch_schema_parity fixtures must define expected.patch_span"
                );
            }
            "util_diff_parity" => {
                assert!(
                    fixture["input"]["kind"].is_string(),
                    "util_diff_parity fixtures must define input.kind"
                );
                assert!(
                    fixture["input"].get("src").is_some(),
                    "util_diff_parity fixtures must define input.src"
                );
                assert!(
                    fixture["input"].get("dst").is_some(),
                    "util_diff_parity fixtures must define input.dst"
                );
                assert!(
                    fixture["expected"]["patch"].is_array(),
                    "util_diff_parity fixtures must define expected.patch"
                );
                match fixture["input"]["kind"].as_str() {
                    Some("str") | Some("bin") => {
                        assert!(
                            fixture["expected"].get("src_from_patch").is_some(),
                            "util_diff_parity str/bin fixtures must define expected.src_from_patch"
                        );
                        assert!(
                            fixture["expected"].get("dst_from_patch").is_some(),
                            "util_diff_parity str/bin fixtures must define expected.dst_from_patch"
                        );
                    }
                    Some("line") => {}
                    _ => panic!("util_diff_parity fixtures kind must be one of str|bin|line"),
                }
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
            "model_apply_replay" => {
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_apply_replay fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["patches_binary_hex"].is_array(),
                    "model_apply_replay fixtures must define input.patches_binary_hex"
                );
                assert!(
                    fixture["input"]["replay_pattern"].is_array(),
                    "model_apply_replay fixtures must define input.replay_pattern"
                );
                assert!(
                    fixture["expected"].get("view_json").is_some(),
                    "model_apply_replay fixtures must define expected.view_json"
                );
            }
            "model_diff_parity" => {
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_diff_parity fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["next_view_json"].is_object()
                        || fixture["input"]["next_view_json"].is_array()
                        || fixture["input"]["next_view_json"].is_string()
                        || fixture["input"]["next_view_json"].is_number()
                        || fixture["input"]["next_view_json"].is_boolean()
                        || fixture["input"]["next_view_json"].is_null(),
                    "model_diff_parity fixtures must define input.next_view_json"
                );
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "model_diff_parity fixtures must define input.sid"
                );
                assert!(
                    fixture["expected"]["patch_present"].is_boolean(),
                    "model_diff_parity fixtures must define expected.patch_present"
                );
                if fixture["expected"]["patch_present"].as_bool() == Some(true) {
                    assert!(
                        fixture["expected"]["patch_binary_hex"].is_string(),
                        "model_diff_parity patch fixtures must define expected.patch_binary_hex"
                    );
                    assert!(
                        fixture["expected"]["patch_opcodes"].is_array(),
                        "model_diff_parity patch fixtures must define expected.patch_opcodes"
                    );
                    assert!(
                        fixture["expected"]["patch_op_count"].is_u64(),
                        "model_diff_parity patch fixtures must define expected.patch_op_count"
                    );
                    assert!(
                        fixture["expected"]["patch_span"].is_u64(),
                        "model_diff_parity patch fixtures must define expected.patch_span"
                    );
                    assert!(
                        fixture["expected"]["patch_id_sid"].is_u64(),
                        "model_diff_parity patch fixtures must define expected.patch_id_sid"
                    );
                    assert!(
                        fixture["expected"]["patch_id_time"].is_u64(),
                        "model_diff_parity patch fixtures must define expected.patch_id_time"
                    );
                    assert!(
                        fixture["expected"]["patch_next_time"].is_u64(),
                        "model_diff_parity patch fixtures must define expected.patch_next_time"
                    );
                }
                assert!(
                    fixture["expected"].get("view_after_apply_json").is_some(),
                    "model_diff_parity fixtures must define expected.view_after_apply_json"
                );
            }
            "model_diff_dst_keys" => {
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_diff_dst_keys fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["dst_keys_view_json"].is_object(),
                    "model_diff_dst_keys fixtures must define input.dst_keys_view_json"
                );
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "model_diff_dst_keys fixtures must define input.sid"
                );
                assert!(
                    fixture["expected"]["patch_present"].is_boolean(),
                    "model_diff_dst_keys fixtures must define expected.patch_present"
                );
                if fixture["expected"]["patch_present"].as_bool() == Some(true) {
                    assert!(
                        fixture["expected"]["patch_binary_hex"].is_string(),
                        "model_diff_dst_keys patch fixtures must define expected.patch_binary_hex"
                    );
                }
                assert!(
                    fixture["expected"].get("view_after_apply_json").is_some(),
                    "model_diff_dst_keys fixtures must define expected.view_after_apply_json"
                );
            }
            "lessdb_model_manager" => {
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "lessdb_model_manager fixtures must define input.sid"
                );
                assert!(
                    fixture["input"]["ops"].is_array(),
                    "lessdb_model_manager fixtures must define input.ops"
                );
                assert!(
                    fixture["expected"]["steps"].is_array(),
                    "lessdb_model_manager fixtures must define expected.steps"
                );
                assert!(
                    fixture["expected"].get("final_view_json").is_some(),
                    "lessdb_model_manager fixtures must define expected.final_view_json"
                );
                assert!(
                    fixture["expected"]["final_model_binary_hex"].is_string(),
                    "lessdb_model_manager fixtures must define expected.final_model_binary_hex"
                );
            }
            "model_api_workflow" => {
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "model_api_workflow fixtures must define input.sid"
                );
                assert!(
                    fixture["input"]["initial_json"].is_object()
                        || fixture["input"]["initial_json"].is_array()
                        || fixture["input"]["initial_json"].is_string()
                        || fixture["input"]["initial_json"].is_number()
                        || fixture["input"]["initial_json"].is_boolean()
                        || fixture["input"]["initial_json"].is_null(),
                    "model_api_workflow fixtures must define input.initial_json"
                );
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_api_workflow fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["ops"].is_array(),
                    "model_api_workflow fixtures must define input.ops"
                );
                assert!(
                    fixture["expected"]["steps"].is_array(),
                    "model_api_workflow fixtures must define expected.steps"
                );
                assert!(
                    fixture["expected"].get("final_view_json").is_some(),
                    "model_api_workflow fixtures must define expected.final_view_json"
                );
                assert!(
                    fixture["expected"]["final_model_binary_hex"].is_string(),
                    "model_api_workflow fixtures must define expected.final_model_binary_hex"
                );
            }
            "model_api_proxy_fanout_workflow" => {
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "model_api_proxy_fanout_workflow fixtures must define input.sid"
                );
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_api_proxy_fanout_workflow fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["scoped_path"].is_array(),
                    "model_api_proxy_fanout_workflow fixtures must define input.scoped_path"
                );
                assert!(
                    fixture["input"]["ops"].is_array(),
                    "model_api_proxy_fanout_workflow fixtures must define input.ops"
                );
                assert!(
                    fixture["expected"]["steps"].is_array(),
                    "model_api_proxy_fanout_workflow fixtures must define expected.steps"
                );
                assert!(
                    fixture["expected"].get("final_view_json").is_some(),
                    "model_api_proxy_fanout_workflow fixtures must define expected.final_view_json"
                );
                assert!(
                    fixture["expected"]["final_model_binary_hex"].is_string(),
                    "model_api_proxy_fanout_workflow fixtures must define expected.final_model_binary_hex"
                );
                assert!(
                    fixture["expected"]["fanout"]["change_count"].is_u64(),
                    "model_api_proxy_fanout_workflow fixtures must define expected.fanout.change_count"
                );
                assert!(
                    fixture["expected"]["fanout"]["scoped_count"].is_u64(),
                    "model_api_proxy_fanout_workflow fixtures must define expected.fanout.scoped_count"
                );
            }
            "model_lifecycle_workflow" => {
                assert!(
                    fixture["input"]["workflow"].is_string(),
                    "model_lifecycle_workflow fixtures must define input.workflow"
                );
                assert!(
                    fixture["input"]["sid"].is_u64(),
                    "model_lifecycle_workflow fixtures must define input.sid"
                );
                assert!(
                    fixture["input"]["base_model_binary_hex"].is_string(),
                    "model_lifecycle_workflow fixtures must define input.base_model_binary_hex"
                );
                assert!(
                    fixture["input"]["seed_patches_binary_hex"].is_array(),
                    "model_lifecycle_workflow fixtures must define input.seed_patches_binary_hex"
                );
                assert!(
                    fixture["input"]["batch_patches_binary_hex"].is_array(),
                    "model_lifecycle_workflow fixtures must define input.batch_patches_binary_hex"
                );
                assert!(
                    fixture["expected"].get("final_view_json").is_some(),
                    "model_lifecycle_workflow fixtures must define expected.final_view_json"
                );
                assert!(
                    fixture["expected"]["final_model_binary_hex"].is_string(),
                    "model_lifecycle_workflow fixtures must define expected.final_model_binary_hex"
                );
            }
            "codec_indexed_binary_parity" => {
                assert!(
                    fixture["input"]["model_binary_hex"].is_string(),
                    "codec_indexed_binary_parity fixtures must define input.model_binary_hex"
                );
                assert!(
                    fixture["expected"]["fields_hex"].is_object(),
                    "codec_indexed_binary_parity fixtures must define expected.fields_hex"
                );
                assert!(
                    fixture["expected"]["fields_roundtrip_hex"].is_object(),
                    "codec_indexed_binary_parity fixtures must define expected.fields_roundtrip_hex"
                );
                assert!(
                    fixture["expected"].get("view_json").is_some(),
                    "codec_indexed_binary_parity fixtures must define expected.view_json"
                );
                assert!(
                    fixture["expected"]["model_binary_hex"].is_string(),
                    "codec_indexed_binary_parity fixtures must define expected.model_binary_hex"
                );
            }
            "codec_sidecar_binary_parity" => {
                assert!(
                    fixture["input"]["model_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define input.model_binary_hex"
                );
                assert!(
                    fixture["expected"]["view_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define expected.view_binary_hex"
                );
                assert!(
                    fixture["expected"]["meta_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define expected.meta_binary_hex"
                );
                assert!(
                    fixture["expected"]["view_roundtrip_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define expected.view_roundtrip_binary_hex"
                );
                assert!(
                    fixture["expected"]["meta_roundtrip_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define expected.meta_roundtrip_binary_hex"
                );
                assert!(
                    fixture["expected"].get("view_json").is_some(),
                    "codec_sidecar_binary_parity fixtures must define expected.view_json"
                );
                assert!(
                    fixture["expected"]["model_binary_hex"].is_string(),
                    "codec_sidecar_binary_parity fixtures must define expected.model_binary_hex"
                );
            }
            "patch_clock_codec_parity" => {
                assert!(
                    fixture["input"]["model_binary_hex"].is_string(),
                    "patch_clock_codec_parity fixtures must define input.model_binary_hex"
                );
                assert!(
                    fixture["expected"]["clock_table_binary_hex"].is_string(),
                    "patch_clock_codec_parity fixtures must define expected.clock_table_binary_hex"
                );
                assert!(
                    fixture["expected"]["clock_table"].is_array(),
                    "patch_clock_codec_parity fixtures must define expected.clock_table"
                );
                assert!(
                    fixture["expected"]["relative_ids"].is_array(),
                    "patch_clock_codec_parity fixtures must define expected.relative_ids"
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
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let has_diff_apply = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_diff_apply"));
    let has_decode_error = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_decode_error"));
    let has_patch_alt_codecs = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_alt_codecs"));
    let has_patch_compaction_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_compaction_parity"));
    let has_patch_schema_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_schema_parity"));
    let has_util_diff_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("util_diff_parity"));
    let has_model_roundtrip = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_roundtrip"));
    let has_model_decode_error = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_decode_error"));
    let has_model_canonical_encode = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_canonical_encode"));
    let has_model_apply_replay = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_apply_replay"));
    let has_model_diff_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_diff_parity"));
    let has_model_diff_dst_keys = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_diff_dst_keys"));
    let has_lessdb_model_manager = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("lessdb_model_manager"));
    let has_model_api_workflow = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_api_workflow"));
    let has_model_api_proxy_fanout_workflow = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_api_proxy_fanout_workflow"));
    let has_model_lifecycle_workflow = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("model_lifecycle_workflow"));
    let has_codec_indexed_binary_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("codec_indexed_binary_parity"));
    let has_codec_sidecar_binary_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("codec_sidecar_binary_parity"));
    let has_patch_clock_codec_parity = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_clock_codec_parity"));
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
    let model_apply_replay_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_apply_replay"))
        .count();
    let model_diff_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_diff_parity"))
        .count();
    let model_diff_dst_keys_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_diff_dst_keys"))
        .count();
    let lessdb_model_manager_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("lessdb_model_manager"))
        .count();
    let model_api_workflow_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_api_workflow"))
        .count();
    let model_api_proxy_fanout_workflow_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_api_proxy_fanout_workflow"))
        .count();
    let model_lifecycle_workflow_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("model_lifecycle_workflow"))
        .count();
    let codec_indexed_binary_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("codec_indexed_binary_parity"))
        .count();
    let codec_sidecar_binary_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("codec_sidecar_binary_parity"))
        .count();
    let patch_clock_codec_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_clock_codec_parity"))
        .count();
    let patch_decode_error_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_decode_error"))
        .count();
    let patch_alt_codecs_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_alt_codecs"))
        .count();
    let patch_compaction_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_compaction_parity"))
        .count();
    let patch_schema_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_schema_parity"))
        .count();
    let util_diff_parity_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("util_diff_parity"))
        .count();
    let patch_canonical_encode_count = fixtures
        .iter()
        .filter(|f| f["scenario"].as_str() == Some("patch_canonical_encode"))
        .count();
    let has_patch_canonical_encode = fixtures
        .iter()
        .any(|f| f["scenario"].as_str() == Some("patch_canonical_encode"));

    assert!(
        has_diff_apply,
        "fixtures must include patch_diff_apply scenarios"
    );
    assert!(
        has_decode_error,
        "fixtures must include patch_decode_error scenarios"
    );
    assert!(
        has_patch_alt_codecs,
        "fixtures must include patch_alt_codecs scenarios"
    );
    assert!(
        has_patch_compaction_parity,
        "fixtures must include patch_compaction_parity scenarios"
    );
    assert!(
        has_patch_schema_parity,
        "fixtures must include patch_schema_parity scenarios"
    );
    assert!(
        has_util_diff_parity,
        "fixtures must include util_diff_parity scenarios"
    );
    assert!(
        has_patch_canonical_encode,
        "fixtures must include patch_canonical_encode scenarios"
    );
    assert!(
        has_model_roundtrip,
        "fixtures must include model_roundtrip scenarios"
    );
    assert!(
        has_model_decode_error,
        "fixtures must include model_decode_error scenarios"
    );
    assert!(
        has_model_canonical_encode,
        "fixtures must include model_canonical_encode scenarios"
    );
    assert!(
        has_model_apply_replay,
        "fixtures must include model_apply_replay scenarios"
    );
    assert!(
        has_model_diff_parity,
        "fixtures must include model_diff_parity scenarios"
    );
    assert!(
        has_model_diff_dst_keys,
        "fixtures must include model_diff_dst_keys scenarios"
    );
    assert!(
        has_lessdb_model_manager,
        "fixtures must include lessdb_model_manager scenarios"
    );
    assert!(
        has_model_api_workflow,
        "fixtures must include model_api_workflow scenarios"
    );
    assert!(
        has_model_api_proxy_fanout_workflow,
        "fixtures must include model_api_proxy_fanout_workflow scenarios"
    );
    assert!(
        has_model_lifecycle_workflow,
        "fixtures must include model_lifecycle_workflow scenarios"
    );
    assert!(
        has_codec_indexed_binary_parity,
        "fixtures must include codec_indexed_binary_parity scenarios"
    );
    assert!(
        has_codec_sidecar_binary_parity,
        "fixtures must include codec_sidecar_binary_parity scenarios"
    );
    assert!(
        has_patch_clock_codec_parity,
        "fixtures must include patch_clock_codec_parity scenarios"
    );
    assert!(
        patch_decode_error_count >= 35,
        "fixtures must include at least 35 patch_decode_error scenarios"
    );
    assert!(
        patch_alt_codecs_count >= 40,
        "fixtures must include at least 40 patch_alt_codecs scenarios"
    );
    assert!(
        patch_compaction_parity_count >= 40,
        "fixtures must include at least 40 patch_compaction_parity scenarios"
    );
    assert!(
        patch_schema_parity_count >= 45,
        "fixtures must include at least 45 patch_schema_parity scenarios"
    );
    assert!(
        util_diff_parity_count >= 80,
        "fixtures must include at least 80 util_diff_parity scenarios"
    );
    assert!(
        patch_canonical_encode_count >= 40,
        "fixtures must include at least 40 patch_canonical_encode scenarios"
    );
    assert!(
        model_roundtrip_count >= 110,
        "fixtures must include at least 110 model_roundtrip scenarios"
    );
    assert!(
        model_decode_error_count >= 35,
        "fixtures must include at least 35 model_decode_error scenarios"
    );
    assert!(
        model_canonical_encode_count >= 30,
        "fixtures must include at least 30 model_canonical_encode scenarios"
    );
    assert!(
        model_apply_replay_count >= 140,
        "fixtures must include at least 140 model_apply_replay scenarios"
    );
    assert!(
        model_diff_parity_count >= 300,
        "fixtures must include at least 300 model_diff_parity scenarios"
    );
    assert!(
        model_diff_dst_keys_count >= 80,
        "fixtures must include at least 80 model_diff_dst_keys scenarios"
    );
    assert!(
        lessdb_model_manager_count >= 90,
        "fixtures must include at least 90 lessdb_model_manager scenarios"
    );
    assert!(
        model_api_workflow_count >= 60,
        "fixtures must include at least 60 model_api_workflow scenarios"
    );
    assert!(
        model_api_proxy_fanout_workflow_count >= 40,
        "fixtures must include at least 40 model_api_proxy_fanout_workflow scenarios"
    );
    assert!(
        model_lifecycle_workflow_count >= 60,
        "fixtures must include at least 60 model_lifecycle_workflow scenarios"
    );
    assert!(
        codec_indexed_binary_parity_count >= 40,
        "fixtures must include at least 40 codec_indexed_binary_parity scenarios"
    );
    assert!(
        codec_sidecar_binary_parity_count >= 40,
        "fixtures must include at least 40 codec_sidecar_binary_parity scenarios"
    );
    assert!(
        patch_clock_codec_parity_count >= 40,
        "fixtures must include at least 40 patch_clock_codec_parity scenarios"
    );
}
