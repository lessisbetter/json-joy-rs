use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use json_joy_core::codec_indexed_binary::{encode_model_binary_to_fields, IndexedFields};
use json_joy_core::codec_sidecar_binary::encode_model_binary_to_sidecar;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use serde_json::Value;

#[test]
fn differential_codec_seeded_matches_oracle_encoders() {
    let seeds = seeds();
    for (i, seed) in seeds.iter().enumerate() {
        let sid = 91000 + i as u64;
        let value = random_json(*seed, 4);
        let model = create_model(&value, sid).expect("create_model must succeed");
        let binary = model_to_binary(&model);

        let rust_indexed = encode_model_binary_to_fields(&binary).expect("indexed encode must succeed");
        let oracle_indexed = oracle_indexed_encode(&binary);
        assert_eq!(fields_to_hex(&rust_indexed), oracle_indexed, "indexed codec mismatch seed={seed}");

        let (rust_view, rust_meta) =
            encode_model_binary_to_sidecar(&binary).expect("sidecar encode must succeed");
        let (oracle_view, oracle_meta) = oracle_sidecar_encode(&binary);
        assert_eq!(hex(&rust_view), oracle_view, "sidecar view mismatch seed={seed}");
        assert_eq!(hex(&rust_meta), oracle_meta, "sidecar meta mismatch seed={seed}");
    }
}

fn oracle_indexed_encode(model_binary: &[u8]) -> BTreeMap<String, String> {
    let script = r#"
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {Encoder} = require('json-joy/lib/json-crdt/codec/indexed/binary/Encoder.js');
const input = JSON.parse(process.argv[1]);
const model = Model.fromBinary(Buffer.from(input.model_binary_hex, 'hex'));
const fields = new Encoder().encode(model);
const out = {};
for (const [k, v] of Object.entries(fields)) out[k] = Buffer.from(v).toString('hex');
process.stdout.write(JSON.stringify(out));
"#;

    let payload = serde_json::json!({
        "model_binary_hex": hex(model_binary),
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run indexed codec oracle");
    assert!(
        output.status.success(),
        "indexed codec oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("indexed oracle output must be json")
}

fn oracle_sidecar_encode(model_binary: &[u8]) -> (String, String) {
    let script = r#"
const {Model} = require('json-joy/lib/json-crdt/index.js');
const {Encoder} = require('json-joy/lib/json-crdt/codec/sidecar/binary/Encoder.js');
const input = JSON.parse(process.argv[1]);
const model = Model.fromBinary(Buffer.from(input.model_binary_hex, 'hex'));
const [view, meta] = new Encoder().encode(model);
process.stdout.write(JSON.stringify({view_hex: Buffer.from(view).toString('hex'), meta_hex: Buffer.from(meta).toString('hex')}));
"#;

    let payload = serde_json::json!({
        "model_binary_hex": hex(model_binary),
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run sidecar codec oracle");
    assert!(
        output.status.success(),
        "sidecar codec oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out: Value = serde_json::from_slice(&output.stdout).expect("sidecar oracle output must be json");
    (
        out["view_hex"]
            .as_str()
            .expect("view_hex must be string")
            .to_string(),
        out["meta_hex"]
            .as_str()
            .expect("meta_hex must be string")
            .to_string(),
    )
}

fn fields_to_hex(fields: &IndexedFields) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in fields {
        out.insert(k.clone(), hex(v));
    }
    out
}

fn oracle_cwd() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

fn seeds() -> [u64; 40] {
    [
        0x5eed_c0de_u64,
        0x0000_0000_0000_0001_u64,
        0x0000_0000_0000_00ff_u64,
        0x0000_0000_00c0_ffee_u64,
        0x0123_4567_89ab_cdef_u64,
        0x0000_0000_0000_1001_u64,
        0x0000_0000_0000_2002_u64,
        0x0000_0000_0000_3003_u64,
        0x0000_0000_0000_4004_u64,
        0x0000_0000_0000_5005_u64,
        0x1111_2222_3333_4444_u64,
        0x2222_3333_4444_5555_u64,
        0x3333_4444_5555_6666_u64,
        0x4444_5555_6666_7777_u64,
        0x5555_6666_7777_8888_u64,
        0x89ab_cdef_0123_4567_u64,
        0xfedc_ba98_7654_3210_u64,
        0x1357_9bdf_2468_ace0_u64,
        0x0f0f_f0f0_55aa_aa55_u64,
        0xa5a5_5a5a_dead_beef_u64,
        0x0101_0101_0101_0101_u64,
        0x0202_0202_0202_0202_u64,
        0x0303_0303_0303_0303_u64,
        0x0404_0404_0404_0404_u64,
        0x0505_0505_0505_0505_u64,
        0x0606_0606_0606_0606_u64,
        0x0707_0707_0707_0707_u64,
        0x0808_0808_0808_0808_u64,
        0x0909_0909_0909_0909_u64,
        0x0a0a_0a0a_0a0a_0a0a_u64,
        0xbeef_dead_0000_0001_u64,
        0xbeef_dead_0000_0002_u64,
        0xbeef_dead_0000_0003_u64,
        0x7777_7777_8888_8888_u64,
        0x8888_8888_9999_9999_u64,
        0x9999_9999_aaaa_aaaa_u64,
        0xaaaa_aaaa_bbbb_bbbb_u64,
        0xbbbb_bbbb_cccc_cccc_u64,
        0xcccc_cccc_dddd_dddd_u64,
        0xdddd_dddd_eeee_eeee_u64,
    ]
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

fn random_json(seed: u64, depth: usize) -> Value {
    let mut rng = Lcg::new(seed);
    random_object(&mut rng, depth)
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
