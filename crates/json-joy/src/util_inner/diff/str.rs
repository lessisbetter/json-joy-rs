//! String diff algorithm — Myers O(ND) difference algorithm.
//!
//! Mirrors `packages/json-joy/src/util/diff/str.ts`.
//!
//! All length/position values are in **Unicode scalar values** (Rust `char`s),
//! not bytes. This differs from the TypeScript original which counts UTF-16
//! code units, but is the correct equivalent when operating on Rust strings.

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOpType {
    Del = -1,
    Eql = 0,
    Ins = 1,
}

pub type PatchOperation = (PatchOpType, String);
pub type Patch = Vec<PatchOperation>;

// ── Public utilities ──────────────────────────────────────────────────────

/// Merge consecutive operations of the same type; discard empty operations.
pub fn normalize(patch: Patch) -> Patch {
    let mut result: Patch = Vec::with_capacity(patch.len());
    for (op_type, text) in patch {
        if text.is_empty() {
            continue;
        }
        match result.last_mut() {
            Some(last) if last.0 == op_type => last.1.push_str(&text),
            _ => result.push((op_type, text)),
        }
    }
    result
}

/// Number of chars in the common prefix of `txt1` and `txt2`.
pub fn pfx(txt1: &str, txt2: &str) -> usize {
    pfx_chars(
        &txt1.chars().collect::<Vec<_>>(),
        &txt2.chars().collect::<Vec<_>>(),
    )
}

/// Number of chars in the common suffix of `txt1` and `txt2`.
pub fn sfx(txt1: &str, txt2: &str) -> usize {
    sfx_chars(
        &txt1.chars().collect::<Vec<_>>(),
        &txt2.chars().collect::<Vec<_>>(),
    )
}

/// Length of the longest suffix of `str1` that is a prefix of `str2` (in chars).
pub fn overlap(str1: &str, str2: &str) -> usize {
    let c1: Vec<char> = str1.chars().collect();
    let c2: Vec<char> = str2.chars().collect();
    overlap_chars(&c1, &c2)
}

/// Compute the diff between `src` and `dst` strings.
///
/// Returns a list of patch operations. EQL operations are included for
/// context. All positions and lengths are in Unicode scalar values (chars).
pub fn diff(src: &str, dst: &str) -> Patch {
    diff_internal(src, dst)
}

/// Fast-path diff when the caret position in `dst` is known.
///
/// If `caret < 0`, falls back to the full `diff` algorithm.
pub fn diff_edit(src: &str, dst: &str, caret: i64) -> Patch {
    if caret >= 0 {
        let caret = caret as usize;
        let src_chars: Vec<char> = src.chars().collect();
        let dst_chars: Vec<char> = dst.chars().collect();
        let src_len = src_chars.len();
        let dst_len = dst_chars.len();

        if src_len != dst_len {
            let dst_sfx = &dst_chars[caret..];
            let sfx_len = dst_sfx.len();
            if sfx_len <= src_len {
                let src_sfx = &src_chars[src_len - sfx_len..];
                if src_sfx == dst_sfx {
                    let is_insert = dst_len > src_len;
                    if is_insert {
                        let pfx_len = src_len - sfx_len;
                        let src_pfx = &src_chars[..pfx_len];
                        let dst_pfx = &dst_chars[..pfx_len];
                        if src_pfx == dst_pfx {
                            let insert: String = dst_chars[pfx_len..caret].iter().collect();
                            let mut patch: Patch = Vec::new();
                            if !src_pfx.is_empty() {
                                patch.push((PatchOpType::Eql, src_pfx.iter().collect()));
                            }
                            if !insert.is_empty() {
                                patch.push((PatchOpType::Ins, insert));
                            }
                            if !dst_sfx.is_empty() {
                                patch.push((PatchOpType::Eql, dst_sfx.iter().collect()));
                            }
                            return patch;
                        }
                    } else {
                        let pfx_len = dst_len - sfx_len;
                        let dst_pfx = &dst_chars[..pfx_len];
                        let src_pfx = &src_chars[..pfx_len];
                        if src_pfx == dst_pfx {
                            let del: String =
                                src_chars[pfx_len..src_len - sfx_len].iter().collect();
                            let mut patch: Patch = Vec::new();
                            if !src_pfx.is_empty() {
                                patch.push((PatchOpType::Eql, src_pfx.iter().collect()));
                            }
                            if !del.is_empty() {
                                patch.push((PatchOpType::Del, del));
                            }
                            if !dst_sfx.is_empty() {
                                patch.push((PatchOpType::Eql, dst_sfx.iter().collect()));
                            }
                            return patch;
                        }
                    }
                }
            }
        }
    }
    diff(src, dst)
}

