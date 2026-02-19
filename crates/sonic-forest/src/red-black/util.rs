use crate::util::{first, next, swap};

use super::types::RbNodeLike;

#[path = "util/print.rs"]
mod print_impl;

pub use print_impl::print;

#[inline]
fn set_p<K, V, N>(arena: &mut Vec<N>, i: u32, v: Option<u32>)
where
    N: RbNodeLike<K, V>,
{
    arena[i as usize].set_p(v);
}

#[inline]
fn set_l<K, V, N>(arena: &mut Vec<N>, i: u32, v: Option<u32>)
where
    N: RbNodeLike<K, V>,
{
    arena[i as usize].set_l(v);
}

#[inline]
fn set_r<K, V, N>(arena: &mut Vec<N>, i: u32, v: Option<u32>)
where
    N: RbNodeLike<K, V>,
{
    arena[i as usize].set_r(v);
}

#[inline]
fn is_black<K, V, N>(arena: &[N], i: u32) -> bool
where
    N: RbNodeLike<K, V>,
{
    arena[i as usize].is_black()
}

#[inline]
fn set_black<K, V, N>(arena: &mut Vec<N>, i: u32, v: bool)
where
    N: RbNodeLike<K, V>,
{
    arena[i as usize].set_black(v);
}

pub fn insert<K, V, N, C>(
    arena: &mut Vec<N>,
    root: Option<u32>,
    n: u32,
    comparator: &C,
) -> Option<u32>
where
    N: RbNodeLike<K, V>,
    C: Fn(&K, &K) -> i32,
{
    let Some(mut curr) = root else {
        set_black(arena, n, true);
        return Some(n);
    };

    let key = arena[n as usize].key();
    loop {
        let curr_key = arena[curr as usize].key();
        let cmp = comparator(key, curr_key);
        let next = if cmp < 0 {
            arena[curr as usize].l()
        } else {
            arena[curr as usize].r()
        };
        match next {
            Some(next) => curr = next,
            None => {
                return if cmp < 0 {
                    insert_left(arena, root, n, curr)
                } else {
                    insert_right(arena, root, n, curr)
                };
            }
        }
    }
}

pub fn insert_right<K, V, N>(arena: &mut Vec<N>, root: Option<u32>, n: u32, p: u32) -> Option<u32>
where
    N: RbNodeLike<K, V>,
{
    let g = arena[p as usize].p();
    set_r(arena, p, Some(n));
    set_p(arena, n, Some(p));
    if is_black(arena, p) || g.is_none() {
        return root;
    }
    let top = r_rebalance(arena, n, p, g.expect("g exists"));
    if arena[top as usize].p().is_some() {
        root
    } else {
        Some(top)
    }
}

pub fn insert_left<K, V, N>(arena: &mut Vec<N>, root: Option<u32>, n: u32, p: u32) -> Option<u32>
where
    N: RbNodeLike<K, V>,
{
    let g = arena[p as usize].p();
    set_l(arena, p, Some(n));
    set_p(arena, n, Some(p));
    if is_black(arena, p) || g.is_none() {
        return root;
    }
    let top = l_rebalance(arena, n, p, g.expect("g exists"));
    if arena[top as usize].p().is_some() {
        root
    } else {
        Some(top)
    }
}

fn r_rebalance<K, V, N>(arena: &mut Vec<N>, n: u32, p: u32, g: u32) -> u32
where
    N: RbNodeLike<K, V>,
{
    let gl = arena[g as usize].l();
    let zigzag = gl == Some(p);
    let u = if zigzag { arena[g as usize].r() } else { gl };
    let uncle_is_black = u.map(|u| is_black(arena, u)).unwrap_or(true);
    if uncle_is_black {
        set_black(arena, g, false);
        if zigzag {
            lr_rotate(arena, g, p, n);
            set_black(arena, n, true);
            return n;
        }
        set_black(arena, p, true);
        r_rotate(arena, g, p);
        return p;
    }
    recolor(arena, p, g, u)
}

