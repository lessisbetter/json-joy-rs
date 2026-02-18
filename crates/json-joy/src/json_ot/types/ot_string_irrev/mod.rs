//! Irreversible string operational transformation.
//!
//! Mirrors `packages/json-joy/src/json-ot/types/ot-string-irreversible/`.
//!
//! Like `ot_string` but does not store the content of deleted text —
//! deletions are represented only by their count.

#[derive(Debug, Clone, PartialEq)]
pub enum StringIrrevComponent {
    Retain(usize),
    Delete(usize),
    Insert(String),
}

pub type StringIrrevOp = Vec<StringIrrevComponent>;

impl StringIrrevComponent {
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
            Self::Insert(s) => s.chars().count(),
        }
    }

    fn is_delete(&self) -> bool {
        matches!(self, Self::Delete(_))
    }
}

/// Append a component, merging with the last if same type.
fn append(op: &mut StringIrrevOp, comp: StringIrrevComponent) {
    match (op.last_mut(), &comp) {
        (Some(StringIrrevComponent::Retain(n)),    StringIrrevComponent::Retain(m))    => { *n += m; return; }
        (Some(StringIrrevComponent::Delete(n)),    StringIrrevComponent::Delete(m))    => { *n += m; return; }
        (Some(StringIrrevComponent::Insert(s)),    StringIrrevComponent::Insert(t))    => { s.push_str(t); return; }
        _ => {}
    }
    op.push(comp);
}

/// Remove trailing Retain components.
pub fn trim(op: &mut StringIrrevOp) {
    while matches!(op.last(), Some(StringIrrevComponent::Retain(_))) {
        op.pop();
    }
}

/// Normalize: coalesce adjacent same-type components and strip trailing retains.
pub fn normalize(op: StringIrrevOp) -> StringIrrevOp {
    let mut result: StringIrrevOp = Vec::new();
    for comp in op {
        match &comp {
            StringIrrevComponent::Retain(0) | StringIrrevComponent::Delete(0) => {}
            StringIrrevComponent::Insert(s) if s.is_empty() => {}
            _ => append(&mut result, comp),
        }
    }
    trim(&mut result);
    result
}

/// Apply a `StringIrrevOp` to a string, returning the result.
pub fn apply(s: &str, op: &StringIrrevOp) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let mut idx = 0usize;

    for comp in op {
        match comp {
            StringIrrevComponent::Retain(n) => {
                result.extend(chars[idx..idx + n].iter());
                idx += n;
            }
            StringIrrevComponent::Delete(n) => {
                idx += n;
            }
            StringIrrevComponent::Insert(ins) => {
                result.push_str(ins);
            }
        }
    }
    result.extend(chars[idx..].iter());
    result
}

/// Compose two sequential operations into one equivalent operation.
pub fn compose(op1: &StringIrrevOp, op2: &StringIrrevOp) -> StringIrrevOp {
    let mut result: StringIrrevOp = Vec::new();
    let mut iter1 = op1.iter().peekable();
    let mut iter2 = op2.iter().peekable();
    let mut rem1: Option<StringIrrevComponent> = None;
    let mut rem2: Option<StringIrrevComponent> = None;

    loop {
        let c1 = rem1.take().or_else(|| iter1.next().cloned());
        let c2 = rem2.take().or_else(|| iter2.next().cloned());

        match (c1, c2) {
            (None, None) => break,
            (Some(c), None) => { append(&mut result, c); }
            (None, Some(c)) => { append(&mut result, c); }
            (Some(c1), Some(c2)) => {
                match (&c1, &c2) {
                    // Delete from op1 passes through unchanged
                    (StringIrrevComponent::Delete(n), _) => {
                        append(&mut result, StringIrrevComponent::Delete(*n));
                        rem2 = Some(c2);
                    }
                    // Insert from op2 passes through unchanged
                    (_, StringIrrevComponent::Insert(s)) => {
                        append(&mut result, StringIrrevComponent::Insert(s.clone()));
                        rem1 = Some(c1);
                    }
                    // Retain op1 + Retain op2
                    (StringIrrevComponent::Retain(n), StringIrrevComponent::Retain(m)) => {
                        let min = (*n).min(*m);
                        append(&mut result, StringIrrevComponent::Retain(min));
                        if n > m { rem1 = Some(StringIrrevComponent::Retain(n - m)); }
                        else if m > n { rem2 = Some(StringIrrevComponent::Retain(m - n)); }
                    }
                    // Retain op1 + Delete op2: retain becomes delete
                    (StringIrrevComponent::Retain(n), StringIrrevComponent::Delete(m)) => {
                        let min = (*n).min(*m);
                        append(&mut result, StringIrrevComponent::Delete(min));
                        if n > m { rem1 = Some(StringIrrevComponent::Retain(n - m)); }
                        else if m > n { rem2 = Some(StringIrrevComponent::Delete(m - n)); }
                    }
                    // Insert op1 + Retain op2: keep the inserted portion
                    (StringIrrevComponent::Insert(s), StringIrrevComponent::Retain(m)) => {
                        let s_len = s.chars().count();
                        let kept: String = s.chars().take(*m).collect();
                        append(&mut result, StringIrrevComponent::Insert(kept));
                        if s_len > *m { rem1 = Some(StringIrrevComponent::Insert(s.chars().skip(*m).collect())); }
                        else if *m > s_len { rem2 = Some(StringIrrevComponent::Retain(m - s_len)); }
                    }
                    // Insert op1 + Delete op2: they cancel out
                    (StringIrrevComponent::Insert(s), StringIrrevComponent::Delete(m)) => {
                        let s_len = s.chars().count();
                        if s_len > *m { rem1 = Some(StringIrrevComponent::Insert(s.chars().skip(*m).collect())); }
                        else if *m > s_len { rem2 = Some(StringIrrevComponent::Delete(m - s_len)); }
                    }
                }
            }
        }
    }
    normalize(result)
}

