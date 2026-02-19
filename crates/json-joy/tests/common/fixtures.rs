use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

pub const EXPECTED_UPSTREAM_PACKAGE: &str = "json-joy";
pub const EXPECTED_UPSTREAM_VERSION: &str = "17.67.0";
pub const EXPECTED_FIXTURE_VERSION: u64 = 1;
pub const EXPECTED_FIXTURE_COUNT: usize = 1398;

pub const EXPECTED_SCENARIO_COUNTS: &[(&str, usize)] = &[
    ("codec_indexed_binary_parity", 40),
    ("codec_sidecar_binary_parity", 40),
    ("lessdb_model_manager", 90),
    ("model_api_proxy_fanout_workflow", 40),
    ("model_api_workflow", 60),
    ("model_apply_replay", 140),
    ("model_canonical_encode", 30),
    ("model_decode_error", 35),
    ("model_diff_dst_keys", 80),
    ("model_diff_parity", 300),
    ("model_lifecycle_workflow", 60),
    ("model_roundtrip", 110),
    ("patch_alt_codecs", 44),
    ("patch_canonical_encode", 44),
    ("patch_clock_codec_parity", 40),
    ("patch_compaction_parity", 45),
    ("patch_decode_error", 35),
    ("patch_diff_apply", 40),
    ("patch_schema_parity", 45),
    ("util_diff_parity", 80),
];

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestEntry {
    pub name: String,
    pub scenario: String,
    pub file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub fixture_version: u64,
    pub upstream_package: String,
    pub upstream_version: String,
    pub fixture_count: usize,
    pub fixtures: Vec<ManifestEntry>,
}

#[derive(Debug, Clone)]
pub struct FixtureRecord {
    pub entry: ManifestEntry,
    pub fixture: Value,
}

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

pub fn xfail_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("xfail.toml")
}

pub fn read_json(path: &Path) -> Value {
    let data = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

pub fn load_manifest() -> Manifest {
    let path = fixtures_dir().join("manifest.json");
    let data = fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

pub fn load_fixture_record(dir: &Path, entry: &ManifestEntry) -> FixtureRecord {
    let path = dir.join(&entry.file);
    let fixture = read_json(&path);
    FixtureRecord {
        entry: entry.clone(),
        fixture,
    }
}

pub fn load_all_fixture_records() -> Vec<FixtureRecord> {
    let dir = fixtures_dir();
    let manifest = load_manifest();
    manifest
        .fixtures
        .iter()
        .map(|entry| load_fixture_record(&dir, entry))
        .collect()
}

pub fn scenario_counts(entries: &[ManifestEntry]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::<String, usize>::new();
    for entry in entries {
        *out.entry(entry.scenario.clone()).or_insert(0) += 1;
    }
    out
}

pub fn expected_scenarios_set() -> BTreeSet<&'static str> {
    EXPECTED_SCENARIO_COUNTS.iter().map(|(k, _)| *k).collect()
}
