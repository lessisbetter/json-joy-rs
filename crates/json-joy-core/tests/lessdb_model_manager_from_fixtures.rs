use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::less_db_compat::{
    apply_patch, create_model, diff_model, fork_model, merge_with_pending_patches, model_from_binary,
    model_load, model_to_binary, view_model, CompatModel,
};
use json_joy_core::patch::Patch;
use json_joy_core::patch_log::{append_patch, deserialize_patches};
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn load_lessdb_fixtures() -> Vec<(String, Value)> {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut out = Vec::new();
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("lessdb_model_manager") {
            continue;
        }
        let name = entry["name"].as_str().expect("fixture entry name must be string");
        let file = entry["file"].as_str().expect("fixture entry file must be string");
        out.push((name.to_string(), read_json(&dir.join(file))));
    }
    out
}

fn assert_final_state(name: &str, fixture: &Value, model: &CompatModel) {
    assert_eq!(
        view_model(model),
        fixture["expected"]["final_view_json"],
        "final view mismatch for fixture {name}"
    );
    let expected_binary = decode_hex(
        fixture["expected"]["final_model_binary_hex"]
            .as_str()
            .expect("expected.final_model_binary_hex must be string"),
    );
    assert_eq!(
        model_to_binary(model),
        expected_binary,
        "final model binary mismatch for fixture {name}"
    );
}

#[test]
fn lessdb_create_diff_apply_matches_oracle() {
    let fixtures = load_lessdb_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if fixture["input"]["workflow"].as_str() != Some("create_diff_apply") {
            continue;
        }
        seen += 1;

        let sid = fixture["input"]["sid"].as_u64().expect("input.sid must be u64");
        let initial = &fixture["input"]["initial_json"];
        let ops = fixture["input"]["ops"].as_array().expect("input.ops must be array");
        let steps = fixture["expected"]["steps"].as_array().expect("expected.steps must be array");

        let mut model = create_model(initial, sid).unwrap_or_else(|e| panic!("create failed for {name}: {e}"));
        let mut last_diff: Option<Vec<u8>> = None;
        let mut pending = Vec::<u8>::new();

        for (idx, op) in ops.iter().enumerate() {
            let kind = op["kind"].as_str().expect("op.kind must be string");
            let step = &steps[idx];
            match kind {
                "diff" => {
                    let next = &op["next_view_json"];
                    let patch = diff_model(&model, next)
                        .unwrap_or_else(|e| panic!("diff failed for {name}: {e}"));
                    let expected_present = step["patch_present"]
                        .as_bool()
                        .expect("step.patch_present must be bool");
                    assert_eq!(patch.is_some(), expected_present, "patch presence mismatch for fixture {name}");
                    if let Some(ref p) = patch {
                        let expected_hex = step["patch_binary_hex"]
                            .as_str()
                            .expect("step.patch_binary_hex must be string for patch-present steps");
                        assert_eq!(hex(p), expected_hex, "patch bytes mismatch for fixture {name}");

                        let decoded = Patch::from_binary(p)
                            .unwrap_or_else(|e| panic!("decoded patch failed for {name}: {e}"));
                        assert_eq!(
                            decoded.op_count(),
                            step["patch_op_count"].as_u64().expect("step.patch_op_count must be u64"),
                            "patch op_count mismatch for fixture {name}"
                        );
                    }
                    last_diff = patch;
                }
                "apply_last_diff" => {
                    if let Some(ref p) = last_diff {
                        apply_patch(&mut model, p)
                            .unwrap_or_else(|e| panic!("apply failed for {name}: {e}"));
                    }
                    assert_eq!(
                        view_model(&model),
                        step["view_json"],
                        "apply view mismatch for fixture {name}"
                    );
                }
                "patch_log_append_last_diff" => {
                    if let Some(ref p) = last_diff {
                        let patch = Patch::from_binary(p).expect("last diff patch should decode");
                        pending = append_patch(&pending, &patch);
                    }
                    let expected_hex = step["pending_patch_log_hex"]
                        .as_str()
                        .expect("step.pending_patch_log_hex must be string");
                    assert_eq!(hex(&pending), expected_hex, "pending patch log mismatch for fixture {name}");
                }
                "patch_log_deserialize" => {
                    let decoded = deserialize_patches(&pending)
                        .unwrap_or_else(|e| panic!("patch log decode failed for {name}: {e}"));
                    let expected_count = step["patch_count"].as_u64().expect("step.patch_count must be u64");
                    assert_eq!(decoded.len() as u64, expected_count, "patch log count mismatch for fixture {name}");
                }
                other => panic!("unexpected op kind in create_diff_apply fixture: {other}"),
            }
        }

        assert_final_state(&name, &fixture, &model);
    }

    assert!(seen >= 20, "expected at least 20 create_diff_apply fixtures");
}

