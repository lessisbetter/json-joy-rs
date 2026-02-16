use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::util_diff::{line, str};
use serde_json::Value;

fn oracle_cwd() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
}

#[test]
fn differential_util_diff_seeded_matches_oracle() {
    let string_cases: Vec<(&str, &str, isize)> = vec![
        ("", "", -1),
        ("a", "a", -1),
        ("a", "ab", 2),
        ("ab", "a", 1),
        ("hello world", "hello, world", -1),
        ("kitten", "sitting", -1),
        ("abcdef", "abXYef", -1),
        ("aaabbb", "ab", -1),
        ("The quick brown fox", "The fast brown fox", -1),
        ("line1\nline2", "line1\nline2\nline3", -1),
        ("👨‍🍳chef", "👨‍🍳chief", -1),
        ("A👩‍💻B", "A👩‍💻BC", 5),
    ];

    for (idx, (src, dst, caret)) in string_cases.iter().enumerate() {
        let oracle = oracle_str_diff(src, dst, *caret);

        let rust_diff = str::diff(src, dst);
        let rust_diff_json = patch_to_json(&rust_diff);
        assert_eq!(
            rust_diff_json,
            oracle["diff"],
            "str.diff mismatch at case {idx}"
        );

        let rust_diff_edit = str::diff_edit(src, dst, *caret);
        let rust_diff_edit_json = patch_to_json(&rust_diff_edit);
        assert_eq!(
            rust_diff_edit_json,
            oracle["diffEdit"],
            "str.diffEdit mismatch at case {idx}"
        );
    }

    let line_cases: Vec<(Vec<&str>, Vec<&str>)> = vec![
        (vec![], vec![]),
        (vec!["a", "b", "c"], vec!["a", "b", "c"]),
        (vec!["a", "b", "c"], vec![]),
        (vec![], vec!["x", "y"]),
        (vec!["x", "y", "z"], vec!["y", "z", "x"]),
        (vec!["hello world", "same"], vec!["hello, world", "same"]),
        (vec!["a", "b"], vec!["x", "b", "c"]),
    ];

    for (idx, (src, dst)) in line_cases.iter().enumerate() {
        let src_owned: Vec<String> = src.iter().map(|s| (*s).to_string()).collect();
        let dst_owned: Vec<String> = dst.iter().map(|s| (*s).to_string()).collect();
        let oracle = oracle_line_diff(src, dst);
        let rust = line::diff(&src_owned, &dst_owned);
        let rust_json = line_patch_to_json(&rust);
        assert_eq!(rust_json, oracle, "line.diff mismatch at case {idx}");
    }
}

fn oracle_str_diff(src: &str, dst: &str, caret: isize) -> Value {
    let script = r#"
const str = require('json-joy/lib/util/diff/str');
const input = JSON.parse(process.argv[1]);
process.stdout.write(JSON.stringify({
  diff: str.diff(input.src, input.dst),
  diffEdit: str.diffEdit(input.src, input.dst, input.caret),
}));
"#;

    let payload = serde_json::json!({"src": src, "dst": dst, "caret": caret});
    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run str diff oracle");

    assert!(
        output.status.success(),
        "str diff oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle str diff must be json")
}

fn oracle_line_diff(src: &[&str], dst: &[&str]) -> Value {
    let script = r#"
const line = require('json-joy/lib/util/diff/line');
const input = JSON.parse(process.argv[1]);
process.stdout.write(JSON.stringify(line.diff(input.src, input.dst)));
"#;

    let payload = serde_json::json!({"src": src, "dst": dst});
    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run line diff oracle");

    assert!(
        output.status.success(),
        "line diff oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle line diff must be json")
}

fn patch_to_json(patch: &str::Patch) -> Value {
    Value::Array(
        patch
            .iter()
            .map(|(ty, txt)| Value::Array(vec![Value::from(*ty as i64), Value::String(txt.clone())]))
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
