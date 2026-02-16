use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::Patch;
use serde_json::Value;

#[test]
fn property_replay_second_pass_matches_oracle() {
    // Upstream reference:
    // `json-crdt/model/Model.ts` applies `clock.observe(op.id, op.span())` for
    // every operation. That means some replay patterns are intentionally not
    // globally idempotent across a full second pass when causal prerequisites
    // were missing during the first pass. Validate against oracle behavior
    // directly instead of assuming universal second-pass stability.
    let fixtures = load_apply_replay_fixtures();
    assert!(fixtures.len() >= 50, "expected >=50 apply replay fixtures");

    for fixture in fixtures {
        let label = fixture["input"]["label"].as_str().unwrap_or_default();
        if !label.contains("dup") {
            continue;
        }
        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("base_model_binary_hex must be string"),
        );
        let patches: Vec<Patch> = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("patches_binary_hex must be array")
            .iter()
            .map(|v| {
                let bytes = decode_hex(v.as_str().expect("patch hex must be string"));
                Patch::from_binary(&bytes).expect("patch must decode")
            })
            .collect();
        let replay: Vec<usize> = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("replay_pattern must be array")
            .iter()
            .map(|v| {
                usize::try_from(v.as_u64().expect("index must be u64")).expect("index out of range")
            })
            .collect();

        let mut model = RuntimeModel::from_model_binary(&base).expect("base decode must succeed");
        model
            .validate_invariants()
            .expect("invariants must hold for base model");
        for idx in replay.iter().copied() {
            model
                .apply_patch(&patches[idx])
                .expect("apply must succeed");
            model
                .validate_invariants()
                .expect("invariants must hold after apply");
        }
        let once_view = model.view_json();

        for idx in replay.iter().copied() {
            model
                .apply_patch(&patches[idx])
                .expect("second pass apply must succeed");
            model
                .validate_invariants()
                .expect("invariants must hold after second-pass apply");
        }
        let twice_view = model.view_json();

        let patch_hexes: Vec<String> = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("patches_binary_hex must be array")
            .iter()
            .map(|v| v.as_str().expect("patch hex must be string").to_string())
            .collect();
        let (oracle_once, oracle_twice) = oracle_replay_once_twice(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("base_model_binary_hex must be string"),
            &patch_hexes,
            &replay,
        );

        assert_eq!(
            once_view, oracle_once,
            "first pass replay mismatch for {}",
            fixture["name"]
        );
        assert_eq!(
            twice_view, oracle_twice,
            "second pass replay mismatch for {}",
            fixture["name"]
        );
    }
}

#[test]
fn property_duplicate_compression_preserves_view_for_duplicate_heavy_fixtures() {
    let fixtures = load_apply_replay_fixtures();

    for fixture in fixtures {
        let label = fixture["input"]["label"].as_str().unwrap_or_default();
        if !label.contains("dup") {
            continue;
        }

        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("base_model_binary_hex must be string"),
        );
        let patches: Vec<Patch> = fixture["input"]["patches_binary_hex"]
            .as_array()
            .expect("patches_binary_hex must be array")
            .iter()
            .map(|v| {
                let bytes = decode_hex(v.as_str().expect("patch hex must be string"));
                Patch::from_binary(&bytes).expect("patch must decode")
            })
            .collect();
        let replay: Vec<usize> = fixture["input"]["replay_pattern"]
            .as_array()
            .expect("replay_pattern must be array")
            .iter()
            .map(|v| {
                usize::try_from(v.as_u64().expect("index must be u64")).expect("index out of range")
            })
            .collect();

        let compressed = compress_adjacent_duplicates(&replay);

        let mut full = RuntimeModel::from_model_binary(&base).expect("base decode must succeed");
        full.validate_invariants()
            .expect("invariants must hold for base model");
        for idx in replay.iter().copied() {
            full.apply_patch(&patches[idx])
                .expect("full apply must succeed");
            full.validate_invariants()
                .expect("invariants must hold after full apply");
        }

        let mut dedup = RuntimeModel::from_model_binary(&base).expect("base decode must succeed");
        dedup
            .validate_invariants()
            .expect("invariants must hold for base model");
        for idx in compressed.iter().copied() {
            dedup
                .apply_patch(&patches[idx])
                .expect("dedup apply must succeed");
            dedup
                .validate_invariants()
                .expect("invariants must hold after dedup apply");
        }

        assert_eq!(
            full.view_json(),
            dedup.view_json(),
            "adjacent duplicate compression changed view for {}",
            fixture["name"]
        );
    }
}

fn load_apply_replay_fixtures() -> Vec<Value> {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let entries = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");

    let mut out = Vec::new();
    for entry in entries {
        if entry["scenario"].as_str() != Some("model_apply_replay") {
            continue;
        }
        let file = entry["file"].as_str().expect("fixture file must be string");
        out.push(read_json(&dir.join(file)));
    }
    out
}

fn compress_adjacent_duplicates(replay: &[usize]) -> Vec<usize> {
    if replay.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(replay.len());
    out.push(replay[0]);
    for idx in replay.iter().copied().skip(1) {
        if out.last().copied() != Some(idx) {
            out.push(idx);
        }
    }
    out
}

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

fn oracle_cwd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

fn oracle_replay_once_twice(
    base_model_binary_hex: &str,
    patches_binary_hex: &[String],
    replay_pattern: &[usize],
) -> (Value, Value) {
    let script = r#"
const {Model} = require('json-joy/lib/json-crdt');
const {Patch} = require('json-joy/lib/json-crdt-patch');
const input = JSON.parse(process.argv[1]);
const model = Model.fromBinary(Buffer.from(input.base_model_binary_hex, 'hex'));
const patches = input.patches_binary_hex.map((h) => Patch.fromBinary(Buffer.from(h, 'hex')));
for (const i of input.replay_pattern) model.applyPatch(patches[i]);
const once = model.view();
for (const i of input.replay_pattern) model.applyPatch(patches[i]);
const twice = model.view();
process.stdout.write(JSON.stringify({once, twice}));
"#;

    let payload = serde_json::json!({
        "base_model_binary_hex": base_model_binary_hex,
        "patches_binary_hex": patches_binary_hex,
        "replay_pattern": replay_pattern,
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run oracle replay script");
    assert!(
        output.status.success(),
        "oracle replay script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value =
        serde_json::from_slice(&output.stdout).expect("oracle replay output must be valid json");
    (parsed["once"].clone(), parsed["twice"].clone())
}
