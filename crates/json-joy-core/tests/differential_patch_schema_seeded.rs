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
    let cases: Vec<Value> = vec![
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

    let parsed: Value = serde_json::from_slice(&output.stdout).expect("schema oracle output must be json");
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