/// Transform `op` against `against`, assuming `left_wins` for concurrent inserts.
pub fn transform(op: &StringIrrevOp, against: &StringIrrevOp, left_wins: bool) -> StringIrrevOp {
    let mut result: StringIrrevOp = Vec::new();
    let mut op_iter = op.iter().cloned().peekable();
    let mut ag_iter = against.iter().cloned().peekable();
    let mut rem_op: Option<StringIrrevComponent> = None;
    let mut rem_ag: Option<StringIrrevComponent> = None;

    loop {
        let o = rem_op.take().or_else(|| op_iter.next());
        let a = rem_ag.take().or_else(|| ag_iter.next());

        match (o, a) {
            (None, _) => break,
            (Some(o), None) => { append(&mut result, o); }
            (Some(o), Some(a)) => {
                match (&o, &a) {
                    // Against inserts: add a retain to skip over the inserted chars
                    (_, StringIrrevComponent::Insert(s)) => {
                        let n = s.chars().count();
                        if left_wins {
                            rem_op = Some(o);
                            append(&mut result, StringIrrevComponent::Retain(n));
                        } else {
                            append(&mut result, StringIrrevComponent::Retain(n));
                            rem_op = Some(o);
                        }
                    }
                    // Op inserts: pass through
                    (StringIrrevComponent::Insert(s), _) => {
                        append(&mut result, StringIrrevComponent::Insert(s.clone()));
                        rem_ag = Some(a);
                    }
                    // Retain vs retain
                    (StringIrrevComponent::Retain(n), StringIrrevComponent::Retain(m)) => {
                        let min = (*n).min(*m);
                        append(&mut result, StringIrrevComponent::Retain(min));
                        if n > m { rem_op = Some(StringIrrevComponent::Retain(n - m)); }
                        else if m > n { rem_ag = Some(StringIrrevComponent::Retain(m - n)); }
                    }
                    // Retain vs delete: the chars we wanted to retain are gone
                    (StringIrrevComponent::Retain(n), StringIrrevComponent::Delete(m)) => {
                        let min = (*n).min(*m);
                        if n > m { rem_op = Some(StringIrrevComponent::Retain(n - m)); }
                        else if m > n { rem_ag = Some(StringIrrevComponent::Delete(m - n)); }
                    }
                    // Delete vs retain: delete passes through
                    (StringIrrevComponent::Delete(n), StringIrrevComponent::Retain(m)) => {
                        let min = (*n).min(*m);
                        append(&mut result, StringIrrevComponent::Delete(min));
                        if n > m { rem_op = Some(StringIrrevComponent::Delete(n - m)); }
                        else if m > n { rem_ag = Some(StringIrrevComponent::Retain(m - n)); }
                    }
                    // Delete vs delete: both deleting same region — op delete is redundant
                    (StringIrrevComponent::Delete(n), StringIrrevComponent::Delete(m)) => {
                        let min = (*n).min(*m);
                        if n > m { rem_op = Some(StringIrrevComponent::Delete(n - m)); }
                        else if m > n { rem_ag = Some(StringIrrevComponent::Delete(m - n)); }
                    }
                }
            }
        }
    }
    normalize(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_insert() {
        let op = vec![StringIrrevComponent::Insert("hello".to_string())];
        assert_eq!(apply("", &op), "hello");
    }

    #[test]
    fn apply_delete() {
        let op = vec![StringIrrevComponent::Delete(3)];
        assert_eq!(apply("hello", &op), "lo");
    }

    #[test]
    fn apply_retain_then_insert() {
        let op = vec![
            StringIrrevComponent::Retain(5),
            StringIrrevComponent::Insert(" world".to_string()),
        ];
        assert_eq!(apply("hello", &op), "hello world");
    }

    #[test]
    fn compose_insert_then_delete() {
        let op1 = vec![StringIrrevComponent::Insert("X".to_string())];
        let op2 = vec![StringIrrevComponent::Delete(1)];
        let composed = compose(&op1, &op2);
        assert!(composed.is_empty());
    }

    #[test]
    fn transform_concurrent_inserts() {
        let op = vec![StringIrrevComponent::Insert("A".to_string())];
        let against = vec![StringIrrevComponent::Insert("B".to_string())];
        let t = transform(&op, &against, true);
        let result = apply("hello", &t);
        assert!(result.contains('A'));
    }
}
