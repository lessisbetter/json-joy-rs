use std::fs;
use std::path::Path;

use json_joy_core::util_diff::{bin, line, str as str_diff};
use serde_json::Value;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn read_fixture(path: &Path) -> Value {
    let data = fs::read_to_string(path).expect("fixture readable");
    serde_json::from_str(&data).expect("fixture json")
}

fn str_patch_to_json(patch: &str_diff::Patch) -> Value {
    Value::Array(
        patch
            .iter()
            .map(|(ty, txt)| {
                Value::Array(vec![Value::from(*ty as i64), Value::String(txt.clone())])
            })
            .collect(),
    )
}

fn line_patch_to_json(patch: &line::LinePatch) -> Value {
    Value::Array(
        patch
            .iter()
            .map(|(ty, src, dst)| {
                Value::Array(vec![
                    Value::from(*ty as i64),
                    Value::from(*src as i64),
                    Value::from(*dst as i64),
                ])
            })
            .collect(),
    )
}

#[test]
fn util_diff_fixtures_match_oracle_outputs() {
    let dir = fixtures_dir();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("manifest"))
            .expect("manifest json");

    let mut seen = 0u32;
    for entry in manifest["fixtures"].as_array().expect("fixtures array") {
        if entry["scenario"].as_str() != Some("util_diff_parity") {
            continue;
        }
        seen += 1;
        let fx = read_fixture(&dir.join(entry["file"].as_str().expect("fixture file")));
        let kind = fx["input"]["kind"].as_str().expect("input.kind");

        match kind {
            "str" => {
                let src = fx["input"]["src"].as_str().expect("input.src str");
                let dst = fx["input"]["dst"].as_str().expect("input.dst str");
                let patch = str_diff::diff(src, dst);
                assert_eq!(
                    str_patch_to_json(&patch),
                    fx["expected"]["patch"],
                    "str diff patch mismatch for {}",
                    fx["name"]
                );
                assert_eq!(
                    str_diff::src(&patch),
                    fx["expected"]["src_from_patch"]
                        .as_str()
                        .expect("expected.src_from_patch str"),
                    "str diff src reconstruction mismatch for {}",
                    fx["name"]
                );
                assert_eq!(
                    str_diff::dst(&patch),
                    fx["expected"]["dst_from_patch"]
                        .as_str()
                        .expect("expected.dst_from_patch str"),
                    "str diff dst reconstruction mismatch for {}",
                    fx["name"]
                );
            }
            "bin" => {
                let src: Vec<u8> = fx["input"]["src"]
                    .as_array()
                    .expect("input.src bin")
                    .iter()
                    .map(|v| u8::try_from(v.as_u64().expect("bin u64")).expect("bin u8"))
                    .collect();
                let dst: Vec<u8> = fx["input"]["dst"]
                    .as_array()
                    .expect("input.dst bin")
                    .iter()
                    .map(|v| u8::try_from(v.as_u64().expect("bin u64")).expect("bin u8"))
                    .collect();
                let patch = bin::diff(&src, &dst);
                assert_eq!(
                    str_patch_to_json(&patch),
                    fx["expected"]["patch"],
                    "bin diff patch mismatch for {}",
                    fx["name"]
                );
                let expected_src: Vec<u8> = fx["expected"]["src_from_patch"]
                    .as_array()
                    .expect("expected.src_from_patch bin")
                    .iter()
                    .map(|v| u8::try_from(v.as_u64().expect("bin u64")).expect("bin u8"))
                    .collect();
                let expected_dst: Vec<u8> = fx["expected"]["dst_from_patch"]
                    .as_array()
                    .expect("expected.dst_from_patch bin")
                    .iter()
                    .map(|v| u8::try_from(v.as_u64().expect("bin u64")).expect("bin u8"))
                    .collect();
                assert_eq!(
                    bin::src(&patch),
                    expected_src,
                    "bin src reconstruction mismatch for {}",
                    fx["name"]
                );
                assert_eq!(
                    bin::dst(&patch),
                    expected_dst,
                    "bin dst reconstruction mismatch for {}",
                    fx["name"]
                );
            }
            "line" => {
                let src: Vec<String> = fx["input"]["src"]
                    .as_array()
                    .expect("input.src line")
                    .iter()
                    .map(|v| v.as_str().expect("line src str").to_owned())
                    .collect();
                let dst: Vec<String> = fx["input"]["dst"]
                    .as_array()
                    .expect("input.dst line")
                    .iter()
                    .map(|v| v.as_str().expect("line dst str").to_owned())
                    .collect();
                let patch = line::diff(&src, &dst);
                assert_eq!(
                    line_patch_to_json(&patch),
                    fx["expected"]["patch"],
                    "line diff patch mismatch for {}",
                    fx["name"]
                );
            }
            other => panic!("unknown util_diff kind {other}"),
        }
    }

    assert!(seen >= 30, "expected at least 30 util_diff_parity fixtures");
}
