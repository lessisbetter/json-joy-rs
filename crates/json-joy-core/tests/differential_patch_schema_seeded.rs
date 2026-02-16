use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::schema;
use serde_json::{json, Value};

fn oracle_cwd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

#[test]
fn differential_patch_schema_seeded_matches_oracle() {
    let sid = 990_123u64;
    let time = 1u64;
    let mut cases: Vec<Value> = vec![
        json!(null),
        json!(true),
        json!(123),
        json!("hello"),
        json!([1, 2, 3]),
        json!([1, {"a": 2}, [3, 4]]),
        json!({"a": 1}),
        json!({"a": 1, "b": "x", "c": false}),
        json!({"nested": {"arr": [1, 2, {"x": "y"}]}}),
        json!({"emoji": "👨‍🍳", "zwj": "A👩‍💻B"}),
        json!({"mix": [null, true, 0, "s", {"k": [1, 2]}]}),
    ];
    let mut rng = Lcg::new(0x9912_3001);
    while cases.len() < 40 {
        cases.push(random_json(&mut rng, 4));
    }

    for (idx, case) in cases.iter().enumerate() {
        let rust_patch = schema::json(case)
            .to_patch(sid, time)
            .expect("schema::json to_patch must succeed");
        let rust_hex = hex(&rust_patch.to_binary());
        let oracle_hex = oracle_schema_json_patch_hex(case, sid, time);
        assert_eq!(
            rust_hex, oracle_hex,
            "schema patch binary mismatch at case {idx}"
        );
    }
}

fn oracle_schema_json_patch_hex(value: &Value, sid: u64, time: u64) -> String {
    let script = r#"
const {PatchBuilder, s} = require('json-joy/lib/json-crdt-patch');
const {ClockVector} = require('json-joy/lib/json-crdt-patch/clock/clock');
const input = JSON.parse(process.argv[1]);
const b = new PatchBuilder(new ClockVector(input.sid, input.time));
const root = s.json(input.value).build(b);
b.setVal({sid: 0, time: 0}, root);
const patch = b.flush();
process.stdout.write(JSON.stringify({ patch_hex: Buffer.from(patch.toBinary()).toString('hex') }));
"#;

    let payload = serde_json::json!({
        "value": value,
        "sid": sid,
        "time": time,
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run schema oracle script");

    assert!(
        output.status.success(),
        "schema oracle script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: Value =
        serde_json::from_slice(&output.stdout).expect("schema oracle output must be json");
    parsed["patch_hex"]
        .as_str()
        .expect("schema oracle patch_hex must be string")
        .to_string()
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

fn random_scalar(rng: &mut Lcg) -> Value {
    match rng.range(5) {
        0 => Value::Null,
        1 => Value::Bool(rng.range(2) == 1),
        2 => Value::Number(serde_json::Number::from((rng.range(100) as i64) - 30)),
        3 => Value::String(format!("s{}", rng.range(1000))),
        _ => Value::String(String::new()),
    }
}

fn random_json(rng: &mut Lcg, depth: usize) -> Value {
    if depth == 0 {
        return random_scalar(rng);
    }
    match rng.range(4) {
        0 => random_scalar(rng),
        1 => {
            let len = rng.range(5) as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(random_json(rng, depth - 1));
            }
            Value::Array(arr)
        }
        _ => {
            let len = (1 + rng.range(4)) as usize;
            let mut map = serde_json::Map::new();
            for i in 0..len {
                map.insert(format!("k{}", i), random_json(rng, depth - 1));
            }
            Value::Object(map)
        }
    }
}
