mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};

use serde::Deserialize;

use common::assertions::compare_expected_fields;
use common::fixtures::{
    load_fixture_record, load_manifest, xfail_path, FixtureRecord, ManifestEntry,
};
use common::scenarios::evaluate_fixture;

#[derive(Debug, Clone, Deserialize)]
struct XfailList {
    #[serde(default)]
    xfail: Vec<XfailEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct XfailEntry {
    scenario: String,
    name: String,
    reason: String,
    source_file: String,
    issue: String,
}

fn load_xfails() -> XfailList {
    let path = xfail_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    toml::from_str(&text).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn matches_xfail<'a>(entry: &ManifestEntry, xfails: &'a [XfailEntry]) -> Option<&'a XfailEntry> {
    xfails.iter().find(|xf| {
        xf.scenario == entry.scenario
            && (xf.name == entry.name || xf.name == "*" || (xf.name.ends_with('*') && entry.name.starts_with(&xf.name[..xf.name.len() - 1])))
    })
}

#[test]
fn compat_fixtures_replay_with_xfail_policy() {
    let manifest = load_manifest();
    let dir = common::fixtures::fixtures_dir();
    let xfail_list = load_xfails();

    let mut seen_fixture_names = BTreeSet::<String>::new();
    for entry in &manifest.fixtures {
        seen_fixture_names.insert(entry.name.clone());
    }

    // stale xfails are failures
    for xf in &xfail_list.xfail {
        let matched = if xf.name == "*" {
            manifest.fixtures.iter().any(|e| e.scenario == xf.scenario)
        } else if xf.name.ends_with('*') {
            let pfx = &xf.name[..xf.name.len() - 1];
            manifest
                .fixtures
                .iter()
                .any(|e| e.scenario == xf.scenario && e.name.starts_with(pfx))
        } else {
            seen_fixture_names.contains(&xf.name)
                && manifest
                    .fixtures
                    .iter()
                    .any(|e| e.scenario == xf.scenario && e.name == xf.name)
        };
        assert!(
            matched,
            "stale xfail entry: scenario={}, name={}, source_file={}, issue={}",
            xf.scenario,
            xf.name,
            xf.source_file,
            xf.issue
        );
    }

    let mut hard_failures = Vec::<String>::new();
    let mut unexpected_pass = Vec::<String>::new();
    let mut counts = BTreeMap::<String, usize>::new();

    for entry in &manifest.fixtures {
        *counts.entry(entry.scenario.clone()).or_insert(0) += 1;
        let record: FixtureRecord = load_fixture_record(&dir, entry);
        let expected = record
            .fixture
            .get("expected")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Null);

        let evaluation = catch_unwind(AssertUnwindSafe(|| {
            evaluate_fixture(&entry.scenario, &record.fixture)
                .map(|actual| compare_expected_fields(&expected, &actual))
        }))
        .map_err(|panic_payload| {
            if let Some(s) = panic_payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            }
        });

        let xfail = matches_xfail(entry, &xfail_list.xfail);

        match evaluation {
            Ok(Ok(diffs)) if diffs.is_empty() => {
                if let Some(xf) = xfail {
                    let wildcard = xf.name == "*" || xf.name.ends_with('*');
                    if !wildcard {
                    unexpected_pass.push(format!("{} ({})", entry.name, entry.scenario));
                    }
                }
            }
            Ok(Ok(diffs)) => {
                let reason = format!(
                    "{} ({}) mismatches:\n  - {}",
                    entry.name,
                    entry.scenario,
                    diffs.join("\n  - ")
                );
                if let Some(xf) = xfail {
                    if xf.reason.is_empty() {
                        hard_failures.push(format!(
                            "{} [xfail has empty reason, requires update]",
                            reason
                        ));
                    }
                } else {
                    hard_failures.push(reason);
                }
            }
            Ok(Err(err)) | Err(err) => {
                let reason = format!("{} ({}) runtime error: {err}", entry.name, entry.scenario);
                if xfail.is_none() {
                    hard_failures.push(reason);
                }
            }
        }
    }

    assert!(
        unexpected_pass.is_empty(),
        "unexpected pass for xfail entries:\n{}",
        unexpected_pass.join("\n")
    );
    assert!(
        hard_failures.is_empty(),
        "compat parity failures:\n{}",
        hard_failures.join("\n")
    );
}
