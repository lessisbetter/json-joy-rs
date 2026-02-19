//! Line-level diff, built on top of the character-level string diff.
//!
//! Mirrors `packages/json-joy/src/util/diff/line.ts`.

use super::str::{self, normalize, Patch, PatchOpType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePatchOpType {
    /// The whole line is deleted.
    Del = -1,
    /// Lines are equal.
    Eql = 0,
    /// The whole line is inserted.
    Ins = 1,
    /// The line is modified (mixed insert + delete).
    Mix = 2,
}

/// A single line-level operation: (type, src_index, dst_index).
pub type LinePatchOp = (LinePatchOpType, i64, i64);
pub type LinePatch = Vec<LinePatchOp>;

/// Push to a patch line, merging consecutive same-type ops.
fn push_to_line(line: &mut Patch, op_type: PatchOpType, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = line.last_mut() {
        if last.0 == op_type {
            last.1.push_str(text);
            return;
        }
    }
    line.push((op_type, text.to_string()));
}

/// Aggregate a character-level patch into a per-line list of sub-patches.
///
/// Each element of the returned `Vec` is a character-level patch for one line.
pub fn agg(patch: &Patch) -> Vec<Patch> {
    let mut lines: Vec<Patch> = Vec::new();
    let mut line: Patch = Vec::new();

    for (op_type, text) in patch {
        let mut remaining = text.as_str();
        loop {
            match remaining.find('\n') {
                None => {
                    push_to_line(&mut line, *op_type, remaining);
                    break;
                }
                Some(idx) => {
                    // include the \n in this line's segment
                    push_to_line(&mut line, *op_type, &remaining[..idx + 1]);
                    if !line.is_empty() {
                        lines.push(std::mem::take(&mut line));
                    }
                    remaining = &remaining[idx + 1..];
                }
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }

    // Normalize each line
    for i in 0..lines.len() {
        lines[i] = normalize(std::mem::take(&mut lines[i]));
    }

    lines
}

/// Compute a line-level diff between `src` and `dst` string arrays.
pub fn diff(src: &[&str], dst: &[&str]) -> LinePatch {
    if dst.is_empty() {
        return src
            .iter()
            .enumerate()
            .map(|(i, _)| (LinePatchOpType::Del, i as i64, -1))
            .collect();
    }
    if src.is_empty() {
        return dst
            .iter()
            .enumerate()
            .map(|(i, _)| (LinePatchOpType::Ins, -1, i as i64))
            .collect();
    }

    let src_txt = src.join("\n") + "\n";
    let dst_txt = dst.join("\n") + "\n";
    if src_txt == dst_txt {
        return vec![];
    }

    let str_patch = str::diff(&src_txt, &dst_txt);
    let lines = agg(&str_patch);

    let mut patch: LinePatch = Vec::new();
    let mut src_idx: i64 = -1;
    let mut dst_idx: i64 = -1;
    let src_len = src.len() as i64;
    let dst_len = dst.len() as i64;

    for (i, line) in lines.iter().enumerate() {
        let mut line_work = line.clone();
        let line_len = line_work.len();

        if line_len == 0 {
            continue;
        }

        // Determine line type by inspecting the last op
        let last_op_type = line_work[line_len - 1].0;
        let last_txt = line_work[line_len - 1].1.clone();

        // Strip trailing \n from the last op
        if last_txt == "\n" {
            line_work.pop();
        } else if last_txt.ends_with('\n') {
            let trimmed = last_txt[..last_txt.len() - 1].to_string();
            if let Some(last) = line_work.last_mut() {
                last.1 = trimmed;
            }
        }

        let line_len2 = line_work.len();
        let line_type: LinePatchOpType;

        if line_len2 == 0 {
            match last_op_type {
                PatchOpType::Eql => {
                    line_type = LinePatchOpType::Eql;
                    src_idx += 1;
                    dst_idx += 1;
                }
                PatchOpType::Ins => {
                    line_type = LinePatchOpType::Ins;
                    dst_idx += 1;
                }
                PatchOpType::Del => {
                    line_type = LinePatchOpType::Del;
                    src_idx += 1;
                }
            }
        } else {
            let is_last = i + 1 == lines.len();
            if is_last {
                if src_idx + 1 < src_len {
                    if dst_idx + 1 < dst_len {
                        line_type = if line_len2 == 1 && line_work[0].0 == PatchOpType::Eql {
                            LinePatchOpType::Eql
                        } else {
                            LinePatchOpType::Mix
                        };
                        src_idx += 1;
                        dst_idx += 1;
                    } else {
                        line_type = LinePatchOpType::Del;
                        src_idx += 1;
                    }
                } else {
                    line_type = LinePatchOpType::Ins;
                    dst_idx += 1;
                }
            } else {
                let first_op = line_work[0].0;
                if line_len2 == 1 && first_op == last_op_type && first_op == PatchOpType::Eql {
                    line_type = LinePatchOpType::Eql;
                    src_idx += 1;
                    dst_idx += 1;
                } else {
                    match last_op_type {
                        PatchOpType::Eql => {
                            line_type = LinePatchOpType::Mix;
                            src_idx += 1;
                            dst_idx += 1;
                        }
                        PatchOpType::Ins => {
                            line_type = LinePatchOpType::Ins;
                            dst_idx += 1;
                        }
                        PatchOpType::Del => {
                            line_type = LinePatchOpType::Del;
                            src_idx += 1;
                        }
                    }
                }
            }
        }

        // Upgrade EQL to MIX if the actual lines differ
        let final_type = if line_type == LinePatchOpType::Eql {
            let si = src_idx as usize;
            let di = dst_idx as usize;
            if si < src.len() && di < dst.len() && src[si] != dst[di] {
                LinePatchOpType::Mix
            } else {
                LinePatchOpType::Eql
            }
        } else {
            line_type
        };

        patch.push((final_type, src_idx, dst_idx));
    }

    patch
}

/// Apply a line-level patch, invoking callbacks for each changed operation.
pub fn apply<FDel, FIns, FMix>(
    patch: &LinePatch,
    mut on_delete: FDel,
    mut on_insert: FIns,
    mut on_mix: FMix,
) where
    FDel: FnMut(usize),
    FIns: FnMut(i64, usize),
    FMix: FnMut(usize, usize),
{
    for i in (0..patch.len()).rev() {
        let (op_type, pos_src, pos_dst) = patch[i];
        match op_type {
            LinePatchOpType::Eql => {}
            LinePatchOpType::Del => on_delete(pos_src as usize),
            LinePatchOpType::Ins => on_insert(pos_src, pos_dst as usize),
            LinePatchOpType::Mix => on_mix(pos_src as usize, pos_dst as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_equal_lines() {
        let src = ["hello", "world"];
        let dst = ["hello", "world"];
        let patch = diff(&src, &dst);
        assert!(patch.is_empty());
    }

    #[test]
    fn diff_insert_line() {
        let src = ["hello"];
        let dst = ["hello", "world"];
        let patch = diff(&src, &dst);
        let has_ins = patch.iter().any(|(t, _, _)| *t == LinePatchOpType::Ins);
        assert!(has_ins);
    }

    #[test]
    fn diff_delete_line() {
        let src = ["hello", "world"];
        let dst = ["hello"];
        let patch = diff(&src, &dst);
        let has_del = patch.iter().any(|(t, _, _)| *t == LinePatchOpType::Del);
        assert!(has_del);
    }

    #[test]
    fn diff_empty_src() {
        let src: [&str; 0] = [];
        let dst = ["a", "b"];
        let patch = diff(&src, &dst);
        assert_eq!(patch.len(), 2);
        assert!(patch.iter().all(|(t, _, _)| *t == LinePatchOpType::Ins));
    }

    #[test]
    fn diff_empty_dst() {
        let src = ["a", "b"];
        let dst: [&str; 0] = [];
        let patch = diff(&src, &dst);
        assert_eq!(patch.len(), 2);
        assert!(patch.iter().all(|(t, _, _)| *t == LinePatchOpType::Del));
    }
}
