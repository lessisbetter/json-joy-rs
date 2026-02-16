use std::path::Path;
use std::process::Command;

use json_joy_core::crdt_binary::first_logical_clock_sid_time;
use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use serde_json::{Map, Value};

#[test]
fn upstream_port_diff_nonempty_recursive_matrix_matches_oracle_bytes() {
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
                "rows": [{"k": "a", "n": 1}, {"k": "b", "n": 2}]
            },
            "count": 7
        }),
    ];
    let seeds = [0x1111u64, 0x2222, 0x3333, 0x4444, 0x5555];

    for (base_idx, base) in bases.into_iter().enumerate() {
        let sid = 93000 + base_idx as u64;
        let model = create_model(&base, sid).expect("create_model must succeed");
        let base_model = model_to_binary(&model);
        let sid = first_logical_clock_sid_time(&base_model)
            .map(|(s, _)| s)
            .unwrap_or(sid);

        for seed in seeds {
            let mut rng = Lcg::new(seed ^ sid);
            for _ in 0..24 {
                let next = mutate_recursive(&mut rng, &base, 0);
                let rust = diff_model_to_patch_bytes(&base_model, &next, sid)
                    .expect("rust diff should succeed");
                let oracle = oracle_diff(&base_model, &next, sid);

                let present = oracle["patch_present"]
                    .as_bool()
                    .expect("oracle patch_present must be bool");
                assert_eq!(
                    rust.is_some(),
                    present,
                    "patch presence mismatch (base_idx={base_idx}, seed={seed})"
                );
                if let Some(bytes) = rust {
                    let oracle_bytes = decode_hex(
                        oracle["patch_binary_hex"]
                            .as_str()
                            .expect("oracle patch_binary_hex must be string"),
                    );
                    assert_eq!(
                        bytes, oracle_bytes,
                        "patch bytes mismatch (base_idx={base_idx}, seed={seed})"
                    );
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
    if rng.range(7) == 0 {
        out.insert(format!("k{}", rng.range(8)), random_leaf(rng));
    }
    if !out.is_empty() && rng.range(8) == 0 {
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
    if rng.range(6) == 0 {
        out.push(random_leaf(rng));
    }
    if !out.is_empty() && rng.range(9) == 0 {
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
    match rng.range(5) {
        0 => Value::Null,
        1 => Value::Bool(rng.range(2) == 0),
        2 => Value::Number(serde_json::Number::from((rng.range(100) as i64) - 30)),
        3 => Value::String(format!("s{}", rng.range(1000))),
        _ => Value::Array(vec![
            Value::Number(serde_json::Number::from(rng.range(9) as i64)),
            Value::String(format!("x{}", rng.range(9))),
        ]),
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

fn oracle_diff(base_model: &[u8], next_view: &Value, sid: u64) -> Value {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
        .join("diff-runtime.cjs");

    let payload = serde_json::json!({
        "base_model_binary_hex": hex(base_model),
        "next_view_json": next_view,
        "sid": sid,
    });

    let output = Command::new("node")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run oracle diff script");

    assert!(
        output.status.success(),
        "oracle diff script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle diff output must be valid json")
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
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
