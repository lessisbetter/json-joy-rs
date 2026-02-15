use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::model_runtime::RuntimeModel;
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
    let data = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex string must have even length");
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}

fn load_apply_replay_fixtures() -> Vec<(String, Value)> {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut out = Vec::new();
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_apply_replay") {
            continue;
        }
        let name = entry["name"].as_str().expect("fixture entry name must be string");
        let file = entry["file"].as_str().expect("fixture entry file must be string");
        out.push((name.to_string(), read_json(&dir.join(file))));
    }
    out
}

#[test]
fn apply_replay_fixtures_match_oracle_view() {
    let fixtures = load_apply_replay_fixtures();
    assert!(fixtures.len() >= 30, "expected at least 30 model_apply_replay fixtures");

    for (name, fixture) in fixtures {
        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let patch_hexes = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("input.patches_binary_hex must be array");
        let replay = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("input.replay_pattern must be array");

        let patches = patch_hexes
            .iter()
            .map(|v| {
                let bytes = decode_hex(v.as_str().expect("patch hex must be string"));
                Patch::from_binary(&bytes).expect("patch must decode")
            })
            .collect::<Vec<_>>();

        let mut runtime = RuntimeModel::from_model_binary(&base_bytes)
            .unwrap_or_else(|e| panic!("runtime base decode failed for {name}: {e}"));
        for idx in replay {
            let i = idx.as_u64().expect("replay index must be u64") as usize;
            runtime
                .apply_patch(&patches[i])
                .unwrap_or_else(|e| panic!("runtime apply failed for {name}: {e}"));
        }

        assert_eq!(
            runtime.view_json(),
            fixture["expected"]["view_json"],
            "runtime view mismatch for fixture {name}"
        );
    }
}

#[test]
fn duplicate_patch_replay_is_idempotent() {
    let fixtures = load_apply_replay_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if !name.contains("dup") {
            continue;
        }
        seen += 1;

        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let patch_hexes = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("input.patches_binary_hex must be array");
        let replay = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("input.replay_pattern must be array");

        let patches = patch_hexes
            .iter()
            .map(|v| {
                let bytes = decode_hex(v.as_str().expect("patch hex must be string"));
                Patch::from_binary(&bytes).expect("patch must decode")
            })
            .collect::<Vec<_>>();

        let mut runtime = RuntimeModel::from_model_binary(&base_bytes).expect("runtime model must decode");
        let mut first_seen = false;
        let mut first_view = None;
        for idx in replay {
            let i = idx.as_u64().expect("replay index must be u64") as usize;
            runtime.apply_patch(&patches[i]).expect("runtime apply must succeed");
            if !first_seen {
                first_seen = true;
                first_view = Some(runtime.view_json());
            }
        }

        assert_eq!(
            runtime.view_json(),
            fixture["expected"]["view_json"],
            "duplicate replay expected view mismatch for fixture {name}"
        );
        assert!(first_view.is_some(), "duplicate fixture should include at least one apply");
    }

    assert!(seen >= 4, "expected at least 4 duplicate replay fixtures");
}

#[test]
fn out_of_order_replay_matches_oracle() {
    let fixtures = load_apply_replay_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if !(name.contains("stale") || name.contains("order") || name.contains("matrix")) {
            continue;
        }
        seen += 1;

        let base_bytes = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let patch_hexes = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("input.patches_binary_hex must be array");
        let replay = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("input.replay_pattern must be array");

        let patches = patch_hexes
            .iter()
            .map(|v| {
                let bytes = decode_hex(v.as_str().expect("patch hex must be string"));
                Patch::from_binary(&bytes).expect("patch must decode")
            })
            .collect::<Vec<_>>();

        let mut runtime = RuntimeModel::from_model_binary(&base_bytes).expect("runtime model must decode");
        for idx in replay {
            let i = idx.as_u64().expect("replay index must be u64") as usize;
            runtime.apply_patch(&patches[i]).expect("runtime apply must succeed");
        }

        assert_eq!(
            runtime.view_json(),
            fixture["expected"]["view_json"],
            "out-of-order expected view mismatch for fixture {name}"
        );
    }

    assert!(seen >= 10, "expected at least 10 out-of-order replay fixtures");
}
