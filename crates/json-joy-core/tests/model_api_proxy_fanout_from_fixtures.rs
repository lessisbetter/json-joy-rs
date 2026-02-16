use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use json_joy_core::model_api::{NativeModelApi, PathStep};
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
fn model_api_proxy_fanout_workflow_fixtures_match_expected_steps() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
    let mut seen = 0u32;

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_api_proxy_fanout_workflow") {
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

        let change_count = Arc::new(Mutex::new(0usize));
        let change_count_clone = Arc::clone(&change_count);
        api.on_change(move |_| {
            let mut v = change_count_clone
                .lock()
                .expect("change_count mutex poisoned");
            *v += 1;
        });

        let scoped_path = as_path(&fixture["input"]["scoped_path"]);
        let scoped_count = Arc::new(Mutex::new(0usize));
        let scoped_count_clone = Arc::clone(&scoped_count);
        api.on_change_at(scoped_path, move |_| {
            let mut v = scoped_count_clone
                .lock()
                .expect("scoped_count mutex poisoned");
            *v += 1;
        });

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
            let path = op.get("path").map(as_path).unwrap_or_default();
            match op["kind"].as_str().expect("op.kind must be string") {
                "read" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    let value = node.read().unwrap_or(Value::Null);
                    assert_eq!(value, step["value_json"], "read value mismatch for {name}");
                }
                "node_obj_put" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.obj_put(
                        op["key"].as_str().expect("node_obj_put key must be string"),
                        op["value_json"].clone(),
                    )
                    .unwrap_or_else(|e| panic!("node_obj_put failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_obj_put step view mismatch for {name}"
                    );
                }
                "node_arr_push" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.arr_push(op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("node_arr_push failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_arr_push step view mismatch for {name}"
                    );
                }
                "node_str_ins" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.str_ins(
                        op["pos"].as_u64().expect("node_str_ins pos must be u64") as usize,
                        op["text"]
                            .as_str()
                            .expect("node_str_ins text must be string"),
                    )
                    .unwrap_or_else(|e| panic!("node_str_ins failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_str_ins step view mismatch for {name}"
                    );
                }
                "node_add" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.add(op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("node_add failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_add step view mismatch for {name}"
                    );
                }
                "node_replace" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.replace(op["value_json"].clone())
                        .unwrap_or_else(|e| panic!("node_replace failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_replace step view mismatch for {name}"
                    );
                }
                "node_remove" => {
                    let mut node = api.node();
                    for seg in &path {
                        node = match seg {
                            PathStep::Key(k) => node.at_key(k.clone()),
                            PathStep::Index(i) => node.at_index(*i),
                            PathStep::Append => node.at_append(),
                        };
                    }
                    node.remove()
                        .unwrap_or_else(|e| panic!("node_remove failed for {name}: {e}"));
                    assert_eq!(
                        api.view(),
                        step["view_json"],
                        "node_remove step view mismatch for {name}"
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
        assert_eq!(
            *change_count.lock().expect("change_count mutex poisoned") as u64,
            fixture["expected"]["fanout"]["change_count"]
                .as_u64()
                .expect("expected.fanout.change_count must be u64"),
            "fanout change_count mismatch for {name}"
        );
        assert_eq!(
            *scoped_count.lock().expect("scoped_count mutex poisoned") as u64,
            fixture["expected"]["fanout"]["scoped_count"]
                .as_u64()
                .expect("expected.fanout.scoped_count must be u64"),
            "fanout scoped_count mismatch for {name}"
        );
    }

    assert!(
        seen >= 40,
        "expected at least 40 model_api_proxy_fanout_workflow fixtures"
    );
}
