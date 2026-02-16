#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOpType {
    Del = -1,
    Eql = 0,
    Ins = 1,
}

pub type PatchOperation = (PatchOpType, String);
pub type Patch = Vec<PatchOperation>;

pub fn normalize(patch: Patch) -> Patch {
    let length = patch.len();
    if length < 2 {
        return patch;
    }
    let mut i = 0usize;
    let mut normalized_already = true;
    if patch[0].1.is_empty() {
        normalized_already = false;
    } else {
        i = 1;
        while i < length {
            let prev = &patch[i - 1];
            let curr = &patch[i];
            if curr.1.is_empty() || prev.0 == curr.0 {
                normalized_already = false;
                break;
            }
            i += 1;
        }
    }
    if normalized_already {
        return patch;
    }

    let mut normalized: Patch = Vec::with_capacity(length);
    for item in patch.iter().take(i) {
        normalized.push(item.clone());
    }
    for op in patch.into_iter().skip(i) {
        if op.1.is_empty() {
            continue;
        }
        if let Some(last) = normalized.last_mut() {
            if last.0 == op.0 {
                last.1.push_str(&op.1);
                continue;
            }
        }
        normalized.push(op);
    }
    normalized
}

fn starts_with_pair_end(s: &str) -> bool {
    let mut units = s.encode_utf16();
    if let Some(first) = units.next() {
        (0xdc00..=0xdfff).contains(&first)
    } else {
        false
    }
}

fn ends_with_pair_start(s: &str) -> bool {
    let mut last = None;
    for u in s.encode_utf16() {
        last = Some(u);
    }
    match last {
        Some(code) => (0xd800..=0xdbff).contains(&code),
        None => false,
    }
}

fn utf16_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn from_utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