/// Reconstruct the source string from a patch.
pub fn patch_src(patch: &Patch) -> String {
    let mut txt = String::new();
    for (op_type, str_val) in patch {
        if *op_type != PatchOpType::Ins {
            txt.push_str(str_val);
        }
    }
    txt
}

/// Reconstruct the destination string from a patch.
pub fn patch_dst(patch: &Patch) -> String {
    let mut txt = String::new();
    for (op_type, str_val) in patch {
        if *op_type != PatchOpType::Del {
            txt.push_str(str_val);
        }
    }
    txt
}

/// Invert a patch so it transforms dst → src instead of src → dst.
pub fn invert(patch: Patch) -> Patch {
    patch
        .into_iter()
        .map(|(op_type, txt)| {
            let inv = match op_type {
                PatchOpType::Eql => PatchOpType::Eql,
                PatchOpType::Ins => PatchOpType::Del,
                PatchOpType::Del => PatchOpType::Ins,
            };
            (inv, txt)
        })
        .collect()
}

/// Apply a patch, calling callbacks for insertions and deletions.
///
/// `src_len` is the length of the source string in chars.
/// Positions passed to callbacks are char positions in the source.
pub fn apply<FIns, FDel>(patch: &Patch, src_len: usize, mut on_insert: FIns, mut on_delete: FDel)
where
    FIns: FnMut(usize, &str),
    FDel: FnMut(usize, usize, &str),
{
    let mut pos = src_len;
    for i in (0..patch.len()).rev() {
        let (op_type, ref str_val) = patch[i];
        match op_type {
            PatchOpType::Eql => {
                pos -= str_val.chars().count();
            }
            PatchOpType::Ins => {
                on_insert(pos, str_val);
            }
            PatchOpType::Del => {
                let len = str_val.chars().count();
                pos -= len;
                on_delete(pos, len, str_val);
            }
        }
    }
}

// ── Internal helpers (char-slice based) ──────────────────────────────────

fn pfx_chars(c1: &[char], c2: &[char]) -> usize {
    if c1.is_empty() || c2.is_empty() || c1[0] != c2[0] {
        return 0;
    }
    let mut min = 0usize;
    let mut max = c1.len().min(c2.len());
    let mut mid = max;
    let mut start = 0;
    while min < mid {
        if c1[start..mid] == c2[start..mid] {
            min = mid;
            start = min;
        } else {
            max = mid;
        }
        mid = (max - min) / 2 + min;
    }
    mid
}

fn sfx_chars(c1: &[char], c2: &[char]) -> usize {
    let n1 = c1.len();
    let n2 = c2.len();
    if n1 == 0 || n2 == 0 || c1[n1 - 1] != c2[n2 - 1] {
        return 0;
    }
    let mut min = 0usize;
    let mut max = n1.min(n2);
    let mut mid = max;
    let mut end = 0;
    while min < mid {
        if c1[n1 - mid..n1 - end] == c2[n2 - mid..n2 - end] {
            min = mid;
            end = min;
        } else {
            max = mid;
        }
        mid = (max - min) / 2 + min;
    }
    mid
}

fn overlap_chars(c1: &[char], c2: &[char]) -> usize {
    let n1 = c1.len();
    let n2 = c2.len();
    if n1 == 0 || n2 == 0 {
        return 0;
    }

    let min_len = n1.min(n2);
    let c1_trim = if n1 > n2 { &c1[n1 - n2..] } else { c1 };
    let c2_trim = if n1 < n2 { &c2[..n1] } else { c2 };

    if c1_trim == c2_trim {
        return min_len;
    }

    let mut best = 0usize;
    let mut length = 1usize;
    loop {
        let pattern = &c1_trim[min_len - length..];
        match find_char_slice(c2_trim, pattern) {
            None => return best,
            Some(found) => {
                length += found;
                if found == 0 || c1_trim[min_len - length..] == c2_trim[..length] {
                    best = length;
                    length += 1;
                }
            }
        }
    }
}

