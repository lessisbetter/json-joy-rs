use json_joy_core::crdt_binary::first_logical_clock_sid_time;
use json_joy_core::diff_runtime::{diff_model_to_patch_bytes, DiffError};
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::Patch;
use serde_json::{Map, Value};

#[test]
fn upstream_port_diff_native_support_matrix_avoids_unsupported_shape_for_logical_models() {
    let bases = [
        serde_json::json!({
            "doc": {
                "title": "hello",
                "meta": {"v": 1, "ok": true},
                "items": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]
            },
            "flag": false
        }),
        serde_json::json!({
            "root": {
                "profile": {"name": "nora", "age": 31},
                "tags": ["x", "y", "z"],
                "rows": [{"k": "a", "n": 1}, {"k": "b", "n": 2}],
                "blob": {"0": 1, "1": 2}
            },
            "count": 7
        }),
    ];
    let seeds = [0x1001u64, 0x2002, 0x3003, 0x4004, 0x5005];

    for (base_idx, base) in bases.into_iter().enumerate() {
        let sid = 94000 + base_idx as u64;
        let model = create_model(&base, sid).expect("create_model must succeed");
        let base_model = model_to_binary(&model);
        let sid = first_logical_clock_sid_time(&base_model)
            .map(|(s, _)| s)
            .unwrap_or(sid);

        for seed in seeds {
            let mut rng = Lcg::new(seed ^ sid);
            for _ in 0..48 {
                let next = mutate_recursive(&mut rng, &base, 0);
                let out = diff_model_to_patch_bytes(&base_model, &next, sid);
                match out {
                    Ok(Some(bytes)) => {
                        let patch =
                            Patch::from_binary(&bytes).expect("diff output patch must decode");
                        let mut runtime = RuntimeModel::from_model_binary(&base_model)
                            .expect("runtime decode must succeed");
                        runtime
                            .apply_patch(&patch)
                            .expect("runtime apply must succeed");
                        assert_eq!(
                            runtime.view_json(),
                            next,
                            "applied native diff must reach target view"
                        );
                    }
                    Ok(None) => {
                        assert_eq!(
                            base, next,
                            "native no-op diff is only valid for equal views"
                        );
                    }
                    Err(DiffError::UnsupportedShape) => {
                        panic!(
                            "unexpected unsupported shape for logical model (base_idx={base_idx}, seed={seed})"
                        );
                    }
                    Err(e) => panic!("unexpected diff error: {e}"),
                }
            }
        }
    }
}

fn mutate_recursive(rng: &mut Lcg, value: &Value, depth: u32) -> Value {
    if depth >= 3 {
        return mutate_leaf(rng, value);
    }
    match value {
        Value::Object(map) => mutate_object(rng, map, depth + 1),
        Value::Array(items) => mutate_array(rng, items, depth + 1),
        _ => mutate_leaf(rng, value),
    }
}

fn mutate_object(rng: &mut Lcg, map: &Map<String, Value>, depth: u32) -> Value {
    let mut out = map.clone();
    for (k, v) in map {
        if rng.range(4) == 0 {
            out.insert(k.clone(), mutate_recursive(rng, v, depth));
        }
    }
    if rng.range(9) == 0 {
        out.insert(format!("k{}", rng.range(8)), random_leaf(rng));
    }
    if !out.is_empty() && rng.range(10) == 0 {
        let key = out
            .keys()
            .next()
            .cloned()
            .expect("non-empty map must have key");
        out.remove(&key);
    }
    Value::Object(out)
}

fn mutate_array(rng: &mut Lcg, items: &[Value], depth: u32) -> Value {
    let mut out = items.to_vec();
    if !out.is_empty() {
        let i = rng.range(out.len() as u64) as usize;
        out[i] = mutate_recursive(rng, &out[i], depth);
    }
    if rng.range(8) == 0 {
        out.push(random_leaf(rng));
    }
    if !out.is_empty() && rng.range(11) == 0 {
        let i = rng.range(out.len() as u64) as usize;
        out.remove(i);
    }
    Value::Array(out)
}

fn mutate_leaf(rng: &mut Lcg, v: &Value) -> Value {
    match v {
        Value::Null => Value::Bool(true),
        Value::Bool(b) => Value::Bool(!b),
        Value::Number(_) => Value::Number(serde_json::Number::from((rng.range(100) as i64) - 30)),
        Value::String(s) => mutate_string(rng, s),
        Value::Array(_) | Value::Object(_) => random_leaf(rng),
    }
}

fn random_leaf(rng: &mut Lcg) -> Value {
    match rng.range(6) {
        0 => Value::Null,
        1 => Value::Bool(rng.range(2) == 0),
        2 => Value::Number(serde_json::Number::from((rng.range(100) as i64) - 30)),
        3 => Value::String(format!("s{}", rng.range(1000))),
        4 => Value::Array(vec![
            Value::Number(serde_json::Number::from(rng.range(9) as i64)),
            Value::String(format!("x{}", rng.range(9))),
        ]),
        _ => Value::Object(Map::from_iter([(
            "n".to_string(),
            Value::Number(serde_json::Number::from((rng.range(20) as i64) - 7)),
        )])),
    }
}

fn mutate_string(rng: &mut Lcg, s: &str) -> Value {
    if s.is_empty() {
        return Value::String(format!("s{}", rng.range(100)));
    }
    let mut chars: Vec<char> = s.chars().collect();
    match rng.range(3) {
        0 => {
            let idx = rng.range(chars.len() as u64) as usize;
            chars[idx] = (b'a' + (rng.range(26) as u8)) as char;
        }
        1 => {
            let idx = rng.range(chars.len() as u64) as usize;
            chars.remove(idx);
            if chars.is_empty() {
                chars.push((b'a' + (rng.range(26) as u8)) as char);
            }
        }
        _ => {
            let idx = rng.range((chars.len() as u64) + 1) as usize;
            chars.insert(idx, (b'a' + (rng.range(26) as u8)) as char);
        }
    }
    Value::String(chars.into_iter().collect())
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn range(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
}
