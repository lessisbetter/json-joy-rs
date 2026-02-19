//! Irreversible binary operational transformation.
//!
//! Mirrors `packages/json-joy/src/json-ot/types/ot-binary-irreversible/`.
//!
//! Operates on `Vec<u8>` documents. Components are:
//! - `Retain(n)` — keep n bytes
//! - `Delete(n)` — skip n bytes
//! - `Insert(bytes)` — insert bytes

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryComponent {
    Retain(usize),
    Delete(usize),
    Insert(Vec<u8>),
}

pub type BinaryOp = Vec<BinaryComponent>;

impl BinaryComponent {
    pub fn src_len(&self) -> usize {
        match self {
            Self::Retain(n) => *n,
            Self::Delete(n) => *n,
            Self::Insert(_) => 0,
        }
    }

    pub fn dst_len(&self) -> usize {
        match self {
            Self::Retain(n) => *n,
            Self::Delete(_) => 0,
            Self::Insert(b) => b.len(),
        }
    }
}

/// Append a component, merging with the last if same type.
fn append(op: &mut BinaryOp, comp: BinaryComponent) {
    match (op.last_mut(), &comp) {
        (Some(BinaryComponent::Retain(n)), BinaryComponent::Retain(m)) => {
            *n += m;
            return;
        }
        (Some(BinaryComponent::Delete(n)), BinaryComponent::Delete(m)) => {
            *n += m;
            return;
        }
        (Some(BinaryComponent::Insert(s)), BinaryComponent::Insert(t)) => {
            s.extend_from_slice(t);
            return;
        }
        _ => {}
    }
    op.push(comp);
}

/// Remove trailing Retain components.
pub fn trim(op: &mut BinaryOp) {
    while matches!(op.last(), Some(BinaryComponent::Retain(_))) {
        op.pop();
    }
}

/// Normalize: coalesce adjacent same-type components and strip trailing retains.
pub fn normalize(op: BinaryOp) -> BinaryOp {
    let mut result: BinaryOp = Vec::new();
    for comp in op {
        match &comp {
            BinaryComponent::Retain(0) | BinaryComponent::Delete(0) => {}
            BinaryComponent::Insert(b) if b.is_empty() => {}
            _ => append(&mut result, comp),
        }
    }
    trim(&mut result);
    result
}

/// Apply a `BinaryOp` to a byte slice, returning the result.
pub fn apply(data: &[u8], op: &BinaryOp) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    let mut idx = 0usize;

    for comp in op {
        match comp {
            BinaryComponent::Retain(n) => {
                result.extend_from_slice(&data[idx..idx + n]);
                idx += n;
            }
            BinaryComponent::Delete(n) => {
                idx += n;
            }
            BinaryComponent::Insert(bytes) => {
                result.extend_from_slice(bytes);
            }
        }
    }
    result.extend_from_slice(&data[idx..]);
    result
}

/// Compose two sequential binary operations into one equivalent operation.
pub fn compose(op1: &BinaryOp, op2: &BinaryOp) -> BinaryOp {
    let mut result: BinaryOp = Vec::new();
    let mut iter1 = op1.iter().peekable();
    let mut iter2 = op2.iter().peekable();
    let mut rem1: Option<BinaryComponent> = None;
    let mut rem2: Option<BinaryComponent> = None;

    loop {
        let c1 = rem1.take().or_else(|| iter1.next().cloned());
        let c2 = rem2.take().or_else(|| iter2.next().cloned());

        match (c1, c2) {
            (None, None) => break,
            (Some(c), None) => {
                append(&mut result, c);
            }
            (None, Some(c)) => {
                append(&mut result, c);
            }
            (Some(c1), Some(c2)) => match (&c1, &c2) {
                (BinaryComponent::Delete(n), _) => {
                    append(&mut result, BinaryComponent::Delete(*n));
                    rem2 = Some(c2);
                }
                (_, BinaryComponent::Insert(b)) => {
                    append(&mut result, BinaryComponent::Insert(b.clone()));
                    rem1 = Some(c1);
                }
                (BinaryComponent::Retain(n), BinaryComponent::Retain(m)) => {
                    let min = (*n).min(*m);
                    append(&mut result, BinaryComponent::Retain(min));
                    if n > m {
                        rem1 = Some(BinaryComponent::Retain(n - m));
                    } else if m > n {
                        rem2 = Some(BinaryComponent::Retain(m - n));
                    }
                }
                (BinaryComponent::Retain(n), BinaryComponent::Delete(m)) => {
                    let min = (*n).min(*m);
                    append(&mut result, BinaryComponent::Delete(min));
                    if n > m {
                        rem1 = Some(BinaryComponent::Retain(n - m));
                    } else if m > n {
                        rem2 = Some(BinaryComponent::Delete(m - n));
                    }
                }
                (BinaryComponent::Insert(b), BinaryComponent::Retain(m)) => {
                    let b_len = b.len();
                    let kept = b[..(*m).min(b_len)].to_vec();
                    append(&mut result, BinaryComponent::Insert(kept));
                    if b_len > *m {
                        rem1 = Some(BinaryComponent::Insert(b[*m..].to_vec()));
                    } else if *m > b_len {
                        rem2 = Some(BinaryComponent::Retain(m - b_len));
                    }
                }
                (BinaryComponent::Insert(b), BinaryComponent::Delete(m)) => {
                    let b_len = b.len();
                    if b_len > *m {
                        rem1 = Some(BinaryComponent::Insert(b[*m..].to_vec()));
                    } else if *m > b_len {
                        rem2 = Some(BinaryComponent::Delete(m - b_len));
                    }
                }
            },
        }
    }
    normalize(result)
}