/// Find the first occurrence of `needle` in `haystack`, returning the starting index.
fn find_char_slice(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect()
}

// ── Core diff algorithm ───────────────────────────────────────────────────

fn diff_internal(src: &str, dst: &str) -> Patch {
    if src == dst {
        return if src.is_empty() {
            vec![]
        } else {
            vec![(PatchOpType::Eql, src.to_string())]
        };
    }

    let c_src: Vec<char> = src.chars().collect();
    let c_dst: Vec<char> = dst.chars().collect();

    // Strip common prefix
    let prefix_len = pfx_chars(&c_src, &c_dst);
    let prefix = chars_to_string(&c_src[..prefix_len]);
    let c_src = &c_src[prefix_len..];
    let c_dst = &c_dst[prefix_len..];

    // Strip common suffix
    let suffix_len = sfx_chars(c_src, c_dst);
    let suffix = if suffix_len > 0 {
        chars_to_string(&c_src[c_src.len() - suffix_len..])
    } else {
        String::new()
    };
    let c_src = &c_src[..c_src.len() - suffix_len];
    let c_dst = &c_dst[..c_dst.len() - suffix_len];

    // Compute diff on the middle block
    let mut result = diff_no_common_affix(c_src, c_dst);
    if !prefix.is_empty() {
        result.insert(0, (PatchOpType::Eql, prefix));
    }
    if !suffix.is_empty() {
        result.push((PatchOpType::Eql, suffix));
    }

    cleanup_merge(&mut result);
    result
}

fn diff_no_common_affix(c1: &[char], c2: &[char]) -> Patch {
    if c1.is_empty() {
        return if c2.is_empty() {
            vec![]
        } else {
            vec![(PatchOpType::Ins, chars_to_string(c2))]
        };
    }
    if c2.is_empty() {
        return vec![(PatchOpType::Del, chars_to_string(c1))];
    }

    let n1 = c1.len();
    let n2 = c2.len();

    // Check if shorter is contained in longer
    let (long, short, long_is_src) = if n1 > n2 {
        (c1, c2, true)
    } else {
        (c2, c1, false)
    };
    if let Some(idx) = find_char_slice(long, short) {
        let short_str = chars_to_string(short);
        let start_str = chars_to_string(&long[..idx]);
        let end_str = chars_to_string(&long[idx + short.len()..]);
        return if long_is_src {
            let mut patch = vec![];
            if !start_str.is_empty() {
                patch.push((PatchOpType::Del, start_str));
            }
            if !short_str.is_empty() {
                patch.push((PatchOpType::Eql, short_str));
            }
            if !end_str.is_empty() {
                patch.push((PatchOpType::Del, end_str));
            }
            patch
        } else {
            let mut patch = vec![];
            if !start_str.is_empty() {
                patch.push((PatchOpType::Ins, start_str));
            }
            if !short_str.is_empty() {
                patch.push((PatchOpType::Eql, short_str));
            }
            if !end_str.is_empty() {
                patch.push((PatchOpType::Ins, end_str));
            }
            patch
        };
    }

    if short.len() == 1 {
        return vec![
            (PatchOpType::Del, chars_to_string(c1)),
            (PatchOpType::Ins, chars_to_string(c2)),
        ];
    }

    bisect(c1, c2)
}

