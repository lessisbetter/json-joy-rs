use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::model_api::{NativeModelApi, PathStep};
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn as_path(path: &Value) -> Vec<PathStep> {
    path.as_array()
        .expect("path must be array")
        .iter()
        .map(|p| {
            if let Some(i) = p.as_u64() {
                PathStep::Index(i as usize)
            } else {
                PathStep::Key(
                    p.as_str()
                        .expect("path string step must be string")
                        .to_string(),
                )
            }
        })
        .collect()
}

#[test]
fn model_api_workflow_fixtures_match_expected_steps() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
    let mut seen = 0u32;

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_api_workflow") {
            continue;
        }
        seen += 1;
        let name = entry["name"].as_str().expect("fixture name must be string");
        let file = entry["file"].as_str().expect("fixture file must be string");
        let fixture = read_json(&dir.join(file));

        let sid = fixture["input"]["sid"]
            .as_u64()
            .expect("input.sid must be u64");
        let base = decode_hex(
            fixture["input"]["base_model_binary_hex"]
                .as_str()
                .expect("input.base_model_binary_hex must be string"),
        );
        let mut api = NativeModelApi::from_model_binary(&base, Some(sid))
            .unwrap_or_else(|e| panic!("from_model_binary failed for {name}: {e}"));

        let ops = fixture["input"]["ops"]
            .as_array()
            .expect("input.ops must be array");
        let expected_steps = fixture["expected"]["steps"]
            .as_array()
            .expect("expected.steps must be array");
        assert_eq!(
            ops.len(),
            expected_steps.len(),
            "ops/expected step length mismatch for {name}"
        );

        for (op, step) in ops.iter().zip(expected_steps.iter()) {
            match op["kind"].as_str().expect("op.kind must be string") {
                "find" => {
                    let path = as_path(&op["path"]);
                    let found = api.find(&path).unwrap_or(Value::Null);
                    assert_eq!(found, step["value_json"], "find result mismatch for {name}");
                }
                "set" => {
                    let path = as_path(&op["path"]);
                    api.set(&path, op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("set failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "set step view mismatch for {name}"
                    );
                }
                "add" => {
                    let path = as_path(&op["path"]);
                    api.add(&path, op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("add failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "add step view mismatch for {name}"
                    );
                }
                "replace" => {
                    let path = as_path(&op["path"]);
                    api.replace(&path, op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("replace failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "replace step view mismatch for {name}"
                    );
                }
                "remove" => {
                    let path = as_path(&op["path"]);
                    api.remove(&path)
                        .unwrap_or_else(|e| panic!("remove failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "remove step view mismatch for {name}"
                    );
                }
                "obj_put" => {
                    let path = as_path(&op["path"]);
                    api.obj_put(
                        &path,
                        op["key"].as_str().expect("obj_put key must be string"),
                        op["value_json"].clone(),
                    )
                    .unwrap_or_else(|e| panic!("obj_put failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "obj_put step view mismatch for {name}"
                    );
                }
                "arr_push" => {
                    let path = as_path(&op["path"]);
                    api.arr_push(&path, op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("arr_push failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "arr_push step view mismatch for {name}"
                    );
                }
                "str_ins" => {
                    let path = as_path(&op["path"]);
                    api.str_ins(
                        &path,
                        op["pos"].as_u64().expect("str_ins pos must be u64") as usize,
                        op["text"].as_str().expect("str_ins text must be string"),
                    )
                    .unwrap_or_else(|e| panic!("str_ins failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "str_ins step view mismatch for {name}"
                    );
                }
                "apply_batch" => {
                    let patch_hexes = op["patches_binary_hex"]
                        .as_array()
                        .expect("apply_batch patches must be array");
                    let mut patches = Vec::with_capacity(patch_hexes.len());
                    for p in patch_hexes {
                        let bytes = decode_hex(p.as_str().expect("patch hex must be string"));
                        patches.push(Patch::from_binary(&bytes).unwrap_or_else(|e| {
                            panic!("apply_batch patch decode failed for {name}: {e}")
                        }));
                    }
                    api.apply_batch(&patches)
                        .unwrap_or_else(|e| panic!("apply_batch failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "apply_batch step view mismatch for {name}"
                    );
                }
                other => panic!("unexpected op kind for {name}: {other}"),
            }
        }

        assert_eq!(
            api.view(),
            fixture["expected"]["final_view_json"],
            "final view mismatch for {name}"
        );
        let has_json_patch_style_mutators = ops.iter().any(|op| {
            matches!(
                op["kind"].as_str(),
                Some("add") | Some("replace") | Some("remove")
            )
        });
        let bin = api
            .to_model_binary()
            .unwrap_or_else(|e| panic!("to_model_binary failed for {name}: {e}"));
        if !has_json_patch_style_mutators {
            assert_eq!(
                hex(&bin),
                fixture["expected"]["final_model_binary_hex"]
                    .as_str()
                    .expect("expected.final_model_binary_hex must be string"),
                "final model binary mismatch for {name}"
            );
        }
    }

    assert!(
        seen >= 20,
        "expected at least 20 model_api_workflow fixtures"
    );
}
