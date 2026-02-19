mod common;

use std::collections::BTreeSet;

use common::fixtures::{
    expected_scenarios_set, fixtures_dir, load_manifest, read_json, scenario_counts,
    EXPECTED_FIXTURE_COUNT, EXPECTED_FIXTURE_VERSION, EXPECTED_SCENARIO_COUNTS,
    EXPECTED_UPSTREAM_PACKAGE, EXPECTED_UPSTREAM_VERSION,
};

#[test]
fn manifest_contract_is_pinned() {
    let manifest = load_manifest();
    assert_eq!(manifest.fixture_version, EXPECTED_FIXTURE_VERSION);
    assert_eq!(manifest.upstream_package, EXPECTED_UPSTREAM_PACKAGE);
    assert_eq!(manifest.upstream_version, EXPECTED_UPSTREAM_VERSION);
    assert_eq!(manifest.fixture_count, EXPECTED_FIXTURE_COUNT);
    assert_eq!(manifest.fixtures.len(), EXPECTED_FIXTURE_COUNT);
}

#[test]
fn manifest_entries_are_unique_and_exist() {
    let manifest = load_manifest();
    let dir = fixtures_dir();

    let mut names = BTreeSet::<String>::new();
    for entry in &manifest.fixtures {
        assert!(names.insert(entry.name.clone()), "duplicate fixture name: {}", entry.name);
        let path = dir.join(&entry.file);
        assert!(path.exists(), "missing fixture file: {:?}", path);
        let fixture = read_json(&path);
        assert_eq!(fixture["fixture_version"].as_u64(), Some(EXPECTED_FIXTURE_VERSION));
        assert_eq!(fixture["name"].as_str(), Some(entry.name.as_str()));
        assert_eq!(fixture["scenario"].as_str(), Some(entry.scenario.as_str()));
        assert_eq!(
            fixture["meta"]["upstream_package"].as_str(),
            Some(EXPECTED_UPSTREAM_PACKAGE)
        );
        assert_eq!(
            fixture["meta"]["upstream_version"].as_str(),
            Some(EXPECTED_UPSTREAM_VERSION)
        );
    }
}

#[test]
fn scenario_set_and_counts_match_expected() {
    let manifest = load_manifest();
    let counts = scenario_counts(&manifest.fixtures);

    let expected_set = expected_scenarios_set();
    let actual_set: BTreeSet<&str> = counts.keys().map(String::as_str).collect();
    assert_eq!(actual_set, expected_set, "scenario set mismatch");

    for (scenario, expected_count) in EXPECTED_SCENARIO_COUNTS {
        let actual = counts.get(*scenario).copied().unwrap_or(0);
        assert_eq!(
            actual, *expected_count,
            "scenario count mismatch for {scenario}"
        );
    }
}
