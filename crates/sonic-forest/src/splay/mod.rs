//! Splay-tree rotations for the **position tree** (p / l / r links).
//!
//! Mirrors `src/splay/util.ts`.
//!
//! All functions take the arena `Vec<N>` and node indices (u32).
//! The naming mirrors the TypeScript originals exactly:
//! `rSplay` → [`r_splay`], `lSplay` → [`l_splay`], etc.

pub mod util2;

use crate::types::Node;

// ── helpers ───────────────────────────────────────────────────────────────

#[inline]
fn get_p<N: Node>(arena: &[N], idx: u32) -> Option<u32>  { arena[idx as usize].p() }
#[inline]
fn get_l<N: Node>(arena: &[N], idx: u32) -> Option<u32>  { arena[idx as usize].l() }
#[inline]
fn get_r<N: Node>(arena: &[N], idx: u32) -> Option<u32>  { arena[idx as usize].r() }
#[inline]
fn set_p<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) { arena[idx as usize].set_p(v); }
#[inline]
fn set_l<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) { arena[idx as usize].set_l(v); }
#[inline]
fn set_r<N: Node>(arena: &mut Vec<N>, idx: u32, v: Option<u32>) { arena[idx as usize].set_r(v); }

// ── single-level rotations ────────────────────────────────────────────────

/// Right-splay: promote `c2` over `c1` (c2 was left child of c1).
///
/// ```text
///   c1           c2
///  /      →        \
/// c2               c1
///   \             /
///    b           b
/// ```
///
/// Mirrors `rSplay` in `splay/util.ts`.
pub fn r_splay<N: Node>(arena: &mut Vec<N>, c2: u32, c1: u32) {
    let b = get_r(arena, c2);
    set_p(arena, c2, None);
    set_r(arena, c2, Some(c1));
    set_p(arena, c1, Some(c2));
    set_l(arena, c1, b);
    if let Some(b) = b { set_p(arena, b, Some(c1)); }
}

/// Left-splay: promote `c2` over `c1` (c2 was right child of c1).
///
/// Mirrors `lSplay` in `splay/util.ts`.
pub fn l_splay<N: Node>(arena: &mut Vec<N>, c2: u32, c1: u32) {
    let b = get_l(arena, c2);
    set_p(arena, c2, None);
    set_l(arena, c2, Some(c1));
    set_p(arena, c1, Some(c2));
    set_r(arena, c1, b);
    if let Some(b) = b { set_p(arena, b, Some(c1)); }
}

// ── double-level rotations ────────────────────────────────────────────────

/// rr-splay: c3 was right child of c2, c2 was right child of c1.
/// Promotes c3 two levels up (zig-zig right).
///
/// Mirrors `rrSplay` in `splay/util.ts`.
pub fn rr_splay<N: Node>(arena: &mut Vec<N>, root: Option<u32>, c3: u32, c2: u32, c1: u32) -> Option<u32> {
    let b = get_l(arena, c2);
    let c = get_l(arena, c3);
    let p = get_p(arena, c1);
    set_p(arena, c3, p);
    set_l(arena, c3, Some(c2));
    set_p(arena, c2, Some(c3));
    set_l(arena, c2, Some(c1));
    set_r(arena, c2, c);
    set_p(arena, c1, Some(c2));
    set_r(arena, c1, b);
    if let Some(b) = b { set_p(arena, b, Some(c1)); }
    if let Some(c) = c { set_p(arena, c, Some(c2)); }
    update_parent(arena, root, p, c1, c3)
}

/// ll-splay: c3 was left child of c2, c2 was left child of c1.
/// Promotes c3 two levels up (zig-zig left).
///
/// Mirrors `llSplay` in `splay/util.ts`.
pub fn ll_splay<N: Node>(arena: &mut Vec<N>, root: Option<u32>, c3: u32, c2: u32, c1: u32) -> Option<u32> {
    let b = get_r(arena, c2);
    let c = get_r(arena, c3);
    let p = get_p(arena, c1);
    set_p(arena, c3, p);
    set_r(arena, c3, Some(c2));
    set_p(arena, c2, Some(c3));
    set_l(arena, c2, c);
    set_r(arena, c2, Some(c1));
    set_p(arena, c1, Some(c2));
    set_l(arena, c1, b);
    if let Some(b) = b { set_p(arena, b, Some(c1)); }
    if let Some(c) = c { set_p(arena, c, Some(c2)); }
    update_parent(arena, root, p, c1, c3)
}