fn l_rebalance<K, V, N>(arena: &mut Vec<N>, n: u32, p: u32, g: u32) -> u32
where
    N: RbNodeLike<K, V>,
{
    let gr = arena[g as usize].r();
    let zigzag = gr == Some(p);
    let u = if zigzag { arena[g as usize].l() } else { gr };
    let uncle_is_black = u.map(|u| is_black(arena, u)).unwrap_or(true);
    if uncle_is_black {
        set_black(arena, g, false);
        if zigzag {
            rl_rotate(arena, g, p, n);
            set_black(arena, n, true);
            return n;
        }
        set_black(arena, p, true);
        l_rotate(arena, g, p);
        return p;
    }
    recolor(arena, p, g, u)
}

fn recolor<K, V, N>(arena: &mut Vec<N>, p: u32, g: u32, u: Option<u32>) -> u32
where
    N: RbNodeLike<K, V>,
{
    set_black(arena, p, true);
    if let Some(u) = u {
        set_black(arena, u, true);
    }

    let gg = arena[g as usize].p();
    if let Some(gg) = gg {
        set_black(arena, g, false);
        if is_black(arena, gg) {
            return g;
        }

        let ggg = arena[gg as usize].p();
        if let Some(ggg) = ggg {
            return if arena[gg as usize].l() == Some(g) {
                l_rebalance(arena, g, gg, ggg)
            } else {
                r_rebalance(arena, g, gg, ggg)
            };
        }

        gg
    } else {
        set_black(arena, g, true);
        g
    }
}

fn l_rotate<K, V, N>(arena: &mut Vec<N>, n: u32, nl: u32)
where
    N: RbNodeLike<K, V>,
{
    let p = arena[n as usize].p();
    let nlr = arena[nl as usize].r();

    set_r(arena, nl, Some(n));
    set_l(arena, n, nlr);
    if let Some(nlr) = nlr {
        set_p(arena, nlr, Some(n));
    }

    set_p(arena, n, Some(nl));
    set_p(arena, nl, p);
    if let Some(p) = p {
        if arena[p as usize].l() == Some(n) {
            set_l(arena, p, Some(nl));
        } else {
            set_r(arena, p, Some(nl));
        }
    }
}

fn r_rotate<K, V, N>(arena: &mut Vec<N>, n: u32, nr: u32)
where
    N: RbNodeLike<K, V>,
{
    let p = arena[n as usize].p();
    let nrl = arena[nr as usize].l();

    set_l(arena, nr, Some(n));
    set_r(arena, n, nrl);
    if let Some(nrl) = nrl {
        set_p(arena, nrl, Some(n));
    }

    set_p(arena, n, Some(nr));
    set_p(arena, nr, p);
    if let Some(p) = p {
        if arena[p as usize].l() == Some(n) {
            set_l(arena, p, Some(nr));
        } else {
            set_r(arena, p, Some(nr));
        }
    }
}

fn lr_rotate<K, V, N>(arena: &mut Vec<N>, g: u32, p: u32, n: u32)
where
    N: RbNodeLike<K, V>,
{
    let gg = arena[g as usize].p();
    let nl = arena[n as usize].l();
    let nr = arena[n as usize].r();

    if let Some(gg) = gg {
        if arena[gg as usize].l() == Some(g) {
            set_l(arena, gg, Some(n));
        } else {
            set_r(arena, gg, Some(n));
        }
    }

    set_p(arena, n, gg);
    set_l(arena, n, Some(p));
    set_r(arena, n, Some(g));
    set_p(arena, p, Some(n));
    set_p(arena, g, Some(n));

    set_r(arena, p, nl);
    if let Some(nl) = nl {
        set_p(arena, nl, Some(p));
    }

    set_l(arena, g, nr);
    if let Some(nr) = nr {
        set_p(arena, nr, Some(g));
    }
}

fn rl_rotate<K, V, N>(arena: &mut Vec<N>, g: u32, p: u32, n: u32)
where
    N: RbNodeLike<K, V>,
{
    let gg = arena[g as usize].p();
    let nl = arena[n as usize].l();
    let nr = arena[n as usize].r();

    if let Some(gg) = gg {
        if arena[gg as usize].l() == Some(g) {
            set_l(arena, gg, Some(n));
        } else {
            set_r(arena, gg, Some(n));
        }
    }

    set_p(arena, n, gg);
    set_l(arena, n, Some(g));
    set_r(arena, n, Some(p));
    set_p(arena, g, Some(n));
    set_p(arena, p, Some(n));

    set_r(arena, g, nl);
    if let Some(nl) = nl {
        set_p(arena, nl, Some(g));
    }

    set_l(arena, p, nr);
    if let Some(nr) = nr {
        set_p(arena, nr, Some(p));
    }
}

