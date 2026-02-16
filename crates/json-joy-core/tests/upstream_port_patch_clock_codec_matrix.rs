use std::fs;
use std::path::Path;

use json_joy_core::crdt_binary::LogicalClockBase;
use json_joy_core::patch_clock_codec::{
    decode_clock_table, decode_relative_timestamp, decode_with_clock_table, encode_clock_table,
    encode_relative_timestamp,
};

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

#[test]
fn upstream_port_patch_clock_codec_matrix_matches_fixtures() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("patch_clock_codec_parity") {
            continue;
        }
        let fixture: Value = serde_json::from_str(
            &fs::read_to_string(dir.join(entry["file"].as_str().expect("fixture file")))
                .expect("fixture"),
        )
        .expect("fixture json");

        let expected_table_bytes = from_hex(
            fixture["expected"]["clock_table_binary_hex"]
                .as_str()
                .expect("clock table hex"),
        );
        let table = decode_clock_table(&expected_table_bytes).expect("clock table decode");

        let expected_table: Vec<LogicalClockBase> = fixture["expected"]["clock_table"]
            .as_array()
            .expect("clock table array")
            .iter()
            .map(|row| LogicalClockBase {
                sid: row[0].as_u64().expect("sid"),
                time: row[1].as_u64().expect("time"),
            })
            .collect();
        assert_eq!(
            table, expected_table,
            "table mismatch for {}",
            fixture["name"]
        );

        let encoded = encode_clock_table(&table);
        assert_eq!(
            encoded, expected_table_bytes,
            "clock table encode mismatch for {}",
            fixture["name"]
        );

        for rel in fixture["expected"]["relative_ids"]
            .as_array()
            .expect("relative ids")
        {
            let session_index = rel["session_index"].as_u64().expect("session index");
            let time_diff = rel["time_diff"].as_u64().expect("time diff");
            let encoded_rel = encode_relative_timestamp(session_index, time_diff);
            let decoded_rel = decode_relative_timestamp(&encoded_rel).expect("decode relative");
            assert_eq!(decoded_rel, (session_index, time_diff));

            let decoded_id =
                decode_with_clock_table(&table, session_index, time_diff).expect("decode id");
            let exp_sid = rel["decoded_id"][0].as_u64().expect("decoded sid");
            let exp_time = rel["decoded_id"][1].as_u64().expect("decoded time");
            assert_eq!(
                decoded_id.sid, exp_sid,
                "decoded sid mismatch for {}",
                fixture["name"]
            );
            assert_eq!(
                decoded_id.time, exp_time,
                "decoded time mismatch for {}",
                fixture["name"]
            );
        }
    }
}
