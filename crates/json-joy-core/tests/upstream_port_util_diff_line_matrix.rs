use json_joy_core::util_diff::line::{self, LinePatchOpType};

fn s(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|v| (*v).to_string()).collect()
}

#[test]
fn upstream_port_util_diff_line_delete_all_matrix() {
    let src = s(&["a", "b", "c", "d"]);
    let dst: Vec<String> = vec![];
    let patch = line::diff(&src, &dst);
    assert_eq!(
        patch,
        vec![
            (LinePatchOpType::Del, 0, -1),
            (LinePatchOpType::Del, 1, -1),
            (LinePatchOpType::Del, 2, -1),
            (LinePatchOpType::Del, 3, -1),
        ]
    );
}

#[test]
fn upstream_port_util_diff_line_move_first_to_end_matrix() {
    let src = s(&["x", "y", "z"]);
    let dst = s(&["y", "z", "x"]);
    let patch = line::diff(&src, &dst);
    assert_eq!(
        patch,
        vec![
            (LinePatchOpType::Del, 0, -1),
            (LinePatchOpType::Eql, 1, 0),
            (LinePatchOpType::Eql, 2, 1),
            (LinePatchOpType::Ins, 2, 2),
        ]
    );
}

#[test]
fn upstream_port_util_diff_line_mix_replace_matrix() {
    let src = s(&["hello world", "same"]);
    let dst = s(&["hello, world", "same"]);
    let patch = line::diff(&src, &dst);
    assert_eq!(
        patch,
        vec![
            (LinePatchOpType::Mix, 0, 0),
            (LinePatchOpType::Eql, 1, 1),
        ]
    );
}

#[test]
fn upstream_port_util_diff_line_apply_callbacks_matrix() {
    let src = s(&["a", "b"]);
    let dst = s(&["x", "b", "c"]);
    let patch = line::diff(&src, &dst);
    let mut dels = Vec::new();
    let mut ins = Vec::new();
    let mut mix = Vec::new();
    line::apply(
        &patch,
        |i| dels.push(i),
        |spos, dpos| ins.push((spos, dpos)),
        |spos, dpos| mix.push((spos, dpos)),
    );
    assert!(!dels.is_empty() || !ins.is_empty() || !mix.is_empty());
}

