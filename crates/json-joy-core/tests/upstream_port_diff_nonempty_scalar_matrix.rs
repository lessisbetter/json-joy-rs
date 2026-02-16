use std::path::Path;
use std::process::Command;

use json_joy_core::crdt_binary::first_logical_clock_sid_time;
use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use serde_json::Value;

#[test]
fn upstream_port_diff_nonempty_scalar_matrix_matches_oracle_bytes() {
    let bases = [
        serde_json::json!({"a": 1, "flag": true, "n": null}),
        serde_json::json!({"score": 3, "ok": false, "count": 7}),
    ];
    let seeds = [0x51u64, 0x5eed_c0de, 0xc0ffee, 0x1234_5678_9abc_def0];

    for (base_idx, base) in bases.into_iter().enumerate() {
        let sid = 92000 + base_idx as u64;
        let model = create_model(&base, sid).expect("create_model must succeed");
        let base_model = model_to_binary(&model);
        let sid = first_logical_clock_sid_time(&base_model)
            .map(|(s, _)| s)
            .unwrap_or(sid);

        for seed in seeds {
            let mut rng = Lcg::new(seed ^ sid);
            for _ in 0..32 {
                let next = mutate_scalar_object(&mut rng, &base);
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

fn mutate_scalar_object(rng: &mut Lcg, base: &Value) -> Value {
    let mut out = base.as_object().cloned().expect("base must be object");
    for (k, v) in out.clone() {
        if rng.range(3) != 0 {
            continue;
        }
        let next = match v {
            Value::Null => Value::Number(serde_json::Number::from(rng.range(10) as i64)),
            Value::Bool(b) => Value::Bool(!b),
            Value::Number(_) => Value::Number(serde_json::Number::from((rng.range(50) as i64) - 10)),
            Value::String(_) => Value::String(format!("s{}", rng.range(100))),
            Value::Object(_) => Value::Object(serde_json::Map::from_iter([(
                "id".to_string(),
                Value::String(format!("d{}", rng.range(100))),
            )])),
            Value::Array(_) => Value::Array(Vec::new()),
        };
        out.insert(k, next);
    }
    if rng.range(6) == 0 {
        out.insert(
            format!("k{}", rng.range(8)),
            Value::Number(serde_json::Number::from((rng.range(100) as i64) - 20)),
        );
    }
    if !out.is_empty() && rng.range(5) == 0 {
        let key = out.keys().next().cloned().expect("non-empty map must have key");
        out.remove(&key);
    }
    Value::Object(out)
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
