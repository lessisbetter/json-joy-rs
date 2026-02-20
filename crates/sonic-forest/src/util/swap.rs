use crate::types::Node;

use super::{get_l, get_p, get_r, set_l, set_p, set_r};

/// Swaps two node positions in a binary tree.
/// Mirrors upstream `util/swap.ts`.
pub fn swap<N: Node>(arena: &mut [N], mut root: u32, x: u32, y: u32) -> u32 {
    if x == y {
        return root;
    }

    let xp = get_p(arena, x);
    let xl = get_l(arena, x);
    let xr = get_r(arena, x);

    let yp = get_p(arena, y);
    let yl = get_l(arena, y);
    let yr = get_r(arena, y);

    if yl == Some(x) {
        set_l(arena, x, Some(y));
        set_p(arena, y, Some(x));
    } else {
        set_l(arena, x, yl);
        if let Some(yl) = yl {
            set_p(arena, yl, Some(x));
        }
    }

    if yr == Some(x) {
        set_r(arena, x, Some(y));
        set_p(arena, y, Some(x));
    } else {
        set_r(arena, x, yr);
        if let Some(yr) = yr {
            set_p(arena, yr, Some(x));
        }
    }

    if xl == Some(y) {
        set_l(arena, y, Some(x));
        set_p(arena, x, Some(y));
    } else {
        set_l(arena, y, xl);
        if let Some(xl) = xl {
            set_p(arena, xl, Some(y));
        }
    }

    if xr == Some(y) {
        set_r(arena, y, Some(x));
        set_p(arena, x, Some(y));
    } else {
        set_r(arena, y, xr);
        if let Some(xr) = xr {
            set_p(arena, xr, Some(y));
        }
    }

    if xp.is_none() {
        root = y;
        set_p(arena, y, None);
    } else if xp != Some(y) {
        set_p(arena, y, xp);
        if let Some(xp) = xp {
            if get_l(arena, xp) == Some(x) {
                set_l(arena, xp, Some(y));
            } else {
                set_r(arena, xp, Some(y));
            }
        }
    }

    if yp.is_none() {
        root = x;
        set_p(arena, x, None);
    } else if yp != Some(x) {
        set_p(arena, x, yp);
        if let Some(yp) = yp {
            if get_l(arena, yp) == Some(y) {
                set_l(arena, yp, Some(x));
            } else {
                set_r(arena, yp, Some(x));
            }
        }
    }

    root
}
