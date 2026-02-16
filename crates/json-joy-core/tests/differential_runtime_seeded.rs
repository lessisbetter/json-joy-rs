use std::path::Path;
use std::process::Command;

use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::Patch;
use serde_json::Value;

#[test]
fn differential_runtime_seeded_diff_and_apply_match_oracle() {
    // model_roundtrip_empty_object_v1 fixture payload (sid=73012).
    let base_model = decode_hex("00000002114001b4ba0402");
    let sid = 73012;
    let seeds = [
        0x5eed_c0de_u64,
        0x0000_0000_0000_0001_u64,
        0x0000_0000_0000_00ff_u64,
        0x0000_0000_00c0_ffee_u64,
        0x0123_4567_89ab_cdef_u64,
    ];

    for seed in seeds {
        let mut rng = Lcg::new(seed);
        for _ in 0..30 {
            let next = random_object(&mut rng, 3);

            let rust_patch = diff_model_to_patch_bytes(&base_model, &next, sid)
                .expect("rust diff should succeed");
            let oracle_diff = oracle_diff(&base_model, &next, sid);

            let oracle_present = oracle_diff["patch_present"]
                .as_bool()
                .expect("oracle patch_present must be bool");
            if !oracle_present {
                assert!(
                    rust_patch.is_none(),
                    "rust returned patch while oracle returned none (seed={seed})"
                );
                continue;
            }

            let rust_patch = rust_patch.expect("rust patch expected");
            let oracle_patch = decode_hex(
                oracle_diff["patch_binary_hex"]
                    .as_str()
                    .expect("oracle patch hex must be string"),
            );
            assert_eq!(rust_patch, oracle_patch, "patch bytes mismatch vs oracle (seed={seed})");

            let mut runtime =
                RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
            let patch = Patch::from_binary(&rust_patch).expect("patch decode must succeed");
            runtime.apply_patch(&patch).expect("runtime apply must succeed");

            assert_eq!(
                runtime.view_json(),
                next,
                "runtime view should match next object (seed={seed})"
            );
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
        2 => Value::Number(serde_json::Number::from((rng.range(50) as i64) - 10)),
        3 => Value::String(format!("s{}", rng.range(100))),
        _ => Value::String("".to_string()),
    }
}

fn random_value(rng: &mut Lcg, depth: usize) -> Value {
    if depth == 0 {
        return random_scalar(rng);
    }
    match rng.range(4) {
        0 => random_scalar(rng),
        1 => {
            let len = rng.range(4) as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(random_value(rng, depth - 1));
            }
            Value::Array(arr)
        }
        _ => random_object(rng, depth - 1),
    }
}

fn random_object(rng: &mut Lcg, depth: usize) -> Value {
    let len = (1 + rng.range(4)) as usize;
    let mut map = serde_json::Map::new();
    for i in 0..len {
        map.insert(format!("k{}", i), random_value(rng, depth));
    }
    Value::Object(map)
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
