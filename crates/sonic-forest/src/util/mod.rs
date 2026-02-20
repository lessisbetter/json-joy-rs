//! Position-tree (p / l / r) utility functions.
//!
//! Mirrors upstream `src/util/*`:
//! - `first.ts` -> `first.rs`
//! - `next.ts` -> `next.rs`
//! - `swap.ts` -> `swap.rs`
//! - `index.ts` -> this module (`mod.rs`)
//!
//! Rust divergence note:
//! - Upstream key-based helpers (`find`, `insert`, `findOrNextLower`) operate on
//!   object fields directly (`node.k`). Here they accept a `key_of` accessor
//!   closure so callers can use arena-backed node layouts.

pub mod first;
pub mod index;
pub mod next;
pub mod print;
pub mod swap;

use crate::types::Node;

pub use first::first;
pub use next::next;
pub use swap::swap;

#[inline]
pub(crate) fn get_p<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].p()
}

#[inline]
pub(crate) fn get_l<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].l()
}

#[inline]
pub(crate) fn get_r<N: Node>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].r()
}

#[inline]
pub(crate) fn set_p<N: Node>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_p(v);
}

#[inline]
pub(crate) fn set_l<N: Node>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_l(v);
}

#[inline]
pub(crate) fn set_r<N: Node>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_r(v);
}

/// Rightmost node in the tree.
pub fn last<N: Node>(arena: &[N], root: Option<u32>) -> Option<u32> {
    let mut curr = root;
    while let Some(idx) = curr {
        match get_r(arena, idx) {
            Some(r) => curr = Some(r),
            None => return Some(idx),
        }
    }
    curr
}

/// In-order predecessor.
pub fn prev<N: Node>(arena: &[N], mut curr: u32) -> Option<u32> {
    if let Some(l) = get_l(arena, curr) {
        let mut c = l;
        while let Some(r) = get_r(arena, c) {
            c = r;
        }
        return Some(c);
    }
    let mut p = get_p(arena, curr);
    while let Some(pi) = p {
        if get_l(arena, pi) == Some(curr) {
            curr = pi;
            p = get_p(arena, pi);
        } else {
            return Some(pi);
        }
    }
    None
}

fn size_inner<N: Node>(arena: &[N], root: u32) -> usize {
    1 + get_l(arena, root).map_or(0, |l| size_inner(arena, l))
        + get_r(arena, root).map_or(0, |r| size_inner(arena, r))
}

/// Number of nodes under `root`.
pub fn size<N: Node>(arena: &[N], root: Option<u32>) -> usize {
    root.map_or(0, |r| size_inner(arena, r))
}

/// Finds a node by key.
pub fn find<N, K, F, C>(
    arena: &[N],
    root: Option<u32>,
    key: &K,
    key_of: F,
    comparator: C,
) -> Option<u32>
where
    N: Node,
    F: Fn(&N) -> &K,
    C: Fn(&K, &K) -> i32,
{
    let mut curr = root;
    while let Some(i) = curr {
        let cmp = comparator(key, key_of(&arena[i as usize]));
        if cmp == 0 {
            return Some(i);
        }
        curr = if cmp < 0 {
            get_l(arena, i)
        } else {
            get_r(arena, i)
        };
    }
    None
}

/// Finds node by key, or the next lower node if the exact key does not exist.
pub fn find_or_next_lower<N, K, F, C>(
    arena: &[N],
    root: Option<u32>,
    key: &K,
    key_of: F,
    comparator: C,
) -> Option<u32>
where
    N: Node,
    F: Fn(&N) -> &K,
    C: Fn(&K, &K) -> i32,
{
    let mut curr = root;
    let mut result: Option<u32> = None;
    while let Some(i) = curr {
        let cmp = comparator(key_of(&arena[i as usize]), key);
        if cmp == 0 {
            return Some(i);
        }
        if cmp > 0 {
            curr = get_l(arena, i);
        } else {
            result = Some(i);
            curr = get_r(arena, i);
        }
    }
    result
}

/// Inserts `node` immediately to the right of `parent`.
pub fn insert_right<N: Node>(arena: &mut [N], node: u32, parent: u32) {
    let r = get_r(arena, parent);
    set_r(arena, node, r);
    set_r(arena, parent, Some(node));
    set_p(arena, node, Some(parent));
    if let Some(r) = r {
        set_p(arena, r, Some(node));
    }
}

/// Inserts `node` immediately to the left of `parent`.
pub fn insert_left<N: Node>(arena: &mut [N], node: u32, parent: u32) {
    let l = get_l(arena, parent);
    set_l(arena, node, l);
    set_l(arena, parent, Some(node));
    set_p(arena, node, Some(parent));
    if let Some(l) = l {
        set_p(arena, l, Some(node));
    }
}

/// BST insert using comparator and key accessor.
pub fn insert<N, K, F, C>(
    arena: &mut [N],
    root: Option<u32>,
    node: u32,
    key_of: F,
    comparator: C,
) -> Option<u32>
where
    N: Node,
    F: Fn(&N) -> &K,
    C: Fn(&K, &K) -> i32,
{
    let Some(mut curr) = root else {
        return Some(node);
    };

    loop {
        let cmp = {
            let key = key_of(&arena[node as usize]);
            let curr_key = key_of(&arena[curr as usize]);
            comparator(key, curr_key)
        };

        let next = if cmp < 0 {
            get_l(arena, curr)
        } else {
            get_r(arena, curr)
        };

        if let Some(nxt) = next {
            curr = nxt;
        } else {
            if cmp < 0 {
                insert_left(arena, node, curr);
            } else {
                insert_right(arena, node, curr);
            }
            return root;
        }
    }
}

/// Remove `node` from the tree rooted at `root`.
///
/// Returns the new root. Mirrors `remove` in upstream `util/index.ts`.
pub fn remove<N: Node>(arena: &mut [N], root: Option<u32>, node: u32) -> Option<u32> {
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
