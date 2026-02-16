#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePatchOpType {
    Del = -1,
    Eql = 0,
    Ins = 1,
    Mix = 2,
}

pub type LinePatchOp = (LinePatchOpType, isize, isize);
pub type LinePatch = Vec<LinePatchOp>;

/// Baseline line diff: LCS for equal lines + DEL/INS folding into MIX when
/// lines are replaced one-for-one.
pub fn diff(src: &[String], dst: &[String]) -> LinePatch {
    if dst.is_empty() {
        return (0..src.len())
            .map(|i| (LinePatchOpType::Del, i as isize, -1))
            .collect();
    }
    if src.is_empty() {
        return (0..dst.len())
            .map(|i| (LinePatchOpType::Ins, -1, i as isize))
            .collect();
    }

    let n = src.len();
    let m = dst.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if src[i] == dst[j] {
                1 + dp[i + 1][j + 1]
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if src[i] == dst[j] {
            out.push((LinePatchOpType::Eql, i as isize, j as isize));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push((LinePatchOpType::Del, i as isize, j as isize - 1));
            i += 1;
        } else {
            out.push((LinePatchOpType::Ins, i as isize - 1, j as isize));
            j += 1;
        }
    }
    while i < n {
        out.push((LinePatchOpType::Del, i as isize, j as isize - 1));
        i += 1;
    }
    while j < m {
        out.push((LinePatchOpType::Ins, i as isize - 1, j as isize));
        j += 1;
    }

    // Convert adjacent DEL+INS (or INS+DEL) into MIX (replacement).
    let mut folded = Vec::with_capacity(out.len());
    let mut k = 0usize;
    while k < out.len() {
        if k + 1 < out.len() {
            let a = out[k];
            let b = out[k + 1];
            if a.0 == LinePatchOpType::Del && b.0 == LinePatchOpType::Ins {
                folded.push((LinePatchOpType::Mix, a.1, b.2));
                k += 2;
                continue;
            }
            if a.0 == LinePatchOpType::Ins && b.0 == LinePatchOpType::Del {
                folded.push((LinePatchOpType::Mix, b.1, a.2));
                k += 2;
                continue;
            }
        }
        folded.push(out[k]);
        k += 1;
    }
    folded
}

pub fn apply<FDel, FIns, FMix>(patch: &LinePatch, mut on_delete: FDel, mut on_insert: FIns, mut on_mix: FMix)
where
    FDel: FnMut(usize),
    FIns: FnMut(isize, usize),
    FMix: FnMut(usize, usize),
{
    for (op, src_pos, dst_pos) in patch.iter().rev() {
        match op {
            LinePatchOpType::Eql => {}
            LinePatchOpType::Del => on_delete(*src_pos as usize),
            LinePatchOpType::Ins => on_insert(*src_pos, *dst_pos as usize),
            LinePatchOpType::Mix => on_mix(*src_pos as usize, *dst_pos as usize),
        }
    }
}