#[test]
fn lessdb_noop_diff_returns_none() {
    let fixtures = load_lessdb_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if fixture["input"]["workflow"].as_str() != Some("create_diff_apply") {
            continue;
        }
        let steps = fixture["expected"]["steps"].as_array().expect("expected.steps must be array");
        if steps[0]["patch_present"].as_bool() != Some(false) {
            continue;
        }
        seen += 1;

        let sid = fixture["input"]["sid"].as_u64().expect("input.sid must be u64");
        let initial = &fixture["input"]["initial_json"];
        let next = &fixture["input"]["ops"][0]["next_view_json"];
        let model = create_model(initial, sid).unwrap_or_else(|e| panic!("create failed for {name}: {e}"));
        let patch = diff_model(&model, next).unwrap_or_else(|e| panic!("diff failed for {name}: {e}"));
        assert!(patch.is_none(), "expected no-op diff for fixture {name}");
    }

    assert!(seen >= 3, "expected at least 3 no-op lessdb fixtures");
}

#[test]
fn lessdb_merge_with_pending_patches_is_idempotent() {
    let fixtures = load_lessdb_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if fixture["input"]["workflow"].as_str() != Some("merge_idempotent") {
            continue;
        }
        seen += 1;

        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let mut model = model_from_binary(&base)
            .unwrap_or_else(|e| panic!("model_from_binary failed for {name}: {e}"));

        let patches = fixture["input"]["ops"][0]["patches_binary_hex"]
            .as_array()
            .expect("ops[0].patches_binary_hex must be array")
            .iter()
            .map(|v| decode_hex(v.as_str().expect("patch hex must be string")))
            .collect::<Vec<_>>();

        merge_with_pending_patches(&mut model, &patches)
            .unwrap_or_else(|e| panic!("merge failed for {name}: {e}"));

        assert_final_state(&name, &fixture, &model);
    }

    assert!(seen >= 5, "expected at least 5 merge_idempotent fixtures");
}

#[test]
fn lessdb_fork_and_merge_scenarios_match_oracle() {
    let fixtures = load_lessdb_fixtures();
    let mut seen = 0u32;

    for (name, fixture) in fixtures {
        if fixture["input"]["workflow"].as_str() != Some("fork_merge") {
            continue;
        }
        seen += 1;

        let sid = fixture["input"]["sid"].as_u64().expect("input.sid must be u64");
        let initial = &fixture["input"]["initial_json"];
        let fork_sid = fixture["input"]["ops"][0]["sid"]
            .as_u64()
            .expect("fork sid must be u64");
        let next = &fixture["input"]["ops"][1]["next_view_json"];

        let mut base = create_model(initial, sid)
            .unwrap_or_else(|e| panic!("create failed for {name}: {e}"));
        let mut fork = fork_model(&base, Some(fork_sid))
            .unwrap_or_else(|e| panic!("fork failed for {name}: {e}"));

        let patch = diff_model(&fork, next)
            .unwrap_or_else(|e| panic!("fork diff failed for {name}: {e}"))
            .unwrap_or_else(|| panic!("expected patch in fork fixture {name}"));

        apply_patch(&mut fork, &patch)
            .unwrap_or_else(|e| panic!("fork apply failed for {name}: {e}"));

        merge_with_pending_patches(&mut base, &[patch])
            .unwrap_or_else(|e| panic!("base merge failed for {name}: {e}"));

        assert_final_state(&name, &fixture, &base);
    }

    assert!(seen >= 5, "expected at least 5 fork_merge fixtures");
}

#[test]
fn lessdb_load_size_limit_is_enforced() {
    let oversized = vec![0u8; (10 * 1024 * 1024) + 1];

    let err1 = model_from_binary(&oversized).expect_err("model_from_binary should reject oversized payload");
    let msg1 = err1.to_string();
    assert!(msg1.contains("too large"), "unexpected model_from_binary error: {msg1}");

    let err2 = model_load(&oversized, 75001).expect_err("model_load should reject oversized payload");
    let msg2 = err2.to_string();
    assert!(msg2.contains("too large"), "unexpected model_load error: {msg2}");
}
