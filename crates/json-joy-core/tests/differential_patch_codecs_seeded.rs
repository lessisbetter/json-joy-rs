use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::patch::Patch;
use json_joy_core::patch_compact_binary_codec::{
    decode_patch_compact_binary, encode_patch_compact_binary,
};
use json_joy_core::patch_compact_codec::{decode_patch_compact, encode_patch_compact};
use json_joy_core::patch_verbose_codec::{decode_patch_verbose, encode_patch_verbose};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn read_json(path: &Path) -> Value {
    let data =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
}

fn oracle_cwd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

#[test]
fn differential_patch_codecs_seeded_match_oracle() {
    let patch_hexes = collect_patch_samples(40);
    assert!(
        !patch_hexes.is_empty(),
        "expected at least one patch sample"
    );

    for (idx, patch_hex) in patch_hexes.iter().enumerate() {
        let patch_bytes = decode_hex(patch_hex);
        let patch = Patch::from_binary(&patch_bytes).unwrap_or_else(|e| {
            panic!("fixture patch decode failed at sample {idx}: {e}");
        });

        let rust_compact = encode_patch_compact(&patch).expect("rust compact encode must succeed");
        let rust_verbose = encode_patch_verbose(&patch).expect("rust verbose encode must succeed");
        let rust_compact_binary =
            encode_patch_compact_binary(&patch).expect("rust compact-binary encode must succeed");

        let oracle = oracle_patch_codecs(patch_hex);

        assert_eq!(
            rust_compact, oracle["compact"],
            "compact payload mismatch at sample {idx}"
        );
        assert_eq!(
            rust_verbose, oracle["verbose"],
            "verbose payload mismatch at sample {idx}"
        );
        assert_eq!(
            hex(&rust_compact_binary),
            oracle["compact_binary_hex"]
                .as_str()
                .expect("oracle compact_binary_hex must be string"),
            "compact-binary payload mismatch at sample {idx}"
        );

        let roundtrip_from_rust_compact = decode_patch_compact(&rust_compact)
            .expect("decode rust compact must succeed")
            .to_binary();
        let roundtrip_from_oracle_compact = decode_patch_compact(&oracle["compact"])
            .expect("decode oracle compact must succeed")
            .to_binary();

        let roundtrip_from_rust_verbose = decode_patch_verbose(&rust_verbose)
            .expect("decode rust verbose must succeed")
            .to_binary();
        let roundtrip_from_oracle_verbose = decode_patch_verbose(&oracle["verbose"])
            .expect("decode oracle verbose must succeed")
            .to_binary();

        let roundtrip_from_rust_compact_bin = decode_patch_compact_binary(&rust_compact_binary)
            .expect("decode rust compact-binary must succeed")
            .to_binary();
        let roundtrip_from_oracle_compact_bin = decode_patch_compact_binary(&decode_hex(
            oracle["compact_binary_hex"]
                .as_str()
                .expect("oracle compact_binary_hex must be string"),
        ))
        .expect("decode oracle compact-binary must succeed")
        .to_binary();

        let original = patch.to_binary();
        assert_eq!(
            roundtrip_from_rust_compact, original,
            "rust compact roundtrip mismatch at sample {idx}"
        );
        assert_eq!(
            roundtrip_from_oracle_compact, original,
            "oracle compact roundtrip mismatch at sample {idx}"
        );
        assert_eq!(
            roundtrip_from_rust_verbose, original,
            "rust verbose roundtrip mismatch at sample {idx}"
        );
        assert_eq!(
            roundtrip_from_oracle_verbose, original,
            "oracle verbose roundtrip mismatch at sample {idx}"
        );
        assert_eq!(
            roundtrip_from_rust_compact_bin, original,
            "rust compact-binary roundtrip mismatch at sample {idx}"
        );
        assert_eq!(
            roundtrip_from_oracle_compact_bin, original,
            "oracle compact-binary roundtrip mismatch at sample {idx}"
        );
    }
}

fn collect_patch_samples(limit: usize) -> Vec<String> {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
    let mut out = Vec::new();

    for entry in fixtures {
        if out.len() >= limit {
            break;
        }
        let scenario = entry["scenario"].as_str().expect("scenario must be string");
        let file = entry["file"].as_str().expect("file must be string");
        let fixture = read_json(&dir.join(file));
        match scenario {
            "patch_canonical_encode" => {
                out.push(
                    fixture["expected"]["patch_binary_hex"]
                        .as_str()
                        .expect("expected.patch_binary_hex must be string")
                        .to_string(),
                );
            }
            "patch_diff_apply" => {
                if fixture["expected"]["patch_present"].as_bool() == Some(true) {
                    out.push(
                        fixture["expected"]["patch_binary_hex"]
                            .as_str()
                            .expect("expected.patch_binary_hex must be string")
                            .to_string(),
                    );
                }
            }
            _ => {}
        }
    }

    out.sort();
    out.dedup();
    if out.len() > limit {
        out.truncate(limit);
    }
    out
}

fn oracle_patch_codecs(patch_binary_hex: &str) -> Value {
    let script = r#"
const patchLib = require('json-joy/lib/json-crdt-patch/index.js');
const compact = require('json-joy/lib/json-crdt-patch/codec/compact');
const verbose = require('json-joy/lib/json-crdt-patch/codec/verbose');
const compactBinary = require('json-joy/lib/json-crdt-patch/codec/compact-binary');
const input = JSON.parse(process.argv[1]);
const patch = patchLib.Patch.fromBinary(Buffer.from(input.patch_binary_hex, 'hex'));
process.stdout.write(JSON.stringify({
  compact: compact.encode(patch),
  verbose: verbose.encode(patch),
  compact_binary_hex: Buffer.from(compactBinary.encode(patch)).toString('hex'),
}));
"#;

    let payload = serde_json::json!({
        "patch_binary_hex": patch_binary_hex,
    });

    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run patch codec oracle script");

    assert!(
        output.status.success(),
        "patch codec oracle script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("oracle patch codec output must be valid json")
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