/// Transform `op` against `against`.
pub fn transform(op: &BinaryOp, against: &BinaryOp, left_wins: bool) -> BinaryOp {
    let mut result: BinaryOp = Vec::new();
    let mut op_iter = op.iter().cloned().peekable();
    let mut ag_iter = against.iter().cloned().peekable();
    let mut rem_op: Option<BinaryComponent> = None;
    let mut rem_ag: Option<BinaryComponent> = None;

    loop {
        let o = rem_op.take().or_else(|| op_iter.next());
        let a = rem_ag.take().or_else(|| ag_iter.next());

        match (o, a) {
            (None, _) => break,
            (Some(o), None) => {
                append(&mut result, o);
            }
            (Some(o), Some(a)) => match (&o, &a) {
                (_, BinaryComponent::Insert(b)) => {
                    let n = b.len();
                    if left_wins {
                        rem_op = Some(o);
                        append(&mut result, BinaryComponent::Retain(n));
                    } else {
                        append(&mut result, BinaryComponent::Retain(n));
                        rem_op = Some(o);
                    }
                }
                (BinaryComponent::Insert(b), _) => {
                    append(&mut result, BinaryComponent::Insert(b.clone()));
                    rem_ag = Some(a);
                }
                (BinaryComponent::Retain(n), BinaryComponent::Retain(m)) => {
                    let min = (*n).min(*m);
                    append(&mut result, BinaryComponent::Retain(min));
                    if n > m {
                        rem_op = Some(BinaryComponent::Retain(n - m));
                    } else if m > n {
                        rem_ag = Some(BinaryComponent::Retain(m - n));
                    }
                }
                (BinaryComponent::Retain(n), BinaryComponent::Delete(m)) => {
                    let min = (*n).min(*m);
                    if n > m {
                        rem_op = Some(BinaryComponent::Retain(n - m));
                    } else if m > n {
                        rem_ag = Some(BinaryComponent::Delete(m - n));
                    }
                }
                (BinaryComponent::Delete(n), BinaryComponent::Retain(m)) => {
                    let min = (*n).min(*m);
                    append(&mut result, BinaryComponent::Delete(min));
                    if n > m {
                        rem_op = Some(BinaryComponent::Delete(n - m));
                    } else if m > n {
                        rem_ag = Some(BinaryComponent::Retain(m - n));
                    }
                }
                (BinaryComponent::Delete(n), BinaryComponent::Delete(m)) => {
                    let min = (*n).min(*m);
                    if n > m {
                        rem_op = Some(BinaryComponent::Delete(n - m));
                    } else if m > n {
                        rem_ag = Some(BinaryComponent::Delete(m - n));
                    }
                }
            },
        }
    }
    normalize(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_insert_bytes() {
        let op = vec![BinaryComponent::Insert(vec![1, 2, 3])];
        assert_eq!(apply(&[], &op), vec![1, 2, 3]);
    }

    #[test]
    fn apply_delete_bytes() {
        let op = vec![BinaryComponent::Delete(2)];
        assert_eq!(apply(&[1, 2, 3], &op), vec![3]);
    }

    #[test]
    fn apply_retain_then_insert() {
        let op = vec![
            BinaryComponent::Retain(2),
            BinaryComponent::Insert(vec![99]),
        ];
        assert_eq!(apply(&[1, 2, 3], &op), vec![1, 2, 99, 3]);
    }

    #[test]
    fn compose_insert_then_delete() {
        let op1 = vec![BinaryComponent::Insert(vec![10])];
        let op2 = vec![BinaryComponent::Delete(1)];
        let composed = compose(&op1, &op2);
        assert!(composed.is_empty());
    }
}
