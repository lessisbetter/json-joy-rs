use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use json_joy_core::patch_compaction::{combine_patches, compact_patch};
use serde_json::Value;

#[test]
fn differential_patch_compaction_seeded_matches_oracle() {
    let cases = build_cases();
    assert!(!cases.is_empty(), "expected compaction cases");

    for (idx, patches) in cases.iter().enumerate() {
        let oracle = oracle_compaction(
            &patches.iter().map(|p| hex(&p.to_binary())).collect::<Vec<_>>(),
        );

        let rust_combined = combine_patches(patches)
            .unwrap_or_else(|e| panic!("combine_patches failed for case {idx}: {e}"));
        let rust_compacted = compact_patch(&patches[0])
            .unwrap_or_else(|e| panic!("compact_patch failed for case {idx}: {e}"));

        assert_eq!(
            hex(&rust_combined.to_binary()),
            oracle["combined_hex"]
                .as_str()
                .expect("oracle combined_hex must be string"),
            "combined patch mismatch for case {idx}"
        );
        assert_eq!(
            hex(&rust_compacted.to_binary()),
            oracle["compacted_hex"]
                .as_str()
                .expect("oracle compacted_hex must be string"),
            "compacted patch mismatch for case {idx}"
        );
    }
}

fn build_cases() -> Vec<Vec<Patch>> {
    let sid = 94001;

    let p1 = patch_from_ops(
        sid,
        1,
        vec![
            DecodedOp::NewStr {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 3 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 1 },
                data: "ab".to_string(),
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 5 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 4 },
                data: "cd".to_string(),
            },
        ],
    );

    let p2 = patch_from_ops(
        sid,
        10,
        vec![DecodedOp::NewCon {
            id: Timestamp { sid, time: 10 },
            value: ConValue::Json(Value::from(7)),
        }],
    );

    let p3 = patch_from_ops(
        sid,
        11,
        vec![DecodedOp::Nop {
            id: Timestamp { sid, time: 11 },
            len: 2,
        }],
    );

    let sid2 = 94002;
    let q1 = patch_from_ops(
        sid2,
        1,
        vec![
            DecodedOp::NewObj {
                id: Timestamp { sid: sid2, time: 1 },
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: sid2, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: Timestamp { sid: sid2, time: 1 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid: sid2, time: 3 },
                value: ConValue::Json(Value::String("x".to_string())),
            },
            DecodedOp::InsObj {
                id: Timestamp { sid: sid2, time: 4 },
                obj: Timestamp { sid: sid2, time: 1 },
                data: vec![("k".to_string(), Timestamp { sid: sid2, time: 3 })],
            },
        ],
    );
    let q2 = patch_from_ops(
        sid2,
        5,
        vec![DecodedOp::NewCon {
            id: Timestamp { sid: sid2, time: 5 },
            value: ConValue::Json(Value::from(8)),
        }],
    );

    let mut out = vec![vec![p1.clone()], vec![p1, p2, p3], vec![q1, q2]];

    // Keep this suite deterministic and broad enough to mirror the seeded
    // differential depth used in other runtime-core parity tests.
    for i in 0..37u64 {
        let sidx = 94100 + i;
        let r1 = patch_from_ops(
            sidx,
            1,
            vec![
                DecodedOp::NewStr {
                    id: Timestamp { sid: sidx, time: 1 },
                },
                DecodedOp::InsVal {
                    id: Timestamp { sid: sidx, time: 2 },
                    obj: Timestamp { sid: 0, time: 0 },
                    val: Timestamp { sid: sidx, time: 1 },
                },
                DecodedOp::InsStr {
                    id: Timestamp { sid: sidx, time: 3 },
                    obj: Timestamp { sid: sidx, time: 1 },
                    reference: Timestamp { sid: sidx, time: 1 },
                    data: "a".to_string(),
                },
                DecodedOp::InsStr {
                    id: Timestamp { sid: sidx, time: 4 },
                    obj: Timestamp { sid: sidx, time: 1 },
                    reference: Timestamp { sid: sidx, time: 3 },
                    data: "b".to_string(),
                },
            ],
        );
        let r2 = patch_from_ops(
            sidx,
            5,
            vec![DecodedOp::Nop {
                id: Timestamp { sid: sidx, time: 5 },
                len: 1 + (i % 3),
            }],
        );
        let r3 = patch_from_ops(
            sidx,
            8,
            vec![DecodedOp::NewCon {
                id: Timestamp { sid: sidx, time: 8 },
                value: ConValue::Json(Value::from((i as i64) - 10)),
            }],
        );
        out.push(vec![r1, r2, r3]);
    }

    out
}

fn patch_from_ops(sid: u64, time: u64, ops: Vec<DecodedOp>) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, &ops).expect("encode_patch_from_ops must succeed");
    Patch::from_binary(&bytes).expect("Patch::from_binary must succeed")
}

fn oracle_cwd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

fn oracle_compaction(patches_binary_hex: &[String]) -> Value {
    let script = r#"
const patchLib = require('json-joy/lib/json-crdt-patch/index.js');
const {combine, compact} = require('json-joy/lib/json-crdt-patch/compaction.js');
const input = JSON.parse(process.argv[1]);
const patchHexes = input.patches_binary_hex;
const patches = patchHexes.map((h) => patchLib.Patch.fromBinary(Buffer.from(h, 'hex')));
const combineInput = patchHexes.map((h) => patchLib.Patch.fromBinary(Buffer.from(h, 'hex')));
combine(combineInput);
const compactInput = patchLib.Patch.fromBinary(Buffer.from(patchHexes[0], 'hex'));
compact(compactInput);
process.stdout.write(JSON.stringify({
  combined_hex: Buffer.from(combineInput[0].toBinary()).toString('hex'),
  compacted_hex: Buffer.from(compactInput.toBinary()).toString('hex'),
}));
"#;

    let payload = serde_json::json!({
        "patches_binary_hex": patches_binary_hex,
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run patch compaction oracle script");

    assert!(
        output.status.success(),
        "patch compaction oracle script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("oracle patch compaction output must be valid json")
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
