use super::str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePatchOpType {
    Del = -1,
    Eql = 0,
    Ins = 1,
    Mix = 2,
}

pub type LinePatchOp = (LinePatchOpType, isize, isize);
pub type LinePatch = Vec<LinePatchOp>;

pub fn agg(patch: str::Patch) -> Vec<str::Patch> {
    let mut lines: Vec<str::Patch> = Vec::new();
    let mut line: str::Patch = Vec::new();

    let push = |ty: str::PatchOpType, s: &str, line: &mut str::Patch| {
        if s.is_empty() {
            return;
        }
        if let Some(last) = line.last_mut() {
            if last.0 == ty {
                last.1.push_str(s);
                return;
            }
        }
        line.push((ty, s.to_string()));
    };

    for (ty, s) in patch {
        if let Some(index) = s.find('\n') {
            push(ty, &s[..index + 1], &mut line);
            if !line.is_empty() {
                lines.push(line);
            }
            line = Vec::new();

            let mut prev = index;
            while prev < s.len() {
                if let Some(next) = s[prev + 1..].find('\n') {
                    let next_abs = prev + 1 + next;
                    lines.push(vec![(ty, s[prev + 1..next_abs + 1].to_string())]);
                    prev = next_abs;
                } else {
                    push(ty, &s[prev + 1..], &mut line);
                    break;
                }
            }
        } else {
            push(ty, &s, &mut line);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }

    let len = lines.len();
    for i in 0..len {
        lines[i] = str::normalize(lines[i].clone());

        'normalize_line_start: {
            let line = &lines[i];
            if line.len() < 2 {
                break 'normalize_line_start;
            }
            let first = &line[0];
            let second = &line[1];
            let second_ty = second.0;
            if first.0 != str::PatchOpType::Eql {
                break 'normalize_line_start;
            }
            if second_ty != str::PatchOpType::Del && second_ty != str::PatchOpType::Ins {
                break 'normalize_line_start;
            }
            for op in line.iter().skip(2) {
                if op.0 != second_ty {
                    break 'normalize_line_start;
                }
            }

            let pfx = first.1.clone();
            for j in i + 1..len {
                lines[j] = str::normalize(lines[j].clone());
                let target = &lines[j];
                if target.len() > 1
                    && target[0].0 == second_ty
                    && target[1].0 == str::PatchOpType::Eql
                    && target[0].1 == pfx
                {
                    let mut li = lines[i].clone();
                    let mut lj = lines[j].clone();
                    li.remove(0);
                    li[0].1 = format!("{}{}", pfx, li[0].1);
                    lj[1].1 = format!("{}{}", pfx, lj[1].1);
                    lj.remove(0);
                    lines[i] = li;
                    lines[j] = lj;
                    break 'normalize_line_start;
                }

                for op in target {
                    if op.0 != second_ty {
                        break 'normalize_line_start;
                    }
                }
            }
        }

        'normalize_line_end: {
            let line = &lines[i];
            if line.len() < 2 {
                break 'normalize_line_end;
            }
            let last = line.last().cloned().unwrap();
            if last.0 != str::PatchOpType::Del {
                break 'normalize_line_end;
            }
            let last_str = last.1;

            'next_line: for j in i + 1..len {
                lines[j] = str::normalize(lines[j].clone());
                let target = &lines[j];
                if target.is_empty() {
                    continue 'next_line;
                }

                let target_last = if target.len() == 1 {
                    if target[0].0 == str::PatchOpType::Del {
                        continue 'next_line;
                    }
                    if target[0].0 != str::PatchOpType::Eql {
                        break 'normalize_line_end;
                    }
                    target[0].clone()
                } else {
                    if target.len() > 2 {
                        break 'normalize_line_end;
                    }
                    if target[0].0 != str::PatchOpType::Del {
                        break 'normalize_line_end;
                    }
                    target[1].clone()
                };

                if target_last.0 == str::PatchOpType::Del {
                    continue 'next_line;
                }
                if target_last.0 != str::PatchOpType::Eql {
                    break 'normalize_line_end;
                }

                let move_str = target_last.1;
                if move_str.len() > last_str.len() || !last_str.ends_with(&move_str) {
                    break 'normalize_line_end;
                }

                let index = last_str.len() - move_str.len();
                let mut li = lines[i].clone();
                let mut lj = lines[j].clone();
                let li_last = li.last_mut().expect("line has last");
                li_last.1 = last_str[..index].to_string();
                li.push((str::PatchOpType::Eql, move_str.clone()));
                if lj.len() == 1 {
                    lj[0].0 = str::PatchOpType::Del;
                } else {
                    lj[1].0 = str::PatchOpType::Del;
                }
                lines[i] = str::normalize(li);
                lines[j] = str::normalize(lj);
                break 'normalize_line_end;
            }
        }
    }

    lines
}

