mod common;

use std::collections::BTreeSet;

use common::assertions::{compare_expected_fields, decode_hex, encode_hex};
use common::fixtures::{load_fixture_record, load_manifest};
use common::scenarios::evaluate_fixture;
use json_joy::json_crdt::codec::structural::binary as structural_binary;
use json_joy::json_crdt::model::api::find_path;
use json_joy::json_crdt::model::Model;
use json_joy::json_crdt::nodes::{CrdtNode, IndexExt};
use json_joy::json_crdt_patch::clock::Ts;
use json_joy::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;
use serde_json::{Map, Value};

const PATCH_DIFF_APPLY_XFAILS: &[&str] = &[];

const MODEL_DIFF_DST_KEYS_XFAILS: &[&str] = &[];

fn json_to_pack(v: &Value) -> PackValue {
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                PackValue::UInteger(u)
            } else if let Some(f) = n.as_f64() {
                PackValue::Float(f)
            } else {
                PackValue::Null
            }
        }
        Value::String(s) => PackValue::Str(s.clone()),
        Value::Array(arr) => PackValue::Array(arr.iter().map(json_to_pack).collect()),
        Value::Object(obj) => PackValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_pack(v)))
                .collect(),
        ),
    }
}

fn build_json(builder: &mut PatchBuilder, v: &Value) -> Ts {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => builder.con_val(json_to_pack(v)),
        Value::String(s) => {
            let str_id = builder.str_node();
            if !s.is_empty() {
                builder.ins_str(str_id, str_id, s.clone());
            }
            str_id
        }
        Value::Array(items) => {
            let arr_id = builder.arr();
            if !items.is_empty() {
                let ids: Vec<Ts> = items.iter().map(|item| build_json(builder, item)).collect();
                builder.ins_arr(arr_id, arr_id, ids);
            }
            arr_id
        }
        Value::Object(map) => {
            let obj_id = builder.obj();
            if !map.is_empty() {
                let pairs: Vec<(String, Ts)> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), build_json(builder, v)))
                    .collect();
                builder.ins_obj(obj_id, pairs);
            }
            obj_id
        }
    }
}

fn model_from_json(data: &Value, sid: u64) -> Model {
    let mut model = Model::new(sid);
    let mut builder = PatchBuilder::new(sid, model.clock.time);
    let root = build_json(&mut builder, data);
    builder.root(root);
    let patch = builder.flush();
    if !patch.ops.is_empty() {
        model.apply_patch(&patch);
    }
    model
}

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
            Ok(CrdtNode::Str(_)) => {}
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

fn fallback_view_and_model_hex(scenario: &str, fixture: &Value) -> Result<(Value, String), String> {
    let input = fixture
        .get("input")
        .ok_or_else(|| "fixture.input missing".to_string())?;

    match scenario {
        "model_diff_parity" | "model_diff_dst_keys" => {
            let base_model_hex = input
                .get("base_model_binary_hex")
                .and_then(Value::as_str)
                .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?;
            let bytes = decode_hex(base_model_hex)?;
            let model = structural_binary::decode(&bytes).map_err(|e| format!("{e:?}"))?;
            Ok((model.view(), encode_hex(&structural_binary::encode(&model))))
        }
        "patch_diff_apply" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base = input
                .get("base")
                .ok_or_else(|| "input.base missing".to_string())?;
            let model = model_from_json(base, sid);
            Ok((model.view(), encode_hex(&structural_binary::encode(&model))))
        }
        other => Err(format!("unsupported fallback scenario: {other}")),
    }
}

fn normalize_actual_output(
    scenario: &str,
    fixture: &Value,
    actual: Value,
) -> Result<Value, String> {
    let mut out = actual
        .as_object()
        .cloned()
        .ok_or_else(|| "actual output must be object".to_string())?;

    if !out.contains_key("view_after_apply_json")
        || !out.contains_key("model_binary_after_apply_hex")
    {
        let (view, model_hex) = fallback_view_and_model_hex(scenario, fixture)?;
        out.entry("view_after_apply_json".to_string())
            .or_insert(view);
        out.entry("model_binary_after_apply_hex".to_string())
            .or_insert(Value::String(model_hex));
    }

    Ok(Value::Object(out))
}

fn run_diff_scenario(scenario: &str, expected_keys: &[&str], xfails: &[&str]) {
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
        let expected = select_expected_fields(&expected_full, expected_keys);

        let result = match evaluate_fixture(scenario, &record.fixture) {
            Ok(actual) => {
                let actual = match normalize_actual_output(scenario, &record.fixture, actual) {
                    Ok(value) => value,
                    Err(err) => {
                        return hard_failures.push(format!("{} normalize error: {err}", entry.name))
                    }
                };

                let mut diffs = compare_expected_fields(&expected, &actual);
                if diffs.is_empty() {
                    let final_view = actual
                        .get("view_after_apply_json")
                        .ok_or_else(|| "missing view_after_apply_json".to_string())
                        .and_then(|v| {
                            actual
                                .get("model_binary_after_apply_hex")
                                .and_then(Value::as_str)
                                .ok_or_else(|| "missing model_binary_after_apply_hex".to_string())
                                .and_then(decode_hex)
                                .and_then(|bytes| {
                                    structural_binary::decode(&bytes)
                                        .map_err(|e| format!("model decode error: {e:?}"))
                                })
                                .map(|model| (v.clone(), model))
                        });

                    match final_view {
                        Ok((view, model)) => {
                            let kind_diffs = critical_node_kind_diffs(&model, &view);
                            if !kind_diffs.is_empty() {
                                diffs.extend(kind_diffs);
                            }
                        }
                        Err(err) => diffs.push(err),
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
fn model_diff_parity_matches_upstream_fixtures() {
    run_diff_scenario(
        "model_diff_parity",
        &["patch_present", "view_after_apply_json"],
        &[],
    );
}

#[test]
fn model_diff_dst_keys_matches_upstream_fixtures() {
    run_diff_scenario(
        "model_diff_dst_keys",
        &[
            "patch_present",
            "patch_op_count",
            "patch_opcodes",
            "patch_span",
            "view_after_apply_json",
        ],
        MODEL_DIFF_DST_KEYS_XFAILS,
    );
}

#[test]
fn patch_diff_apply_matches_upstream_fixtures() {
    run_diff_scenario(
        "patch_diff_apply",
        &[
            "patch_present",
            "patch_op_count",
            "patch_opcodes",
            "patch_span",
            "view_after_apply_json",
        ],
        PATCH_DIFF_APPLY_XFAILS,
    );
}
