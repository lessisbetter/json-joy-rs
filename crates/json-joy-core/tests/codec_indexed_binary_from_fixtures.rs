use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use json_joy_core::codec_indexed_binary::{
    decode_fields_to_model_binary, encode_model_binary_to_fields,
};
use json_joy_core::model::Model;
use serde_json::Value;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn from_hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex length must be even");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn read_fixture(path: &Path) -> Value {
    let data = fs::read_to_string(path).expect("fixture readable");
    serde_json::from_str(&data).expect("fixture json")
}

#[test]
fn codec_indexed_binary_fixtures_match_oracle_fields_and_decode() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("codec_indexed_binary_parity") {
            continue;
        }
        let fx = read_fixture(&dir.join(entry["file"].as_str().expect("fixture file")));
        let input_bin = from_hex(fx["input"]["model_binary_hex"].as_str().expect("model hex"));

        let mut expected_fields = BTreeMap::new();
        for (k, v) in fx["expected"]["fields_hex"]
            .as_object()
            .expect("fields map")
        {
            expected_fields.insert(k.clone(), from_hex(v.as_str().expect("field hex")));
        }

        let actual_fields = encode_model_binary_to_fields(&input_bin).expect("indexed encode");
        assert_eq!(
            actual_fields, expected_fields,
            "fields mismatch for {}",
            fx["name"]
        );

        let decoded_bin = decode_fields_to_model_binary(&actual_fields).expect("indexed decode");
        let expected_bin = from_hex(
            fx["expected"]["model_binary_hex"]
                .as_str()
                .expect("expected model hex"),
        );
        assert_eq!(
            decoded_bin, expected_bin,
            "decoded model mismatch for {}",
            fx["name"]
        );

        let decoded_model = Model::from_binary(&decoded_bin).expect("decoded model parse");
        assert_eq!(
            decoded_model.view(),
            &fx["expected"]["view_json"],
            "decoded view mismatch for {}",
            fx["name"]
        );
    }
}
