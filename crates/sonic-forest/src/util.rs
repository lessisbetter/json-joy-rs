//! Position-tree (p / l / r) utility functions.
//!
//! Mirrors `src/util/index.ts`.

use crate::types::Node;

#[inline]
fn get_p<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].p()
}
#[inline]
fn get_l<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].l()
}
#[inline]
fn get_r<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].r()
}
#[inline]
fn set_l<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_l(v);
}
#[inline]
fn set_r<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_r(v);
}
#[inline]
fn set_p<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_p(v);
}

/// Leftmost node in the position tree.  Mirrors `first`.
pub fn first<N: Node>(arena: &[N], root: Option<u32>) -> Option<u32> {
    let mut curr = root;
    while let Some(idx) = curr {
        match get_l(arena, idx) {
            Some(l) => curr = Some(l),
            None => return Some(idx),
        }
    }
    curr
}

/// In-order successor in the position tree.  Mirrors `next`.
pub fn next<N: Node>(arena: &[N], node: u32) -> Option<u32> {
    if let Some(r) = get_r(arena, node) {
        let mut curr = r;
        while let Some(l) = get_l(arena, curr) {
            curr = l;
        }
        return Some(curr);
    }
    let mut curr = node;
    let mut p = get_p(arena, node);
    while let Some(pi) = p {
        if get_r(arena, pi) == Some(curr) {
            curr = pi;
            p = get_p(arena, pi);
        } else {
            return Some(pi);
        }
    }
    None
}

/// Remove `node` from the position tree rooted at `root`.
///
/// Returns the new root.  Mirrors `remove` in `util/index.ts`.
pub fn remove<N: Node>(arena: &mut Vec<N>, root: Option<u32>, node: u32) -> Option<u32> {
    let p = get_p(arena, node);
    let l = get_l(arena, node);
    let r = get_r(arena, node);
    set_p(arena, node, None);
    set_l(arena, node, None);
    set_r(arena, node, None);

    match (l, r) {
        (None, None) => {
            if let Some(p) = p {
                if get_l(arena, p) == Some(node) {
                    set_l(arena, p, None);
                } else {
                    set_r(arena, p, None);
                }
            }
            if p.is_none() {
                None
            } else {
                root
            }
        }
        (Some(l), Some(r)) => {
            let mut most_right = l;
            while let Some(rr) = get_r(arena, most_right) {
                most_right = rr;
            }
            set_r(arena, most_right, Some(r));
            set_p(arena, r, Some(most_right));
            if let Some(p) = p {
                if get_l(arena, p) == Some(node) {
                    set_l(arena, p, Some(l));
                } else {
                    set_r(arena, p, Some(l));
                }
                set_p(arena, l, Some(p));
                root
            } else {
                set_p(arena, l, None);
                Some(l)
            }
        }
        _ => {
            let child = l.or(r).unwrap();
            set_p(arena, child, p);
            if let Some(p) = p {
                if get_l(arena, p) == Some(node) {
                    set_l(arena, p, Some(child));
                } else {
                    set_r(arena, p, Some(child));
                }
                root
            } else {
                Some(child)
            }
        }
    }
}