fn cleanup_merge(diff: &mut Patch, fix_unicode: bool) {
    diff.push((PatchOpType::Eql, String::new()));
    let mut pointer = 0usize;
    let mut del_cnt = 0usize;
    let mut ins_cnt = 0usize;
    let mut del_txt = String::new();
    let mut ins_txt = String::new();

    while pointer < diff.len() {
        if pointer < diff.len() - 1 && diff[pointer].1.is_empty() {
            diff.remove(pointer);
            continue;
        }

        let (op_type, op_txt) = diff[pointer].clone();
        match op_type {
            PatchOpType::Ins => {
                ins_cnt += 1;
                pointer += 1;
                ins_txt.push_str(&op_txt);
            }
            PatchOpType::Del => {
                del_cnt += 1;
                pointer += 1;
                del_txt.push_str(&op_txt);
            }
            PatchOpType::Eql => {
                let mut prev_eq = pointer as isize - ins_cnt as isize - del_cnt as isize - 1;

                if fix_unicode {
                    if prev_eq >= 0 {
                        let idx = prev_eq as usize;
                        let prev_txt = diff[idx].1.clone();
                        if ends_with_pair_start(&prev_txt) {
                            let prev_units = utf16_units(&prev_txt);
                            if !prev_units.is_empty() {
                                let stray = from_utf16(&prev_units[prev_units.len() - 1..]);
                                let kept = from_utf16(&prev_units[..prev_units.len() - 1]);
                                diff[idx].1 = kept.clone();
                                del_txt = format!("{}{}", stray, del_txt);
                                ins_txt = format!("{}{}", stray, ins_txt);
                                if kept.is_empty() {
                                    diff.remove(idx);
                                    pointer = pointer.saturating_sub(1);
                                    let mut k = prev_eq - 1;
                                    if k >= 0 {
                                        let kidx = k as usize;
                                        if kidx < diff.len() {
                                            let (kt, ks) = diff[kidx].clone();
                                            match kt {
                                                PatchOpType::Ins => {
                                                    ins_cnt += 1;
                                                    k -= 1;
                                                    ins_txt = format!("{}{}", ks, ins_txt);
                                                }
                                                PatchOpType::Del => {
                                                    del_cnt += 1;
                                                    k -= 1;
                                                    del_txt = format!("{}{}", ks, del_txt);
                                                }
                                                PatchOpType::Eql => {}
                                            }
                                        }
                                    }
                                    prev_eq = k;
                                }
                            }
                        }
                    }

                    let cur_txt = diff[pointer].1.clone();
                    if starts_with_pair_end(&cur_txt) {
                        let units = utf16_units(&cur_txt);
                        if !units.is_empty() {
                            let stray = from_utf16(&units[..1]);
                            diff[pointer].1 = from_utf16(&units[1..]);
                            del_txt.push_str(&stray);
                            ins_txt.push_str(&stray);
                        }
                    }
                }

                if pointer < diff.len() - 1 && diff[pointer].1.is_empty() {
                    diff.remove(pointer);
                    continue;
                }

                let has_del = !del_txt.is_empty();
                let has_ins = !ins_txt.is_empty();
                if has_del || has_ins {
                    if has_del && has_ins {
                        let common_prefix = pfx(&ins_txt, &del_txt);
                        if common_prefix != 0 {
                            let ins_u = utf16_units(&ins_txt);
                            let del_u = utf16_units(&del_txt);
                            let pre = from_utf16(&ins_u[..common_prefix]);
                            if prev_eq >= 0 {
                                diff[prev_eq as usize].1.push_str(&pre);
                            } else {
                                diff.insert(0, (PatchOpType::Eql, pre));
                                pointer += 1;
                            }
                            ins_txt = from_utf16(&ins_u[common_prefix..]);
                            del_txt = from_utf16(&del_u[common_prefix..]);
                        }

                        let common_suffix = sfx(&ins_txt, &del_txt);
                        if common_suffix != 0 {
                            let ins_u = utf16_units(&ins_txt);
                            let del_u = utf16_units(&del_txt);
                            let ins_len = ins_u.len();
                            let del_len = del_u.len();
                            let suf = from_utf16(&ins_u[ins_len - common_suffix..]);
                            let cur = diff[pointer].1.clone();
                            diff[pointer].1 = format!("{}{}", suf, cur);
                            ins_txt = from_utf16(&ins_u[..ins_len - common_suffix]);
                            del_txt = from_utf16(&del_u[..del_len - common_suffix]);
                        }
                    }

                    let n = ins_cnt + del_cnt;
                    let start = pointer - n;
                    let del_len = del_txt.len();
                    let ins_len = ins_txt.len();
                    if del_len == 0 && ins_len == 0 {
                        diff.drain(start..start + n);
                        pointer = start;
                    } else if del_len == 0 {
                        diff.splice(start..start + n, [(PatchOpType::Ins, ins_txt.clone())]);
                        pointer = start + 1;
                    } else if ins_len == 0 {
                        diff.splice(start..start + n, [(PatchOpType::Del, del_txt.clone())]);
                        pointer = start + 1;
                    } else {
                        diff.splice(
                            start..start + n,
                            [
                                (PatchOpType::Del, del_txt.clone()),
                                (PatchOpType::Ins, ins_txt.clone()),
                            ],
                        );
                        pointer = start + 2;
                    }
                }

                if pointer != 0 && diff[pointer - 1].0 == PatchOpType::Eql {
                    let cur = diff[pointer].1.clone();
                    diff[pointer - 1].1.push_str(&cur);
                    diff.remove(pointer);
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

    if diff.last().is_some_and(|(_, s)| s.is_empty()) {
        diff.pop();
    }

    let mut changes = false;
    let mut ptr = 1usize;
    while ptr + 1 < diff.len() {
        if diff[ptr - 1].0 == PatchOpType::Eql && diff[ptr + 1].0 == PatchOpType::Eql {
            let str0 = diff[ptr - 1].1.clone();
            let str1 = diff[ptr].1.clone();
            let str2 = diff[ptr + 1].1.clone();
            if str1.ends_with(&str0) {
                let left = str1[..str1.len().saturating_sub(str0.len())].to_string();
                diff[ptr].1 = format!("{}{}", str0, left);
                diff[ptr + 1].1 = format!("{}{}", str0, str2);
                diff.remove(ptr - 1);
                changes = true;
            } else if str1.starts_with(&str2) {
                diff[ptr - 1].1.push_str(&str2);
                diff[ptr].1 = format!("{}{}", &str1[str2.len()..], str2);
                diff.remove(ptr + 1);
                changes = true;
            }
        }
        ptr += 1;
    }

    if changes {
        cleanup_merge(diff, fix_unicode);
    }
}

fn bisect_split(text1: &str, text2: &str, mut x: usize, mut y: usize) -> Patch {
    let t1: Vec<char> = text1.chars().collect();
    let t2: Vec<char> = text2.chars().collect();

    if x > 0 && x < t1.len() {
        let c = t1[x] as u32;
        if (0xdc00..=0xdfff).contains(&c) {
            x -= 1;
        }
    }
    if y > 0 && y < t2.len() {
        let c = t2[y] as u32;
        if (0xdc00..=0xdfff).contains(&c) {
            y -= 1;
        }
    }

    let left1: String = t1[..x].iter().collect();
    let left2: String = t2[..y].iter().collect();
    let right1: String = t1[x..].iter().collect();
    let right2: String = t2[y..].iter().collect();

    let mut diffs_a = diff_(&left1, &left2, false);
    let diffs_b = diff_(&right1, &right2, false);
    diffs_a.extend(diffs_b);
    diffs_a
}

fn bisect(text1: &str, text2: &str) -> Patch {
    let a: Vec<char> = text1.chars().collect();
    let b: Vec<char> = text2.chars().collect();
    let n = a.len();
    let m = b.len();

    let max_d = ((n + m) as f64 / 2.0).ceil() as isize;
    let v_offset = max_d;
    // JS implementation uses dynamic arrays and indexes up to `vOffset + 1`.
    // Reserve an extra slot to keep those accesses in-bounds in Rust vectors.
    let v_length = (2 * max_d + 2) as usize;
    let mut v1 = vec![-1isize; v_length];
    let mut v2 = vec![-1isize; v_length];
    v1[(v_offset + 1) as usize] = 0;
    v2[(v_offset + 1) as usize] = 0;

    let delta = n as isize - m as isize;
    let front = delta % 2 != 0;
    let mut k1start = 0isize;
    let mut k1end = 0isize;
    let mut k2start = 0isize;
    let mut k2end = 0isize;

    for d in 0..max_d {
        let mut k1 = -d + k1start;
        while k1 <= d - k1end {
            let k1_offset = v_offset + k1;
            let v10 = v1[(k1_offset - 1) as usize];
            let v11 = v1[(k1_offset + 1) as usize];
            let mut x1 = if k1 == -d || (k1 != d && v10 < v11) {
                v11
            } else {
                v10 + 1
            };
            let mut y1 = x1 - k1;
            while x1 < n as isize && y1 < m as isize && a[x1 as usize] == b[y1 as usize] {
                x1 += 1;
                y1 += 1;
            }
            v1[k1_offset as usize] = x1;
            if x1 > n as isize {
                k1end += 2;
            } else if y1 > m as isize {
                k1start += 2;
            } else if front {
                let k2_offset = v_offset + delta - k1;
                if k2_offset >= 0 && k2_offset < v_length as isize {
                    let v2o = v2[k2_offset as usize];
                    if v2o != -1 && x1 >= n as isize - v2o {
                        return bisect_split(text1, text2, x1 as usize, y1 as usize);
                    }
                }
            }
            k1 += 2;
        }

        let mut k2 = -d + k2start;
        while k2 <= d - k2end {
            let k2_offset = v_offset + k2;
            let mut x2 = if k2 == -d
                || (k2 != d && v2[(k2_offset - 1) as usize] < v2[(k2_offset + 1) as usize])
            {
                v2[(k2_offset + 1) as usize]
            } else {
                v2[(k2_offset - 1) as usize] + 1
            };
            let mut y2 = x2 - k2;
            while x2 < n as isize
                && y2 < m as isize
                && a[n - x2 as usize - 1] == b[m - y2 as usize - 1]
            {
                x2 += 1;
                y2 += 1;
            }
            v2[k2_offset as usize] = x2;
            if x2 > n as isize {
                k2end += 2;
            } else if y2 > m as isize {
                k2start += 2;
            } else if !front {
                let k1_offset = v_offset + delta - k2;
                if k1_offset >= 0 && k1_offset < v_length as isize {
                    let x1 = v1[k1_offset as usize];
                    if x1 != -1 {
                        let y1 = v_offset + x1 - k1_offset;
                        let x2f = n as isize - x2;
                        if x1 >= x2f {
                            return bisect_split(text1, text2, x1 as usize, y1 as usize);
                        }
                    }
                }
            }
            k2 += 2;
        }
    }

    vec![
        (PatchOpType::Del, text1.to_string()),
        (PatchOpType::Ins, text2.to_string()),
    ]
}

fn diff_no_common_affix(src: &str, dst: &str) -> Patch {
    if src.is_empty() {
        return vec![(PatchOpType::Ins, dst.to_string())];
    }
    if dst.is_empty() {
        return vec![(PatchOpType::Del, src.to_string())];
    }

    let src_len = src.chars().count();
    let dst_len = dst.chars().count();
    let (long, short, src_longer) = if src_len > dst_len {
        (src, dst, true)
    } else {
        (dst, src, false)
    };

    if let Some(index) = long.find(short) {
        let start = long[..index].to_string();
        let end = long[index + short.len()..].to_string();
        if src_longer {
            return vec![
                (PatchOpType::Del, start),
                (PatchOpType::Eql, short.to_string()),
                (PatchOpType::Del, end),
            ];
        }
        return vec![
            (PatchOpType::Ins, start),
            (PatchOpType::Eql, short.to_string()),
            (PatchOpType::Ins, end),
        ];
    }

    if short.chars().count() == 1 {
        return vec![
            (PatchOpType::Del, src.to_string()),
            (PatchOpType::Ins, dst.to_string()),
        ];
    }

    bisect(src, dst)
}

pub fn pfx(txt1: &str, txt2: &str) -> usize {
    if txt1.is_empty() || txt2.is_empty() {
        return 0;
    }
    let a = utf16_units(txt1);
    let b = utf16_units(txt2);
    if a.first() != b.first() {
        return 0;
    }

    let mut min = 0usize;
    let mut max = a.len().min(b.len());
    let mut mid = max;
    let mut start = 0usize;

    while min < mid {
        if a[start..mid] == b[start..mid] {
            min = mid;
            start = min;
        } else {
            max = mid;
        }
        mid = (max - min) / 2 + min;
    }

    if mid > 0 {
        let code = a[mid - 1];
        if (0xd800..=0xdbff).contains(&code) {
            mid -= 1;
        }
    }
    mid
}

pub fn sfx(txt1: &str, txt2: &str) -> usize {
    if txt1.is_empty() || txt2.is_empty() {
        return 0;
    }
    let a = utf16_units(txt1);
    let b = utf16_units(txt2);
    if a.last() != b.last() {
        return 0;
    }

    let mut min = 0usize;
    let mut max = a.len().min(b.len());
    let mut mid = max;
    let mut end = 0usize;

    while min < mid {
        if a[a.len() - mid..a.len() - end] == b[b.len() - mid..b.len() - end] {
            min = mid;
            end = min;
        } else {
            max = mid;
        }
        mid = (max - min) / 2 + min;
    }

    if mid > 0 && mid < a.len() {
        let boundary = a[a.len() - mid - 1];
        let is_high = (0xd800..=0xdbff).contains(&boundary);
        let is_combining = boundary == 0x200d
            || (0xfe00..=0xfe0f).contains(&boundary)
            || (0x0300..=0x036f).contains(&boundary);
        if is_high || is_combining {
            mid -= 1;
            while mid > 0 {
                let pos = a.len().saturating_sub(mid + 1);
                if pos >= a.len() {
                    break;
                }
                let prev = a[pos];
                let prev_high = (0xd800..=0xdbff).contains(&prev);
                let prev_comb = prev == 0x200d
                    || (0xfe00..=0xfe0f).contains(&prev)
                    || (0x0300..=0x036f).contains(&prev);
                if !prev_high && !prev_comb {
                    break;
                }
                mid -= 1;
            }
        }
    }

    mid
}

pub fn overlap(mut str1: &str, mut str2: &str) -> usize {
    let str1_len = utf16_units(str1).len();
    let str2_len = utf16_units(str2).len();
    if str1_len == 0 || str2_len == 0 {
        return 0;
    }

    let mut min_len = str1_len;
    if str1_len > str2_len {
        min_len = str2_len;
        let u = utf16_units(str1);
        str1 = Box::leak(from_utf16(&u[str1_len - str2_len..]).into_boxed_str());
    } else if str1_len < str2_len {
        let u = utf16_units(str2);
        str2 = Box::leak(from_utf16(&u[..str1_len]).into_boxed_str());
    }

    if str1 == str2 {
        return min_len;
    }

    let mut best = 0usize;
    let mut length = 1usize;
    loop {
        let u1 = utf16_units(str1);
        let pattern = from_utf16(&u1[min_len - length..]);
        if let Some(found_byte) = str2.find(&pattern) {
            let before = &str2[..found_byte];
            let found = utf16_units(before).len();
            length += found;
            let u1 = utf16_units(str1);
            let u2 = utf16_units(str2);
            if found == 0 || u1[min_len - length..] == u2[..length] {
                best = length;
                length += 1;
            }
        } else {
            return best;
        }
    }
}

fn diff_(mut src: &str, mut dst: &str, fix_unicode: bool) -> Patch {
    if src == dst {
        if src.is_empty() {
            return vec![];
        }
        return vec![(PatchOpType::Eql, src.to_string())];
    }

    let prefix_len = pfx(src, dst);
    let src_u = utf16_units(src);
    let dst_u = utf16_units(dst);
    let prefix = from_utf16(&src_u[..prefix_len]);
    let src_mid = from_utf16(&src_u[prefix_len..]);
    let dst_mid = from_utf16(&dst_u[prefix_len..]);
    let src_mid_u = utf16_units(&src_mid);
    let dst_mid_u = utf16_units(&dst_mid);

    let suffix_len = sfx(&src_mid, &dst_mid);
    let suffix = from_utf16(&src_mid_u[src_mid_u.len().saturating_sub(suffix_len)..]);
    src = Box::leak(
        from_utf16(&src_mid_u[..src_mid_u.len().saturating_sub(suffix_len)]).into_boxed_str(),
    );
    dst = Box::leak(
        from_utf16(&dst_mid_u[..dst_mid_u.len().saturating_sub(suffix_len)]).into_boxed_str(),
    );

    let mut d = diff_no_common_affix(src, dst);
    if !prefix.is_empty() {
        d.insert(0, (PatchOpType::Eql, prefix));
    }
    if !suffix.is_empty() {
        d.push((PatchOpType::Eql, suffix));
    }
    cleanup_merge(&mut d, fix_unicode);
    d
}

pub fn diff(src: &str, dst: &str) -> Patch {
    diff_(src, dst, true)
}

pub fn diff_edit(src: &str, dst: &str, caret: isize) -> Patch {
    if caret >= 0 {
        let src_len = utf16_units(src).len();
        let dst_len = utf16_units(dst).len();
        if src_len != dst_len {
            let caret_u = caret as usize;
            let dst_u = utf16_units(dst);
            if caret_u <= dst_u.len() {
                let dst_sfx = from_utf16(&dst_u[caret_u..]);
                let sfx_len = utf16_units(&dst_sfx).len();
                if sfx_len <= src_len {
                    let src_u = utf16_units(src);
                    let src_sfx = from_utf16(&src_u[src_len - sfx_len..]);
                    if src_sfx == dst_sfx {
                        if dst_len > src_len {
                            let pfx_len = src_len - sfx_len;
                            let src_pfx = from_utf16(&src_u[..pfx_len]);
                            let dst_pfx = from_utf16(&dst_u[..pfx_len]);
                            if src_pfx == dst_pfx {
                                let insert = from_utf16(&dst_u[pfx_len..caret_u]);
                                let mut patch = Vec::new();
                                if !src_pfx.is_empty() {
                                    patch.push((PatchOpType::Eql, src_pfx));
                                }
                                if !insert.is_empty() {
                                    patch.push((PatchOpType::Ins, insert));
                                }
                                if !dst_sfx.is_empty() {
                                    patch.push((PatchOpType::Eql, dst_sfx));
                                }
                                return patch;
                            }
                        } else {
                            let pfx_len = dst_len - sfx_len;
                            let dst_pfx = from_utf16(&dst_u[..pfx_len]);
                            let src_pfx = from_utf16(&src_u[..pfx_len]);
                            if src_pfx == dst_pfx {
                                let del = from_utf16(&src_u[pfx_len..src_len - sfx_len]);
                                let mut patch = Vec::new();
                                if !src_pfx.is_empty() {
                                    patch.push((PatchOpType::Eql, src_pfx));
                                }
                                if !del.is_empty() {
                                    patch.push((PatchOpType::Del, del));
                                }
                                if !dst_sfx.is_empty() {
                                    patch.push((PatchOpType::Eql, dst_sfx));
                                }
                                return patch;
                            }
                        }
                    }
                }
            }
        }
    }
    diff(src, dst)
}

pub fn src(patch: &Patch) -> String {
    let mut txt = String::new();
    for (t, s) in patch {
        if *t != PatchOpType::Ins {
            txt.push_str(s);
        }
    }
    txt
}

pub fn dst(patch: &Patch) -> String {
    let mut txt = String::new();
    for (t, s) in patch {
        if *t != PatchOpType::Del {
            txt.push_str(s);
        }
    }
    txt
}

pub fn invert(patch: &Patch) -> Patch {
    patch
        .iter()
        .map(|(t, s)| match t {
            PatchOpType::Eql => (PatchOpType::Eql, s.clone()),
            PatchOpType::Ins => (PatchOpType::Del, s.clone()),
            PatchOpType::Del => (PatchOpType::Ins, s.clone()),
        })
        .collect()
}

pub fn apply<FIns, FDel>(patch: &Patch, src_len: usize, mut on_insert: FIns, mut on_delete: FDel)
where
    FIns: FnMut(usize, &str),
    FDel: FnMut(usize, usize, &str),
{
    let mut pos = src_len;
    for (t, s) in patch.iter().rev() {
        match t {
            PatchOpType::Eql => pos = pos.saturating_sub(utf16_units(s).len()),
            PatchOpType::Ins => on_insert(pos, s),
            PatchOpType::Del => {
                let len = utf16_units(s).len();
                pos = pos.saturating_sub(len);
                on_delete(pos, len, s);
            }
        }
    }
}
