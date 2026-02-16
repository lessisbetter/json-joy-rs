use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::model_api::NativeModelApi;
use json_joy_core::patch::Patch;
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
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[test]
fn model_lifecycle_workflow_fixtures_match_expected_outputs() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures must be array");
    let mut seen = 0u32;

    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_lifecycle_workflow") {
            continue;
        }
        seen += 1;
        let name = entry["name"].as_str().expect("fixture name must be string");
        let file = entry["file"].as_str().expect("fixture file must be string");
        let fixture = read_json(&dir.join(file));
        let workflow = fixture["input"]["workflow"]
            .as_str()
            .expect("input.workflow must be string");

        let batch_hex = fixture["input"]["batch_patches_binary_hex"]
            .as_array()
            .expect("input.batch_patches_binary_hex must be array");
        let mut batch = Vec::with_capacity(batch_hex.len());
        for h in batch_hex {
            let bytes = decode_hex(h.as_str().expect("batch patch hex must be string"));
            batch.push(
                Patch::from_binary(&bytes)
                    .unwrap_or_else(|e| panic!("batch patch decode failed for {name}: {e}")),
            );
        }

        let mut api = match workflow {
            "from_patches_apply_batch" => {
                let seed_hex = fixture["input"]["seed_patches_binary_hex"]
                    .as_array()
                    .expect("input.seed_patches_binary_hex must be array");
                let mut seed = Vec::with_capacity(seed_hex.len());
                for h in seed_hex {
                    let bytes = decode_hex(h.as_str().expect("seed patch hex must be string"));
                    seed.push(
                        Patch::from_binary(&bytes)
                            .unwrap_or_else(|e| panic!("seed patch decode failed for {name}: {e}")),
                    );
                }
                NativeModelApi::from_patches(&seed)
                    .unwrap_or_else(|e| panic!("from_patches failed for {name}: {e}"))
            }
            "load_apply_batch" => {
                let base = decode_hex(
                    fixture["input"]["base_model_binary_hex"]
                        .as_str()
                        .expect("input.base_model_binary_hex must be string"),
                );
                let load_sid = fixture["input"]["load_sid"].as_u64();
                NativeModelApi::from_model_binary(&base, load_sid)
                    .unwrap_or_else(|e| panic!("from_model_binary failed for {name}: {e}"))
            }
            other => panic!("unexpected workflow for {name}: {other}"),
        };

        api.apply_batch(&batch)
            .unwrap_or_else(|e| panic!("apply_batch failed for {name}: {e}"));

        assert_eq!(
            api.view(),
            fixture["expected"]["final_view_json"],
            "final view mismatch for {name}"
        );
        let out = api
            .to_model_binary()
            .unwrap_or_else(|e| panic!("to_model_binary failed for {name}: {e}"));
        assert_eq!(
            hex(&out),
            fixture["expected"]["final_model_binary_hex"]
                .as_str()
                .expect("expected.final_model_binary_hex must be string"),
            "final binary mismatch for {name}"
        );
    }

    assert!(
        seen >= 12,
        "expected at least 12 model_lifecycle_workflow fixtures"
    );
}