pub fn remove<K, V, N>(arena: &mut Vec<N>, mut root: Option<u32>, mut n: u32) -> Option<u32>
where
    N: RbNodeLike<K, V>,
{
    let original = n;
    let r = arena[n as usize].r();
    let l = arena[n as usize].l();
    let child: Option<u32>;

    if let Some(r) = r {
        let mut successor = r;
        while let Some(next) = arena[successor as usize].l() {
            successor = next;
        }
        n = successor;
        child = arena[n as usize].r();
    } else if arena[n as usize].p().is_none() {
        if let Some(l) = l {
            set_black(arena, l, true);
            set_p(arena, l, None);
        }
        return l;
    } else {
        child = r.or(l);
    }

    if n != original {
        // Upstream also copies key/value from successor into original before swap.
        // Rust keeps key/value attached to physical nodes and only swaps topology +
        // colors here; externally observable map behavior is unchanged.
        let b = is_black(arena, n);
        let original_black = is_black(arena, original);
        set_black(arena, n, original_black);
        set_black(arena, original, b);

        let root_idx = root.expect("root exists when removing from non-empty tree");
        root = Some(swap(arena, root_idx, original, n));
        n = original;
    }

    if let Some(child) = child {
        let p = arena[n as usize].p().expect("child replacement has parent");
        set_p(arena, child, Some(p));
        if arena[p as usize].l() == Some(n) {
            set_l(arena, p, Some(child));
        } else {
            set_r(arena, p, Some(child));
        }

        if !is_black(arena, child) {
            set_black(arena, child, true);
        } else {
            root = correct_double_black(arena, root, child);
        }
    } else {
        if is_black(arena, n) {
            root = correct_double_black(arena, root, n);
        }
        let p2 = arena[n as usize].p();
        if let Some(p2) = p2 {
            if arena[p2 as usize].l() == Some(n) {
                set_l(arena, p2, None);
            } else {
                set_r(arena, p2, None);
            }
        } else {
            set_black(arena, n, true);
            return Some(n);
        }
    }

    root
}