fn bisect(c1: &[char], c2: &[char]) -> Patch {
    let n1 = c1.len();
    let n2 = c2.len();
    let max_d = (n1 + n2).div_ceil(2) + 1;
    let v_offset = max_d;
    let v_length = 2 * max_d;

    let mut v1: Vec<i64> = vec![-1; v_length];
    let mut v2: Vec<i64> = vec![-1; v_length];
    v1[v_offset + 1] = 0;
    v2[v_offset + 1] = 0;

    let delta = n1 as i64 - n2 as i64;
    let front = delta % 2 != 0;

    let mut k1start = 0i64;
    let mut k1end = 0i64;
    let mut k2start = 0i64;
    let mut k2end = 0i64;

    for d in 0..max_d as i64 {
        // Forward path
        let mut k1 = -d + k1start;
        while k1 <= d - k1end {
            let k1_offset = (v_offset as i64 + k1) as usize;
            let mut x1: i64 = if k1 == -d || (k1 != d && v1[k1_offset - 1] < v1[k1_offset + 1]) {
                v1[k1_offset + 1]
            } else {
                v1[k1_offset - 1] + 1
            };
            let mut y1 = x1 - k1;
            while x1 < n1 as i64 && y1 < n2 as i64 && c1[x1 as usize] == c2[y1 as usize] {
                x1 += 1;
                y1 += 1;
            }
            v1[k1_offset] = x1;
            if x1 > n1 as i64 {
                k1end += 2;
            } else if y1 > n2 as i64 {
                k1start += 2;
            } else if front {
                let k2_offset = (v_offset as i64 + delta - k1) as usize;
                if k2_offset < v_length && v2[k2_offset] != -1 && x1 >= n1 as i64 - v2[k2_offset] {
                    return bisect_split(c1, c2, x1 as usize, y1 as usize);
                }
            }
            k1 += 2;
        }

        // Reverse path
        let mut k2 = -d + k2start;
        while k2 <= d - k2end {
            let k2_offset = (v_offset as i64 + k2) as usize;
            let mut x2: i64 = if k2 == -d || (k2 != d && v2[k2_offset - 1] < v2[k2_offset + 1]) {
                v2[k2_offset + 1]
            } else {
                v2[k2_offset - 1] + 1
            };
            let mut y2 = x2 - k2;
            while x2 < n1 as i64
                && y2 < n2 as i64
                && c1[n1 - 1 - x2 as usize] == c2[n2 - 1 - y2 as usize]
            {
                x2 += 1;
                y2 += 1;
            }
            v2[k2_offset] = x2;
            if x2 > n1 as i64 {
                k2end += 2;
            } else if y2 > n2 as i64 {
                k2start += 2;
            } else if !front {
                let k1_offset = (v_offset as i64 + delta - k2) as usize;
                if k1_offset < v_length {
                    let x1 = v1[k1_offset];
                    if x1 != -1 {
                        let y1 = v_offset as i64 + x1 - k1_offset as i64;
                        let x2_real = n1 as i64 - x2;
                        if x1 >= x2_real {
                            return bisect_split(c1, c2, x1 as usize, y1 as usize);
                        }
                    }
                }
            }
            k2 += 2;
        }
    }

    // No split found — delete all of c1 and insert all of c2
    vec![
        (PatchOpType::Del, chars_to_string(c1)),
        (PatchOpType::Ins, chars_to_string(c2)),
    ]
}

fn bisect_split(c1: &[char], c2: &[char], x: usize, y: usize) -> Patch {
    let src_a: String = c1[..x].iter().collect();
    let dst_a: String = c2[..y].iter().collect();
    let src_b: String = c1[x..].iter().collect();
    let dst_b: String = c2[y..].iter().collect();
    let mut result = diff_internal(&src_a, &dst_a);
    result.extend(diff_internal(&src_b, &dst_b));
    result
}

// ── cleanup_merge ─────────────────────────────────────────────────────────

