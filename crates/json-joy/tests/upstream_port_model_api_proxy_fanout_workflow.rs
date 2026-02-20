mod common;

use std::collections::BTreeSet;

use common::assertions::{compare_expected_fields, decode_hex};
use common::fixtures::{load_fixture_record, load_manifest};
use common::scenarios::evaluate_fixture;
use json_joy::json_crdt::codec::structural::binary as structural_binary;
use json_joy::json_crdt::model::api::find_path;
use json_joy::json_crdt::model::Model;
use json_joy::json_crdt::nodes::{CrdtNode, IndexExt};
use json_joy::json_crdt_patch::clock::Ts;
use serde_json::{Map, Value};

const MODEL_API_PROXY_FANOUT_WORKFLOW_XFAILS: &[&str] = &[];

fn select_expected_fields(expected_full: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = expected_full.get(*key) {
            out.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(out)
}

fn format_path(path: &[Value]) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    let mut out = String::new();
    for token in path {
        out.push('/');
        match token {
            Value::String(s) => out.push_str(&s.replace('~', "~0").replace('/', "~1")),
            Value::Number(n) => out.push_str(&n.to_string()),
            _ => out.push_str(&token.to_string()),
        }
    }
    out
}

fn node_kind_name(node: &CrdtNode) -> &'static str {
    match node {
        CrdtNode::Con(_) => "con",
        CrdtNode::Val(_) => "val",
        CrdtNode::Obj(_) => "obj",
        CrdtNode::Vec(_) => "vec",
        CrdtNode::Str(_) => "str",
        CrdtNode::Bin(_) => "bin",
        CrdtNode::Arr(_) => "arr",
    }
}

fn resolve_non_val_node(model: &Model, mut id: Ts) -> Result<&CrdtNode, String> {
    for _ in 0..16 {
        let node = IndexExt::get(&model.index, &id).ok_or_else(|| {
            format!(
                "missing node sid={}, time={} while resolving val chain",
                id.sid, id.time
            )
        })?;
        if let CrdtNode::Val(val) = node {
            id = val.val;
            continue;
        }
        return Ok(node);
    }
    Err("val chain exceeded depth limit".to_string())
}

fn collect_paths(
    value: &Value,
    path: &mut Vec<Value>,
    string_paths: &mut Vec<Vec<Value>>,
    array_scalar_paths: &mut Vec<Vec<Value>>,
) {
    match value {
        Value::String(_) => string_paths.push(path.clone()),
        Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                path.push(Value::from(idx as u64));
                if matches!(item, Value::Null | Value::Bool(_) | Value::Number(_)) {
                    array_scalar_paths.push(path.clone());
                }
                collect_paths(item, path, string_paths, array_scalar_paths);
                path.pop();
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                path.push(Value::String(key.clone()));
                collect_paths(item, path, string_paths, array_scalar_paths);
                path.pop();
            }
        }
        _ => {}
    }
}

fn critical_node_kind_diffs(model: &Model, final_view: &Value) -> Vec<String> {
    let mut diffs = Vec::<String>::new();
    let mut string_paths = Vec::<Vec<Value>>::new();
    let mut array_scalar_paths = Vec::<Vec<Value>>::new();
    let mut path = Vec::<Value>::new();

    collect_paths(
        final_view,
        &mut path,
        &mut string_paths,
        &mut array_scalar_paths,
    );

    for path in string_paths {
        let id = match find_path(model, model.root.val, &path) {
            Ok(id) => id,
            Err(err) => {
                diffs.push(format!(
                    "{}: string path not found ({err})",
                    format_path(&path)
                ));
                continue;
            }
        };
        match resolve_non_val_node(model, id) {
            Ok(node) if matches!(node, CrdtNode::Str(_)) => {}
            Ok(node) => diffs.push(format!(
                "{}: expected string to resolve to str node, got {}",
                format_path(&path),
                node_kind_name(node)
            )),
            Err(err) => diffs.push(format!("{}: {err}", format_path(&path))),
        }
    }

    for path in array_scalar_paths {
        let id = match find_path(model, model.root.val, &path) {
            Ok(id) => id,
            Err(err) => {
                diffs.push(format!(
                    "{}: array scalar path not found ({err})",
                    format_path(&path)
                ));
                continue;
            }
        };
        let node = match IndexExt::get(&model.index, &id) {
            Some(node) => node,
            None => {
                diffs.push(format!(
                    "{}: missing array element node",
                    format_path(&path)
                ));
                continue;
            }
        };
        match node {
            CrdtNode::Val(val) => {
                let child = match IndexExt::get(&model.index, &val.val) {
                    Some(node) => node,
                    None => {
                        diffs.push(format!(
                            "{}: val child node missing sid={}, time={}",
                            format_path(&path),
                            val.val.sid,
                            val.val.time
                        ));
                        continue;
                    }
                };
                if !matches!(child, CrdtNode::Con(_)) {
                    diffs.push(format!(
                        "{}: expected array scalar val child to be con, got {}",
                        format_path(&path),
                        node_kind_name(child)
                    ));
                }
            }
            other => diffs.push(format!(
                "{}: expected array scalar node kind val, got {}",
                format_path(&path),
                node_kind_name(other)
            )),
        }
    }

    diffs
}