/// lr-splay: c3 was right child of c2, c2 was left child of c1.
/// Promotes c3 two levels up (zig-zag left-right).
///
/// Mirrors `lrSplay` in `splay/util.ts`.
pub fn lr_splay<N: Node>(arena: &mut Vec<N>, root: Option<u32>, c3: u32, c2: u32, c1: u32) -> Option<u32> {
    let c = get_l(arena, c3);
    let d = get_r(arena, c3);
    let p = get_p(arena, c1);
    set_p(arena, c3, p);
    set_l(arena, c3, Some(c2));
    set_r(arena, c3, Some(c1));
    set_p(arena, c2, Some(c3));
    set_r(arena, c2, c);
    set_p(arena, c1, Some(c3));
    set_l(arena, c1, d);
    if let Some(c) = c { set_p(arena, c, Some(c2)); }
    if let Some(d) = d { set_p(arena, d, Some(c1)); }
    update_parent(arena, root, p, c1, c3)
}

/// rl-splay: c3 was left child of c2, c2 was right child of c1.
/// Promotes c3 two levels up (zig-zag right-left).
///
/// Mirrors `rlSplay` in `splay/util.ts`.
pub fn rl_splay<N: Node>(arena: &mut Vec<N>, root: Option<u32>, c3: u32, c2: u32, c1: u32) -> Option<u32> {
    let c = get_r(arena, c3);
    let d = get_l(arena, c3);
    let p = get_p(arena, c1);
    set_p(arena, c3, p);
    set_l(arena, c3, Some(c1));
    set_r(arena, c3, Some(c2));
    set_p(arena, c2, Some(c3));
    set_l(arena, c2, c);
    set_p(arena, c1, Some(c3));
    set_r(arena, c1, d);
    if let Some(c) = c { set_p(arena, c, Some(c2)); }
    if let Some(d) = d { set_p(arena, d, Some(c1)); }
    update_parent(arena, root, p, c1, c3)
}

// ── top-level splay ───────────────────────────────────────────────────────

/// Splay `node` toward the root, repeating `repeat` times.
///
/// Mirrors `splay` in `splay/util.ts`.
pub fn splay<N: Node>(arena: &mut Vec<N>, root: Option<u32>, node: u32, repeat: usize) -> Option<u32> {
    let p = get_p(arena, node);
    let Some(p) = p else { return root; };
    let pp = get_p(arena, p);
    let l2 = get_l(arena, p) == Some(node);
    let root = if let Some(pp) = pp {
        let l1 = get_l(arena, pp) == Some(p);
        match (l1, l2) {
            (true,  true)  => ll_splay(arena, root, node, p, pp),
            (true,  false) => lr_splay(arena, root, node, p, pp),
            (false, true)  => rl_splay(arena, root, node, p, pp),
            (false, false) => rr_splay(arena, root, node, p, pp),
        }
    } else {
        if l2 { r_splay(arena, node, p); }
        else  { l_splay(arena, node, p); }
        Some(node)
    };
    if repeat > 1 { splay(arena, root, node, repeat - 1) } else { root }
}

// ── internal helper ───────────────────────────────────────────────────────

/// After a double rotation that moved `c3` into the slot previously occupied
/// by `c1`, wire `c3` into c1's old parent `p`.
fn update_parent<N: Node>(arena: &mut Vec<N>, root: Option<u32>, p: Option<u32>, c1: u32, c3: u32) -> Option<u32> {
    if let Some(p) = p {
        if get_l(arena, p) == Some(c1) { set_l(arena, p, Some(c3)); }
        else                            { set_r(arena, p, Some(c3)); }
        root
    } else {
        Some(c3)
    }
}
