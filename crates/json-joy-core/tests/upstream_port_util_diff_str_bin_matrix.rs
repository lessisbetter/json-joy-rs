use json_joy_core::util_diff::{bin, str};

#[test]
fn upstream_port_util_diff_str_prefix_suffix_overlap_matrix() {
    assert_eq!(str::pfx("abc", "ab"), 2);
    let chef_utf16 = "👨‍🍳".encode_utf16().count();
    assert_eq!(str::pfx("👨‍🍳chef", "👨‍🍳"), chef_utf16);
    assert_eq!(str::sfx("chef👨‍🍳", "👨‍🍳"), chef_utf16);
    assert_eq!(str::overlap("abcXXX", "XXXdef"), 3);
}

#[test]
fn upstream_port_util_diff_str_normalize_src_dst_invert_apply_matrix() {
    let patch = str::normalize(vec![
        (str::PatchOpType::Ins, "a".into()),
        (str::PatchOpType::Ins, "b".into()),
        (str::PatchOpType::Eql, "c".into()),
        (str::PatchOpType::Del, "".into()),
    ]);
    assert_eq!(
        patch,
        vec![
            (str::PatchOpType::Ins, "ab".into()),
            (str::PatchOpType::Eql, "c".into())
        ]
    );
    assert_eq!(str::src(&patch), "c");
    assert_eq!(str::dst(&patch), "abc");

    let inv = str::invert(&patch);
    assert_eq!(str::src(&inv), "abc");
    assert_eq!(str::dst(&inv), "c");

    let mut ins = Vec::new();
    let mut del = Vec::new();
    str::apply(
        &patch,
        str::src(&patch).len(),
        |pos, s| ins.push((pos, s.to_string())),
        |pos, len, s| del.push((pos, len, s.to_string())),
    );
    assert_eq!(ins, vec![(0usize, "ab".to_string())]);
    assert!(del.is_empty());
}

#[test]
fn upstream_port_util_diff_bin_roundtrip_matrix() {
    let src = vec![1u8, 2, 3];
    let dst = vec![0u8, 1, 2, 3, 4];
    let patch = bin::diff(&src, &dst);
    assert_eq!(bin::src(&patch), src);
    assert_eq!(bin::dst(&patch), dst);

    let mut ins = Vec::new();
    let mut del = Vec::new();
    bin::apply(
        &patch,
        3,
        |pos, data| ins.push((pos, data)),
        |pos, len| del.push((pos, len)),
    );
    assert!(!ins.is_empty() || !del.is_empty());
}