pub fn diff(src: &[String], dst: &[String]) -> LinePatch {
    if dst.is_empty() {
        return src
            .iter()
            .enumerate()
            .map(|(i, _)| (LinePatchOpType::Del, i as isize, -1))
            .collect();
    }
    if src.is_empty() {
        return dst
            .iter()
            .enumerate()
            .map(|(i, _)| (LinePatchOpType::Ins, -1, i as isize))
            .collect();
    }

    let src_txt = format!("{}\n", src.join("\n"));
    let dst_txt = format!("{}\n", dst.join("\n"));
    if src_txt == dst_txt {
        return vec![];
    }

    let str_patch = str::diff(&src_txt, &dst_txt);
    let mut lines = agg(str_patch);
    let mut patch: LinePatch = Vec::new();
    let mut src_idx: isize = -1;
    let mut dst_idx: isize = -1;
    let src_len = src.len() as isize;
    let dst_len = dst.len() as isize;

    let lines_total = lines.len();
    for (i, line) in lines.iter_mut().enumerate().take(lines_total) {
        if line.is_empty() {
            continue;
        }

        let last_idx = line.len() - 1;
        let last_ty = line[last_idx].0;
        let last_txt = line[last_idx].1.clone();
        if last_txt == "\n" {
            line.remove(last_idx);
        } else if last_txt.ends_with('\n') {
            if last_txt.len() == 1 {
                line.remove(last_idx);
            } else {
                line[last_idx].1 = last_txt[..last_txt.len() - 1].to_string();
            }
        }

        let mut line_ty = LinePatchOpType::Eql;
        if line.is_empty() {
            match last_ty {
                str::PatchOpType::Eql => {
                    line_ty = LinePatchOpType::Eql;
                    src_idx += 1;
                    dst_idx += 1;
                }
                str::PatchOpType::Ins => {
                    line_ty = LinePatchOpType::Ins;
                    dst_idx += 1;
                }
                str::PatchOpType::Del => {
                    line_ty = LinePatchOpType::Del;
                    src_idx += 1;
                }
            }
        } else if i + 1 == lines_total {
            if src_idx + 1 < src_len {
                if dst_idx + 1 < dst_len {
                    line_ty = if line.len() == 1 && line[0].0 == str::PatchOpType::Eql {
                        LinePatchOpType::Eql
                    } else {
                        LinePatchOpType::Mix
                    };
                    src_idx += 1;
                    dst_idx += 1;
                } else {
                    line_ty = LinePatchOpType::Del;
                    src_idx += 1;
                }
            } else {
                line_ty = LinePatchOpType::Ins;
                dst_idx += 1;
            }
        } else {
            let first_ty = line[0].0;
            if line.len() == 1 && first_ty == last_ty && first_ty == str::PatchOpType::Eql {
                src_idx += 1;
                dst_idx += 1;
            } else if last_ty == str::PatchOpType::Eql {
                line_ty = LinePatchOpType::Mix;
                src_idx += 1;
                dst_idx += 1;
            } else if last_ty == str::PatchOpType::Ins {
                line_ty = LinePatchOpType::Ins;
                dst_idx += 1;
            } else if last_ty == str::PatchOpType::Del {
                line_ty = LinePatchOpType::Del;
                src_idx += 1;
            }
        }

        if line_ty == LinePatchOpType::Eql
            && src_idx >= 0
            && dst_idx >= 0
            && src[src_idx as usize] != dst[dst_idx as usize]
        {
            line_ty = LinePatchOpType::Mix;
        }

        patch.push((line_ty, src_idx, dst_idx));
    }

    patch
}

pub fn apply<FDel, FIns, FMix>(
    patch: &LinePatch,
    mut on_delete: FDel,
    mut on_insert: FIns,
    mut on_mix: FMix,
) where
    FDel: FnMut(usize),
    FIns: FnMut(isize, usize),
    FMix: FnMut(usize, usize),
{
    for (ty, pos_src, pos_dst) in patch.iter().rev() {
        match ty {
            LinePatchOpType::Eql => {}
            LinePatchOpType::Del => on_delete(*pos_src as usize),
            LinePatchOpType::Ins => on_insert(*pos_src, *pos_dst as usize),
            LinePatchOpType::Mix => on_mix(*pos_src as usize, *pos_dst as usize),
        }
    }
}
