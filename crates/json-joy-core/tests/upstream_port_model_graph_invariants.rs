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
    let data =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex string must have even length"
    );
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}

#[test]
fn upstream_port_model_graph_invariants_hold_during_replay_matrix() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_apply_replay") {
            continue;
        }
        seen += 1;

        let file = entry["file"]
            .as_str()
            .expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));
        let name = fixture["name"].as_str().unwrap_or("unknown");

        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let mut runtime = RuntimeModel::from_model_binary(&base)
            .unwrap_or_else(|e| panic!("runtime decode failed for {name}: {e}"));
        runtime
            .validate_invariants()
            .unwrap_or_else(|e| panic!("base invariants failed for {name}: {e}"));

        let patches = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("input.patches_binary_hex must be array")
            .iter()
            .map(|v| decode_hex(v.as_str().expect("patch hex must be string")))
            .collect::<Vec<_>>();
        let replay = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("input.replay_pattern must be array");

        for step in replay {
            let ix = step.as_u64().expect("replay index must be u64") as usize;
            let patch = Patch::from_binary(&patches[ix])
                .unwrap_or_else(|e| panic!("patch decode failed for {name}: {e}"));
            runtime
                .apply_patch(&patch)
                .unwrap_or_else(|e| panic!("runtime apply failed for {name}: {e}"));
            runtime
                .validate_invariants()
                .unwrap_or_else(|e| panic!("post-apply invariants failed for {name}: {e}"));
        }
    }

    assert!(
        seen >= 50,
        "expected at least 50 model_apply_replay fixtures"
    );
}