pub(crate) fn cleanup_merge(diff: &mut Patch) {
    diff.push((PatchOpType::Eql, String::new()));
    let mut pointer = 0usize;
    let mut del_cnt = 0usize;
    let mut ins_cnt = 0usize;
    let mut del_txt = String::new();
    let mut ins_txt = String::new();

    while pointer < diff.len() {
        // Remove empty ops that are not the last
        if pointer < diff.len() - 1 && diff[pointer].1.is_empty() {
            diff.remove(pointer);
            continue;
        }

        let op_type = diff[pointer].0;
        match op_type {
            PatchOpType::Ins => {
                ins_cnt += 1;
                let txt = diff[pointer].1.clone();
                ins_txt.push_str(&txt);
                pointer += 1;
            }
            PatchOpType::Del => {
                del_cnt += 1;
                let txt = diff[pointer].1.clone();
                del_txt.push_str(&txt);
                pointer += 1;
            }
            PatchOpType::Eql => {
                let prev_eq: Option<usize> = {
                    let p = pointer as i64 - ins_cnt as i64 - del_cnt as i64 - 1;
                    if p >= 0 {
                        Some(p as usize)
                    } else {
                        None
                    }
                };

                // Handle accumulated del/ins before this equality
                if !del_txt.is_empty() || !ins_txt.is_empty() {
                    let has_del = !del_txt.is_empty();
                    let has_ins = !ins_txt.is_empty();

                    if has_del && has_ins {
                        // Factor out common prefix
                        let del_chars: Vec<char> = del_txt.chars().collect();
                        let ins_chars: Vec<char> = ins_txt.chars().collect();
                        let common = pfx_chars(&ins_chars, &del_chars);
                        if common > 0 {
                            let prefix: String = ins_chars[..common].iter().collect();
                            if let Some(pq) = prev_eq {
                                diff[pq].1.push_str(&prefix);
                            } else {
                                diff.insert(0, (PatchOpType::Eql, prefix));
                                pointer += 1;
                            }
                            ins_txt = ins_chars[common..].iter().collect();
                            del_txt = del_chars[common..].iter().collect();
                        }

                        // Factor out common suffix
                        let del_chars: Vec<char> = del_txt.chars().collect();
                        let ins_chars: Vec<char> = ins_txt.chars().collect();
                        let common = sfx_chars(&ins_chars, &del_chars);
                        if common > 0 {
                            let ins_len = ins_chars.len();
                            let suffix: String = ins_chars[ins_len - common..].iter().collect();
                            let cur_txt = diff[pointer].1.clone();
                            diff[pointer].1 = suffix + &cur_txt;
                            ins_txt = ins_chars[..ins_len - common].iter().collect();
                            del_txt = del_chars[..del_chars.len() - common].iter().collect();
                        }
                    }

                    // Splice replacement
                    let n = ins_cnt + del_cnt;
                    let start = pointer - n;
                    let del_empty = del_txt.is_empty();
                    let ins_empty = ins_txt.is_empty();

                    if del_empty && ins_empty {
                        let _ = diff.splice(start..pointer, []);
                        pointer = start;
                    } else if del_empty {
                        let ins = ins_txt.clone();
                        let _ = diff.splice(start..pointer, [(PatchOpType::Ins, ins)]);
                        pointer = start + 1;
                    } else if ins_empty {
                        let del = del_txt.clone();
                        let _ = diff.splice(start..pointer, [(PatchOpType::Del, del)]);
                        pointer = start + 1;
                    } else {
                        let del = del_txt.clone();
                        let ins = ins_txt.clone();
                        let _ = diff.splice(
                            start..pointer,
                            [(PatchOpType::Del, del), (PatchOpType::Ins, ins)],
                        );
                        pointer = start + 2;
                    }
                }

                // Merge this equality with the previous one if it's also EQL
                if pointer != 0 && diff[pointer - 1].0 == PatchOpType::Eql {
                    let cur_txt = diff[pointer].1.clone();
                    diff[pointer - 1].1.push_str(&cur_txt);
                    diff.remove(pointer);
                    // pointer stays; don't increment
                } else {
                    pointer += 1;
                }

                ins_cnt = 0;
                del_cnt = 0;
                del_txt.clear();
                ins_txt.clear();
            }
        }
    }

    // Remove the dummy entry at the end
    if diff.last().map(|(_, s)| s.is_empty()) == Some(true) {
        diff.pop();
    }

    // Second pass: shift single edits sideways to eliminate equalities
    let mut changes = false;
    let mut pointer = 1usize;
    while pointer + 1 < diff.len() {
        let prev_type = diff[pointer - 1].0;
        let next_type = diff[pointer + 1].0;
        if prev_type == PatchOpType::Eql && next_type == PatchOpType::Eql {
            let prev_chars: Vec<char> = diff[pointer - 1].1.chars().collect();
            let cur_chars: Vec<char> = diff[pointer].1.chars().collect();
            let next_chars: Vec<char> = diff[pointer + 1].1.chars().collect();

            if cur_chars.len() >= prev_chars.len()
                && cur_chars[cur_chars.len() - prev_chars.len()..] == prev_chars[..]
            {
                // Shift edit over previous equality
                let new_cur: String = prev_chars
                    .iter()
                    .chain(cur_chars[..cur_chars.len() - prev_chars.len()].iter())
                    .collect();
                let new_next: String = prev_chars.iter().chain(next_chars.iter()).collect();
                diff[pointer].1 = new_cur;
                diff[pointer + 1].1 = new_next;
                diff.remove(pointer - 1);
                changes = true;
                // pointer is now one behind; don't increment so we re-check
            } else if cur_chars.len() >= next_chars.len()
                && cur_chars[..next_chars.len()] == next_chars[..]
            {
                // Shift edit over next equality
                let new_prev: String = prev_chars.iter().chain(next_chars.iter()).collect();
                let new_cur: String = cur_chars[next_chars.len()..]
                    .iter()
                    .chain(next_chars.iter())
                    .collect();
                diff[pointer - 1].1 = new_prev;
                diff[pointer].1 = new_cur;
                diff.remove(pointer + 1);
                changes = true;
                pointer += 1;
            } else {
                pointer += 1;
            }
        } else {
            pointer += 1;
        }
    }

    if changes {
        cleanup_merge(diff);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfx_empty() {
        assert_eq!(pfx("", "hello"), 0);
        assert_eq!(pfx("hello", ""), 0);
    }

    #[test]
    fn pfx_basic() {
        assert_eq!(pfx("hello", "helloworld"), 5);
        assert_eq!(pfx("abc", "abd"), 2);
        assert_eq!(pfx("abc", "xyz"), 0);
    }

    #[test]
    fn sfx_basic() {
        assert_eq!(sfx("hello", "world"), 0);
        assert_eq!(sfx("hello", "jello"), 4);
        assert_eq!(sfx("abc", "bc"), 2);
    }

    #[test]
    fn overlap_basic() {
        assert_eq!(overlap("abcxxx", "xxxdef"), 3);
        assert_eq!(overlap("abc", "abc"), 3);
        assert_eq!(overlap("abc", "xyz"), 0);
    }

    #[test]
    fn diff_equal_strings() {
        let p = diff("hello", "hello");
        assert_eq!(p, vec![(PatchOpType::Eql, "hello".to_string())]);
    }

    #[test]
    fn diff_empty_src() {
        let p = diff("", "hello");
        assert_eq!(p, vec![(PatchOpType::Ins, "hello".to_string())]);
    }

    #[test]
    fn diff_empty_dst() {
        let p = diff("hello", "");
        assert_eq!(p, vec![(PatchOpType::Del, "hello".to_string())]);
    }

    #[test]
    fn diff_simple_insert() {
        let p = diff("ac", "abc");
        let src = patch_src(&p);
        let dst = patch_dst(&p);
        assert_eq!(src, "ac");
        assert_eq!(dst, "abc");
    }

    #[test]
    fn diff_simple_delete() {
        let p = diff("abc", "ac");
        let src = patch_src(&p);
        let dst = patch_dst(&p);
        assert_eq!(src, "abc");
        assert_eq!(dst, "ac");
    }

    #[test]
    fn diff_roundtrip_src_dst() {
        let s = "the quick brown fox";
        let d = "the slow green fox";
        let p = diff(s, d);
        assert_eq!(patch_src(&p), s);
        assert_eq!(patch_dst(&p), d);
    }

    #[test]
    fn normalize_merges_consecutive() {
        let patch = vec![
            (PatchOpType::Ins, "hello".to_string()),
            (PatchOpType::Ins, " world".to_string()),
        ];
        let n = normalize(patch);
        assert_eq!(n, vec![(PatchOpType::Ins, "hello world".to_string())]);
    }

    #[test]
    fn normalize_drops_empty() {
        let patch = vec![
            (PatchOpType::Eql, "".to_string()),
            (PatchOpType::Ins, "hello".to_string()),
        ];
        let n = normalize(patch);
        assert_eq!(n, vec![(PatchOpType::Ins, "hello".to_string())]);
    }

    #[test]
    fn invert_patch() {
        let p = diff("abc", "aXc");
        let inv = invert(p);
        assert_eq!(patch_src(&inv), "aXc");
        assert_eq!(patch_dst(&inv), "abc");
    }

    #[test]
    fn diff_edit_insertion() {
        let p = diff_edit("ac", "abc", 2);
        let src = patch_src(&p);
        let dst = patch_dst(&p);
        assert_eq!(src, "ac");
        assert_eq!(dst, "abc");
    }
}
