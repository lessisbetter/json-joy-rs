use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn read_to_string(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path))
}

#[test]
fn must_have_test_modules_exist_for_each_layer() {
    let must_exist = [
        // Fixture contract / fixture parity
        "compat_fixtures.rs",
        "patch_codec_from_fixtures.rs",
        "patch_encode_from_canonical_fixtures.rs",
        "patch_alt_codecs_from_fixtures.rs",
        "patch_compaction_from_fixtures.rs",
        "patch_schema_from_fixtures.rs",
        "util_diff_from_fixtures.rs",
        "model_codec_from_fixtures.rs",
        "model_encode_from_canonical_fixtures.rs",
        "model_apply_replay_from_fixtures.rs",
        "model_diff_parity_from_fixtures.rs",
        "model_diff_dst_keys_from_fixtures.rs",
        "model_api_from_fixtures.rs",
        "model_api_proxy_fanout_from_fixtures.rs",
        "model_lifecycle_from_fixtures.rs",
        "lessdb_model_manager_from_fixtures.rs",
        "codec_indexed_binary_from_fixtures.rs",
        "codec_sidecar_binary_from_fixtures.rs",
        // Upstream mapped
        "upstream_port_diff_matrix.rs",
        "upstream_port_model_api_matrix.rs",
        "upstream_port_model_apply_matrix.rs",
        "upstream_port_nodes_family_matrix.rs",
        "upstream_port_patch_builder_matrix.rs",
        "upstream_port_patch_schema_matrix.rs",
        "upstream_port_util_diff_str_bin_matrix.rs",
        "upstream_port_codec_indexed_binary_matrix.rs",
        // Differential + property
        "differential_runtime_seeded.rs",
        "differential_codec_seeded.rs",
        "differential_patch_codecs_seeded.rs",
        "differential_patch_compaction_seeded.rs",
        "differential_patch_schema_seeded.rs",
        "differential_util_diff_seeded.rs",
        "property_replay_idempotence.rs",
        "property_codec_roundtrip_invariants.rs",
        "property_model_api_event_convergence.rs",
    ];

    let dir = tests_dir();
    for file in must_exist {
        let path = dir.join(file);
        assert!(path.exists(), "required test module missing: {:?}", path);
    }
}

#[test]
fn layer_prefixes_have_expected_depth() {
    let dir = tests_dir();
    let mut upstream = 0usize;
    let mut fixtures = 0usize;
    let mut differential = 0usize;
    let mut property = 0usize;

    for entry in fs::read_dir(&dir).expect("tests directory must be readable") {
        let entry = entry.expect("read_dir entry should be readable");
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".rs") {
            continue;
        }
        if name.starts_with("upstream_port_") {
            upstream += 1;
        } else if name.ends_with("_from_fixtures.rs") || name == "compat_fixtures.rs" {
            fixtures += 1;
        } else if name.starts_with("differential_") {
            differential += 1;
        } else if name.starts_with("property_") {
            property += 1;
        }
    }

    assert!(
        upstream >= 25,
        "expected at least 25 upstream_port suites, got {upstream}"
    );
    assert!(
        fixtures >= 16,
        "expected at least 16 fixture-oriented suites, got {fixtures}"
    );
    assert!(
        differential >= 6,
        "expected at least 6 differential suites, got {differential}"
    );
    assert!(
        property >= 3,
        "expected at least 3 property suites, got {property}"
    );
}

#[test]
fn fixture_manifest_scenarios_have_runner_coverage() {
    let manifest_path = fixtures_dir().join("manifest.json");
    let manifest_raw = read_to_string(&manifest_path);
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("manifest must parse as json");

    let mut scenarios = BTreeSet::new();
    for entry in manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array")
    {
        let scenario = entry["scenario"]
            .as_str()
            .expect("scenario must be string")
            .to_string();
        scenarios.insert(scenario);
    }

    let expected: BTreeSet<&str> = [
        "patch_diff_apply",
        "patch_decode_error",
        "patch_canonical_encode",
        "patch_alt_codecs",
        "patch_compaction_parity",
        "patch_schema_parity",
        "util_diff_parity",
        "model_roundtrip",
        "model_decode_error",
        "model_canonical_encode",
        "model_apply_replay",
        "model_diff_parity",
        "model_diff_dst_keys",
        "lessdb_model_manager",
        "model_api_workflow",
        "model_api_proxy_fanout_workflow",
        "model_lifecycle_workflow",
        "codec_indexed_binary_parity",
        "codec_sidecar_binary_parity",
        "patch_clock_codec_parity",
    ]
    .into_iter()
    .collect();

    assert_eq!(
        scenarios,
        expected
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>(),
        "fixture scenario inventory drifted; update compat coverage and runners intentionally"
    );
}