fn run_model_api_proxy_fanout_workflow(xfails: &[&str]) {
    let scenario = "model_api_proxy_fanout_workflow";
    let manifest = load_manifest();
    let dir = common::fixtures::fixtures_dir();

    let entries: Vec<_> = manifest
        .fixtures
        .iter()
        .filter(|entry| entry.scenario == scenario)
        .cloned()
        .collect();
    assert!(
        !entries.is_empty(),
        "no {scenario} fixtures found in manifest"
    );

    let known: BTreeSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    for xfail in xfails {
        assert!(known.contains(xfail), "stale xfail for {scenario}: {xfail}");
    }

    let xfail_set: BTreeSet<&str> = xfails.iter().copied().collect();
    let mut hard_failures = Vec::<String>::new();
    let mut unexpected_pass = Vec::<String>::new();

    for entry in entries {
        let record = load_fixture_record(&dir, &entry);
        let expected_full = record
            .fixture
            .get("expected")
            .cloned()
            .unwrap_or_else(|| Value::Null);
        let expected =
            select_expected_fields(&expected_full, &["steps", "final_view_json", "fanout"]);

        let result = match evaluate_fixture(scenario, &record.fixture) {
            Ok(actual) => {
                let mut diffs = compare_expected_fields(&expected, &actual);
                if diffs.is_empty() {
                    let final_view = actual
                        .get("final_view_json")
                        .ok_or_else(|| "missing final_view_json".to_string());
                    let final_model_hex = actual
                        .get("final_model_binary_hex")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "missing final_model_binary_hex".to_string())
                        .and_then(decode_hex);
                    match (final_view, final_model_hex) {
                        (Ok(view), Ok(bytes)) => {
                            let decoded = structural_binary::decode(&bytes)
                                .map_err(|e| format!("model decode error: {e:?}"));
                            match decoded {
                                Ok(model) => {
                                    let kind_diffs = critical_node_kind_diffs(&model, view);
                                    if !kind_diffs.is_empty() {
                                        diffs.extend(kind_diffs);
                                    }
                                }
                                Err(err) => diffs.push(err),
                            }
                        }
                        (Err(err), _) | (_, Err(err)) => diffs.push(err),
                    }
                }

                if diffs.is_empty() {
                    Ok(())
                } else {
                    Err(diffs.join("\n  - "))
                }
            }
            Err(err) => Err(format!("runtime error: {err}")),
        };

        let expected_fail = xfail_set.contains(entry.name.as_str());
        match (expected_fail, result) {
            (false, Ok(())) => {}
            (false, Err(err)) => hard_failures.push(format!("{}:\n  - {}", entry.name, err)),
            (true, Ok(())) => unexpected_pass.push(entry.name),
            (true, Err(_)) => {}
        }
    }

    assert!(
        unexpected_pass.is_empty(),
        "{scenario} unexpected pass fixtures:\n{}",
        unexpected_pass.join("\n")
    );
    assert!(
        hard_failures.is_empty(),
        "{scenario} parity failures:\n{}",
        hard_failures.join("\n")
    );
}

#[test]
fn model_api_proxy_fanout_workflow_matches_upstream_fixtures() {
    run_model_api_proxy_fanout_workflow(MODEL_API_PROXY_FANOUT_WORKFLOW_XFAILS);
}
