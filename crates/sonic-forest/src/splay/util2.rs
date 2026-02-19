//! Splay-tree rotations for the **ID tree** (p2 / l2 / r2 links).
//!
//! Mirrors `src/splay/util2.ts`.
//!
//! Identical algorithm to `splay/mod.rs` but uses the `Node2` trait
//! (p2 / l2 / r2) instead of `Node` (p / l / r).

use crate::types::Node2;

// ── helpers ───────────────────────────────────────────────────────────────

#[inline]
fn get_p2<N: Node2>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].p2()
}
#[inline]
fn get_l2<N: Node2>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].l2()
}
#[inline]
fn get_r2<N: Node2>(arena: &[N], idx: u32) -> Option<u32> {
    arena[idx as usize].r2()
}
#[inline]
fn set_p2<N: Node2>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_p2(v);
}
#[inline]
fn set_l2<N: Node2>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_l2(v);
}
#[inline]
fn set_r2<N: Node2>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) {
    arena[idx as usize].set_r2(v);
}

// ── single-level rotations ────────────────────────────────────────────────

pub fn r_splay2<N: Node2>(arena: &mut Vec<N>, c2: u32, c1: u32) {
    let b = get_r2(arena, c2);
    set_p2(arena, c2, None);
    set_r2(arena, c2, Some(c1));
    set_p2(arena, c1, Some(c2));
    set_l2(arena, c1, b);
    if let Some(b) = b {
        set_p2(arena, b, Some(c1));
    }
}

pub fn l_splay2<N: Node2>(arena: &mut Vec<N>, c2: u32, c1: u32) {
    let b = get_l2(arena, c2);
    set_p2(arena, c2, None);
    set_l2(arena, c2, Some(c1));
    set_p2(arena, c1, Some(c2));
    set_r2(arena, c1, b);
    if let Some(b) = b {
        set_p2(arena, b, Some(c1));
    }
}

// ── double-level rotations ────────────────────────────────────────────────

pub fn rr_splay2<N: Node2>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    c3: u32,
    c2: u32,
    c1: u32,
) -> Option<u32> {
    let b = get_l2(arena, c2);
    let c = get_l2(arena, c3);
    let p = get_p2(arena, c1);
    set_p2(arena, c3, p);
    set_l2(arena, c3, Some(c2));
    set_p2(arena, c2, Some(c3));
    set_l2(arena, c2, Some(c1));
    set_r2(arena, c2, c);
    set_p2(arena, c1, Some(c2));
    set_r2(arena, c1, b);
    if let Some(b) = b {
        set_p2(arena, b, Some(c1));
    }
    if let Some(c) = c {
        set_p2(arena, c, Some(c2));
    }
    update_parent2(arena, root, p, c1, c3)
}

pub fn ll_splay2<N: Node2>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    c3: u32,
    c2: u32,
    c1: u32,
) -> Option<u32> {
    let b = get_r2(arena, c2);
    let c = get_r2(arena, c3);
    let p = get_p2(arena, c1);
    set_p2(arena, c3, p);
    set_r2(arena, c3, Some(c2));
    set_p2(arena, c2, Some(c3));
    set_l2(arena, c2, c);
    set_r2(arena, c2, Some(c1));
    set_p2(arena, c1, Some(c2));
    set_l2(arena, c1, b);
    if let Some(b) = b {
        set_p2(arena, b, Some(c1));
    }
    if let Some(c) = c {
        set_p2(arena, c, Some(c2));
    }
    update_parent2(arena, root, p, c1, c3)
}

pub fn lr_splay2<N: Node2>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    c3: u32,
    c2: u32,
    c1: u32,
) -> Option<u32> {
    let c = get_l2(arena, c3);
    let d = get_r2(arena, c3);
    let p = get_p2(arena, c1);
    set_p2(arena, c3, p);
    set_l2(arena, c3, Some(c2));
    set_r2(arena, c3, Some(c1));
    set_p2(arena, c2, Some(c3));
    set_r2(arena, c2, c);
    set_p2(arena, c1, Some(c3));
    set_l2(arena, c1, d);
    if let Some(c) = c {
        set_p2(arena, c, Some(c2));
    }
    if let Some(d) = d {
        set_p2(arena, d, Some(c1));
    }
    update_parent2(arena, root, p, c1, c3)
}

pub fn rl_splay2<N: Node2>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    c3: u32,
    c2: u32,
    c1: u32,
) -> Option<u32> {
    let c = get_r2(arena, c3);
    let d = get_l2(arena, c3);
    let p = get_p2(arena, c1);
    set_p2(arena, c3, p);
    set_l2(arena, c3, Some(c1));
    set_r2(arena, c3, Some(c2));
    set_p2(arena, c2, Some(c3));
    set_l2(arena, c2, c);
    set_p2(arena, c1, Some(c3));
    set_r2(arena, c1, d);
    if let Some(c) = c {
        set_p2(arena, c, Some(c2));
    }
    if let Some(d) = d {
        set_p2(arena, d, Some(c1));
    }
    update_parent2(arena, root, p, c1, c3)
}

// ── top-level splay2 ──────────────────────────────────────────────────────

/// Splay `node` to the root of the ID tree (p2/l2/r2).
///
/// Mirrors `splay2` in `splay/util2.ts`.
pub fn splay2<N: Node2>(arena: &mut Vec<N>, root: Option<u32>, node: u32) -> Option<u32> {
    let p = get_p2(arena, node);
    let Some(p) = p else {
        return root;
    };
    let pp = get_p2(arena, p);
    let l2 = get_l2(arena, p) == Some(node);
    let root = if let Some(pp) = pp {
        let l1 = get_l2(arena, pp) == Some(p);
        match (l1, l2) {
            (true, true) => ll_splay2(arena, root, node, p, pp),
            (true, false) => lr_splay2(arena, root, node, p, pp),
            (false, true) => rl_splay2(arena, root, node, p, pp),
            (false, false) => rr_splay2(arena, root, node, p, pp),
        }
    } else {
        if l2 {
            r_splay2(arena, node, p);
        } else {
            l_splay2(arena, node, p);
        }
        Some(node)
    };
    splay2(arena, root, node)
}

// ── internal helper ───────────────────────────────────────────────────────

fn update_parent2<N: Node2>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    p: Option<u32>,
    c1: u32,
    c3: u32,
) -> Option<u32> {
    if let Some(p) = p {
        if get_l2(arena, p) == Some(c1) {
            set_l2(arena, p, Some(c3));
        } else {
            set_r2(arena, p, Some(c3));
        }
        root
    } else {
        Some(c3)
    }
}