fn correct_double_black<K, V, N>(
    arena: &mut Vec<N>,
    mut root: Option<u32>,
    mut n: u32,
) -> Option<u32>
where
    N: RbNodeLike<K, V>,
{
    loop {
        let p = match arena[n as usize].p() {
            Some(p) => p,
            None => return Some(n),
        };

        let mut sibling = if arena[p as usize].l() == Some(n) {
            arena[p as usize].r()
        } else {
            arena[p as usize].l()
        };

        let Some(mut s) = sibling else {
            n = p;
            continue;
        };

        let sl = arena[s as usize].l();
        let left_child = arena[p as usize].l() == Some(n);

        if !is_black(arena, s) && sl.map(|x| is_black(arena, x)).unwrap_or(true) {
            let sr = arena[s as usize].r();
            if sr.map(|x| is_black(arena, x)).unwrap_or(true) {
                if left_child {
                    r_rotate(arena, p, s);
                } else {
                    l_rotate(arena, p, s);
                }
                set_black(arena, p, false);
                set_black(arena, s, true);
                if arena[s as usize].p().is_none() {
                    root = Some(s);
                }
            }
        }

        if is_black(arena, p)
            && is_black(arena, s)
            && sl.map(|x| is_black(arena, x)).unwrap_or(true)
        {
            let sr = arena[s as usize].r();
            if sr.map(|x| is_black(arena, x)).unwrap_or(true) {
                set_black(arena, s, false);
                n = p;
                continue;
            }
        }

        if !is_black(arena, p) {
            let s2 = if arena[p as usize].l() == Some(n) {
                arena[p as usize].r()
            } else {
                arena[p as usize].l()
            };
            if let Some(s2) = s2 {
                let sl2 = arena[s2 as usize].l();
                if is_black(arena, s2) && sl2.map(|x| is_black(arena, x)).unwrap_or(true) {
                    let sr2 = arena[s2 as usize].r();
                    if sr2.map(|x| is_black(arena, x)).unwrap_or(true) {
                        set_black(arena, s2, false);
                        set_black(arena, p, true);
                        return root;
                    }
                }
            }
        }

        if is_black(arena, s) {
            let sl = arena[s as usize].l();
            let sr = arena[s as usize].r();

            if arena[p as usize].l() == Some(n)
                && sr.map(|x| is_black(arena, x)).unwrap_or(true)
                && sl.map(|x| !is_black(arena, x)).unwrap_or(false)
            {
                let sl = sl.expect("sl exists");
                set_black(arena, sl, true);
                set_black(arena, s, false);
                l_rotate(arena, s, sl);
            } else if arena[p as usize].r() == Some(n)
                && sl.map(|x| is_black(arena, x)).unwrap_or(true)
                && sr.map(|x| !is_black(arena, x)).unwrap_or(false)
            {
                let sr = sr.expect("sr exists");
                set_black(arena, sr, true);
                set_black(arena, s, false);
                r_rotate(arena, s, sr);
            }

            if arena[s as usize].p().is_none() {
                return Some(s);
            }

            sibling = if arena[p as usize].l() == Some(n) {
                arena[p as usize].r()
            } else {
                arena[p as usize].l()
            };
            let Some(s2) = sibling else {
                return root;
            };
            s = s2;
        }

        let parent_black = is_black(arena, p);
        set_black(arena, s, parent_black);
        set_black(arena, p, true);

        if arena[p as usize].l() == Some(n) {
            let sr = arena[s as usize]
                .r()
                .expect("right child exists for final rebalance");
            set_black(arena, sr, true);
            r_rotate(arena, p, s);
        } else {
            let sl = arena[s as usize]
                .l()
                .expect("left child exists for final rebalance");
            set_black(arena, sl, true);
            l_rotate(arena, p, s);
        }

        return if arena[s as usize].p().is_some() {
            root
        } else {
            Some(s)
        };
    }
}

pub fn assert_red_black_tree<K, V, N, C>(
    arena: &[N],
    root: Option<u32>,
    comparator: &C,
) -> Result<(), String>
where
    N: RbNodeLike<K, V>,
    C: Fn(&K, &K) -> i32,
{
    let Some(root) = root else {
        return Ok(());
    };

    if arena[root as usize].p().is_some() {
        return Err("Root has parent".to_string());
    }
    if !arena[root as usize].is_black() {
        return Err("Root is not black".to_string());
    }

    fn black_height<K, V, N>(arena: &[N], node: Option<u32>) -> Result<usize, String>
    where
        N: RbNodeLike<K, V>,
    {
        let Some(node) = node else {
            return Ok(0);
        };

        let l = arena[node as usize].l();
        let r = arena[node as usize].r();

        if let Some(li) = l {
            if arena[li as usize].p() != Some(node) {
                return Err("Broken parent link on left child".to_string());
            }
        }
        if let Some(ri) = r {
            if arena[ri as usize].p() != Some(node) {
                return Err("Broken parent link on right child".to_string());
            }
        }

        if !arena[node as usize].is_black() {
            if l.map(|i| !arena[i as usize].is_black()).unwrap_or(false) {
                return Err("Red node has red left child".to_string());
            }
            if r.map(|i| !arena[i as usize].is_black()).unwrap_or(false) {
                return Err("Red node has red right child".to_string());
            }
        }

        let lh = black_height(arena, l)?;
        let rh = black_height(arena, r)?;
        if lh != rh {
            return Err("Black height mismatch".to_string());
        }

        Ok(lh
            + if arena[node as usize].is_black() {
                1
            } else {
                0
            })
    }

    black_height(arena, Some(root))?;

    let mut curr = first(arena, Some(root));
    let mut prev_node: Option<u32> = None;
    while let Some(i) = curr {
        if let Some(prev) = prev_node {
            let cmp = comparator(arena[prev as usize].key(), arena[i as usize].key());
            if cmp > 0 {
                return Err("Node order violated".to_string());
            }
        }
        prev_node = Some(i);
        curr = next(arena, i);
    }

    Ok(())
}
