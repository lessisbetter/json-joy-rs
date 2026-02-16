use std::path::{Path, PathBuf};
use std::process::Command;

use json_joy_core::util_diff::{bin, line, str};
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
    let mut string_cases: Vec<(String, String, isize)> = vec![
        (String::new(), String::new(), -1),
        ("a".to_string(), "a".to_string(), -1),
        ("a".to_string(), "ab".to_string(), 2),
        ("ab".to_string(), "a".to_string(), 1),
        ("hello world".to_string(), "hello, world".to_string(), -1),
        ("kitten".to_string(), "sitting".to_string(), -1),
        ("abcdef".to_string(), "abXYef".to_string(), -1),
        ("aaabbb".to_string(), "ab".to_string(), -1),
        (
            "The quick brown fox".to_string(),
            "The fast brown fox".to_string(),
            -1,
        ),
        (
            "line1\nline2".to_string(),
            "line1\nline2\nline3".to_string(),
            -1,
        ),
        ("👨‍🍳chef".to_string(), "👨‍🍳chief".to_string(), -1),
        ("A👩‍💻B".to_string(), "A👩‍💻BC".to_string(), 5),
    ];

    let mut line_cases: Vec<(Vec<String>, Vec<String>)> = vec![
        (vec![], vec![]),
        (
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        ),
        (
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec![],
        ),
        (vec![], vec!["x".to_string(), "y".to_string()]),
        (
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
            vec!["y".to_string(), "z".to_string(), "x".to_string()],
        ),
        (
            vec!["hello world".to_string(), "same".to_string()],
            vec!["hello, world".to_string(), "same".to_string()],
        ),
        (
            vec!["a".to_string(), "b".to_string()],
            vec!["x".to_string(), "b".to_string(), "c".to_string()],
        ),
    ];

    let mut bin_cases: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (vec![], vec![]),
        (vec![1], vec![1, 2]),
        (vec![1, 2, 3], vec![1, 3]),
        (vec![0, 255, 42], vec![0, 255, 42, 99]),
        (vec![10, 20, 30, 40], vec![10, 30, 40]),
        (vec![1, 1, 2, 3], vec![1, 2, 2, 3]),
        (vec![5, 6, 7, 8], vec![8, 7, 6, 5]),
        (vec![9, 9, 9], vec![9, 9, 9]),
    ];

    let mut rng = Lcg::new(0x5151_7777_u64);
    while string_cases.len() < 40 {
        let src = random_string(&mut rng, 0, 24);
        let dst = random_string(&mut rng, 0, 24);
        let caret = if rng.range(2) == 0 {
            -1
        } else {
            rng.range((src.chars().count() + 1) as u64) as isize
        };
        string_cases.push((src, dst, caret));
    }
    while line_cases.len() < 40 {
        let src_len = rng.range(7) as usize;
        let dst_len = rng.range(7) as usize;
        let src = (0..src_len)
            .map(|_| random_string(&mut rng, 0, 10))
            .collect::<Vec<_>>();
        let dst = (0..dst_len)
            .map(|_| random_string(&mut rng, 0, 10))
            .collect::<Vec<_>>();
        line_cases.push((src, dst));
    }
    while bin_cases.len() < 40 {
        let src_len = rng.range(16) as usize;
        let dst_len = rng.range(16) as usize;
        let src = (0..src_len)
            .map(|_| rng.range(256) as u8)
            .collect::<Vec<_>>();
        let dst = (0..dst_len)
            .map(|_| rng.range(256) as u8)
            .collect::<Vec<_>>();
        bin_cases.push((src, dst));
    }

    for (idx, (src, dst, caret)) in string_cases.iter().enumerate() {
        let oracle = oracle_str_diff(src, dst, *caret);

        let rust_diff = str::diff(src, dst);
        let rust_diff_json = patch_to_json(&rust_diff);
        assert_eq!(
            rust_diff_json, oracle["diff"],
            "str.diff mismatch at case {idx}"
        );

        let rust_diff_edit = str::diff_edit(src, dst, *caret);
        let rust_diff_edit_json = patch_to_json(&rust_diff_edit);
        assert_eq!(
            rust_diff_edit_json, oracle["diffEdit"],
            "str.diffEdit mismatch at case {idx}"
        );
    }

    for (idx, (src, dst)) in line_cases.iter().enumerate() {
        let src_refs = src.iter().map(String::as_str).collect::<Vec<_>>();
        let dst_refs = dst.iter().map(String::as_str).collect::<Vec<_>>();
        let oracle = oracle_line_diff(&src_refs, &dst_refs);
        let rust = line::diff(src, dst);
        let rust_json = line_patch_to_json(&rust);
        assert_eq!(rust_json, oracle, "line.diff mismatch at case {idx}");
    }

    for (idx, (src, dst)) in bin_cases.iter().enumerate() {
        let oracle = oracle_bin_diff(src, dst);
        let rust_diff = bin::diff(src, dst);
        let rust_diff_json = patch_to_json(&rust_diff);
        let rust_src = Value::Array(bin::src(&rust_diff).into_iter().map(Value::from).collect());
        let rust_dst = Value::Array(bin::dst(&rust_diff).into_iter().map(Value::from).collect());
        assert_eq!(
            rust_diff_json, oracle["diff"],
            "bin.diff mismatch at case {idx}"
        );
        assert_eq!(rust_src, oracle["src"], "bin.src mismatch at case {idx}");
        assert_eq!(rust_dst, oracle["dst"], "bin.dst mismatch at case {idx}");
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

fn oracle_bin_diff(src: &[u8], dst: &[u8]) -> Value {
    let script = r#"
const bin = require('json-joy/lib/util/diff/bin');
const input = JSON.parse(process.argv[1]);
const patch = bin.diff(new Uint8Array(input.src), new Uint8Array(input.dst));
process.stdout.write(JSON.stringify({
  diff: patch,
  src: Array.from(bin.src(patch)),
  dst: Array.from(bin.dst(patch)),
}));
"#;

    let payload = serde_json::json!({"src": src, "dst": dst});
    let output = Command::new("node")
        .current_dir(oracle_cwd())
        .arg("-e")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .expect("failed to run bin diff oracle");

    assert!(
        output.status.success(),
        "bin diff oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("oracle bin diff must be json")
}

fn patch_to_json(patch: &str::Patch) -> Value {
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

fn random_string(rng: &mut Lcg, min: usize, max: usize) -> String {
    let span = max.saturating_sub(min);
    let len = min + rng.range((span + 1) as u64) as usize;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        let idx = rng.range(ALPHABET.len() as u64) as usize;
        out.push(ALPHABET[idx] as char);
    }
    out
}
