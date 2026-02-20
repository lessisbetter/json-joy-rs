use std::fmt::Debug;

use crate::types::KvNode;
use crate::util::{first, next};

use super::types::AvlNodeLike;

#[inline]
fn set_p<K, V, N>(arena: &mut [N], i: u32, v: Option<u32>)
where
    N: AvlNodeLike<K, V>,
{
    arena[i as usize].set_p(v);
}

#[inline]
fn set_l<K, V, N>(arena: &mut [N], i: u32, v: Option<u32>)
where
    N: AvlNodeLike<K, V>,
{
    arena[i as usize].set_l(v);
}

#[inline]
fn set_r<K, V, N>(arena: &mut [N], i: u32, v: Option<u32>)
where
    N: AvlNodeLike<K, V>,
{
    arena[i as usize].set_r(v);
}

#[inline]
fn bf<K, V, N>(arena: &[N], i: u32) -> i32
where
    N: AvlNodeLike<K, V>,
{
    arena[i as usize].bf()
}

#[inline]
fn set_bf<K, V, N>(arena: &mut [N], i: u32, v: i32)
where
    N: AvlNodeLike<K, V>,
{
    arena[i as usize].set_bf(v);
}

fn rebalance_after_insert<K, V, N>(arena: &mut [N], root: u32, node: u32, child: u32) -> u32
where
    N: AvlNodeLike<K, V>,
{
    let Some(p) = arena[node as usize].p() else {
        return root;
    };

    let is_left = arena[p as usize].l() == Some(node);
    let mut pbf = bf(arena, p);
    if is_left {
        pbf += 1;
    } else {
        pbf -= 1;
    }
    set_bf(arena, p, pbf);

    match pbf {
        0 => root,
        1 | -1 => rebalance_after_insert(arena, root, p, node),
        _ => {
            let is_child_left = arena[node as usize].l() == Some(child);
            if is_left {
                if is_child_left {
                    ll_rotate(arena, p, node);
                    if arena[node as usize].p().is_some() {
                        root
                    } else {
                        node
                    }
                } else {
                    lr_rotate(arena, p, node, child);
                    if arena[child as usize].p().is_some() {
                        root
                    } else {
                        child
                    }
                }
            } else if is_child_left {
                rl_rotate(arena, p, node, child);
                if arena[child as usize].p().is_some() {
                    root
                } else {
                    child
                }
            } else {
                rr_rotate(arena, p, node);
                if arena[node as usize].p().is_some() {
                    root
                } else {
                    node
                }
            }
        }
    }
}

fn ll_rotate<K, V, N>(arena: &mut [N], n: u32, nl: u32)
where
    N: AvlNodeLike<K, V>,
{
    let p = arena[n as usize].p();
    let nlr = arena[nl as usize].r();

    set_p(arena, nl, p);
    set_r(arena, nl, Some(n));
    set_p(arena, n, Some(nl));
    set_l(arena, n, nlr);
    if let Some(nlr) = nlr {
        set_p(arena, nlr, Some(n));
    }
    if let Some(p) = p {
        if arena[p as usize].l() == Some(n) {
            set_l(arena, p, Some(nl));
        } else {
            set_r(arena, p, Some(nl));
        }
    }

    let mut nbf = bf(arena, n);
    let mut nlbf = bf(arena, nl);
    nbf += -1 - if nlbf > 0 { nlbf } else { 0 };
    nlbf += -1 + if nbf < 0 { nbf } else { 0 };
    set_bf(arena, n, nbf);
    set_bf(arena, nl, nlbf);
}

fn rr_rotate<K, V, N>(arena: &mut [N], n: u32, nr: u32)
where
    N: AvlNodeLike<K, V>,
{
    let p = arena[n as usize].p();
    let nrl = arena[nr as usize].l();

    set_p(arena, nr, p);
    set_l(arena, nr, Some(n));
    set_p(arena, n, Some(nr));
    set_r(arena, n, nrl);
    if let Some(nrl) = nrl {
        set_p(arena, nrl, Some(n));
    }
    if let Some(p) = p {
        if arena[p as usize].l() == Some(n) {
            set_l(arena, p, Some(nr));
        } else {
            set_r(arena, p, Some(nr));
        }
    }

    let mut nbf = bf(arena, n);
    let mut nrbf = bf(arena, nr);
    nbf += 1 - if nrbf < 0 { nrbf } else { 0 };
    nrbf += 1 + if nbf > 0 { nbf } else { 0 };
    set_bf(arena, n, nbf);
    set_bf(arena, nr, nrbf);
}

fn lr_rotate<K, V, N>(arena: &mut [N], n: u32, nl: u32, nlr: u32)
where
    N: AvlNodeLike<K, V>,
{
    rr_rotate(arena, nl, nlr);
    ll_rotate(arena, n, nlr);
}

fn rl_rotate<K, V, N>(arena: &mut [N], n: u32, nr: u32, nrl: u32)
where
    N: AvlNodeLike<K, V>,
{
    ll_rotate(arena, nr, nrl);
    rr_rotate(arena, n, nrl);
}

pub fn insert_right<K, V, N>(arena: &mut [N], root: Option<u32>, n: u32, p: u32) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
{
    let root = root.expect("root exists");
    set_r(arena, p, Some(n));
    set_p(arena, n, Some(p));
    let pbf = bf(arena, p) - 1;
    set_bf(arena, p, pbf);
    if arena[p as usize].l().is_some() {
        Some(root)
    } else {
        Some(rebalance_after_insert(arena, root, p, n))
    }
}

pub fn insert_left<K, V, N>(arena: &mut [N], root: Option<u32>, n: u32, p: u32) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
{
    let root = root.expect("root exists");
    set_l(arena, p, Some(n));
    set_p(arena, n, Some(p));
    let pbf = bf(arena, p) + 1;
    set_bf(arena, p, pbf);
    if arena[p as usize].r().is_some() {
        Some(root)
    } else {
        Some(rebalance_after_insert(arena, root, p, n))
    }
}

pub fn insert<K, V, N, C>(arena: &mut [N], root: Option<u32>, n: u32, comparator: &C) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
    C: Fn(&K, &K) -> i32,
{
    let Some(mut curr) = root else {
        return Some(n);
    };

    let key = arena[n as usize].key();
    loop {
        let cmp = comparator(key, arena[curr as usize].key());
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

pub fn remove<K, V, N>(arena: &mut Vec<N>, root: Option<u32>, n: u32) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
{
    let Some(root) = root else {
        return Some(n);
    };

    let p = arena[n as usize].p();
    let l = arena[n as usize].l();
    let r = arena[n as usize].r();
    set_p(arena, n, None);
    set_l(arena, n, None);
    set_r(arena, n, None);

    if let (Some(l), Some(r)) = (l, r) {
        let lr = arena[l as usize].r();
        if lr.is_none() {
            if let Some(p) = p {
                if arena[p as usize].l() == Some(n) {
                    set_l(arena, p, Some(l));
                } else {
                    set_r(arena, p, Some(l));
                }
            }
            set_p(arena, l, p);
            set_r(arena, l, Some(r));
            set_p(arena, r, Some(l));
            let nbf = bf(arena, n);
            if p.is_some() {
                set_bf(arena, l, nbf);
                return l_rebalance(arena, Some(root), l, 1);
            }

            let lbf = nbf - 1;
            set_bf(arena, l, lbf);
            if lbf >= -1 {
                return Some(l);
            }
            let rl = arena[r as usize].l();
            if bf(arena, r) > 0 {
                let rl = rl.expect("rl exists");
                rl_rotate(arena, l, r, rl);
                return Some(rl);
            }
            rr_rotate(arena, l, r);
            return Some(r);
        }

        // In-order predecessor path.
        let mut v = l;
        while let Some(tmp) = arena[v as usize].r() {
            v = tmp;
        }
        let vl = arena[v as usize].l();
        let vp = arena[v as usize]
            .p()
            .expect("in-order predecessor has parent");
        let vc = vl;

        if let Some(p) = p {
            if arena[p as usize].l() == Some(n) {
                set_l(arena, p, Some(v));
            } else {
                set_r(arena, p, Some(v));
            }
        }

        set_p(arena, v, p);
        set_r(arena, v, Some(r));
        let nbf = bf(arena, n);
        set_bf(arena, v, nbf);
        if l != v {
            set_l(arena, v, Some(l));
            set_p(arena, l, Some(v));
        }
        set_p(arena, r, Some(v));

        if arena[vp as usize].l() == Some(v) {
            set_l(arena, vp, vc);
        } else {
            set_r(arena, vp, vc);
        }
        if let Some(vc) = vc {
            set_p(arena, vc, Some(vp));
        }

        return r_rebalance(arena, if p.is_some() { Some(root) } else { Some(v) }, vp, 1);
    }

    let c = l.or(r);
    if let Some(c) = c {
        set_p(arena, c, p);
    }
    let Some(p) = p else {
        return c;
    };

    if arena[p as usize].l() == Some(n) {
        set_l(arena, p, c);
        l_rebalance(arena, Some(root), p, 1)
    } else {
        set_r(arena, p, c);
        r_rebalance(arena, Some(root), p, 1)
    }
}

fn l_rebalance<K, V, N>(arena: &mut Vec<N>, root: Option<u32>, mut n: u32, d: i32) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
{
    let mut nbf = bf(arena, n) - d;
    set_bf(arena, n, nbf);
    let mut next_d = d;

    if nbf == -1 {
        return root;
    }

    if nbf < -1 {
        let u = arena[n as usize].r().expect("right child exists");
        if bf(arena, u) <= 0 {
            if arena[u as usize].l().is_some() && bf(arena, u) == 0 {
                next_d = 0;
            }
            rr_rotate(arena, n, u);
            n = u;
        } else {
            let ul = arena[u as usize].l().expect("left child exists");
            rl_rotate(arena, n, u, ul);
            n = ul;
        }
        nbf = bf(arena, n);
        let _ = nbf;
    }

    let Some(p) = arena[n as usize].p() else {
        return Some(n);
    };

    if arena[p as usize].l() == Some(n) {
        l_rebalance(arena, root, p, next_d)
    } else {
        r_rebalance(arena, root, p, next_d)
    }
}

fn r_rebalance<K, V, N>(arena: &mut Vec<N>, root: Option<u32>, mut n: u32, d: i32) -> Option<u32>
where
    N: AvlNodeLike<K, V>,
{
    let mut nbf = bf(arena, n) + d;
    set_bf(arena, n, nbf);
    let mut next_d = d;

    if nbf == 1 {
        return root;
    }

    if nbf > 1 {
        let u = arena[n as usize].l().expect("left child exists");
        if bf(arena, u) >= 0 {
            if arena[u as usize].r().is_some() && bf(arena, u) == 0 {
                next_d = 0;
            }
            ll_rotate(arena, n, u);
            n = u;
        } else {
            let ur = arena[u as usize].r().expect("right child exists");
            lr_rotate(arena, n, u, ur);
            n = ur;
        }
        nbf = bf(arena, n);
        let _ = nbf;
    }

    let Some(p) = arena[n as usize].p() else {
        return Some(n);
    };

    if arena[p as usize].l() == Some(n) {
        l_rebalance(arena, root, p, next_d)
    } else {
        r_rebalance(arena, root, p, next_d)
    }
}

fn tree_height<K, V, N>(arena: &[N], node: u32) -> usize
where
    N: AvlNodeLike<K, V>,
{
    let l = arena[node as usize]
        .l()
        .map(|i| tree_height(arena, i))
        .unwrap_or(0);
    let r = arena[node as usize]
        .r()
        .map(|i| tree_height(arena, i))
        .unwrap_or(0);
    1 + l.max(r)
}

pub fn assert_avl_tree<K, V, N, C>(
    arena: &[N],
    root: Option<u32>,
    comparator: &C,
) -> Result<(), String>
where
    N: AvlNodeLike<K, V>,
    C: Fn(&K, &K) -> i32,
{
    let Some(root) = root else {
        return Ok(());
    };

    if arena[root as usize].p().is_some() {
        return Err("Root has parent".to_string());
    }

    fn validate_links_and_bf<K, V, N>(arena: &[N], node: u32) -> Result<(), String>
    where
        N: AvlNodeLike<K, V>,
    {
        let l = arena[node as usize].l();
        let r = arena[node as usize].r();

        if let Some(l) = l {
            if arena[l as usize].p() != Some(node) {
                return Err("Broken parent link on left child".to_string());
            }
            validate_links_and_bf(arena, l)?;
        }
        if let Some(r) = r {
            if arena[r as usize].p() != Some(node) {
                return Err("Broken parent link on right child".to_string());
            }
            validate_links_and_bf(arena, r)?;
        }

        let lh = l.map(|i| tree_height(arena, i)).unwrap_or(0) as i32;
        let rh = r.map(|i| tree_height(arena, i)).unwrap_or(0) as i32;
        let expected_bf = lh - rh;
        let actual_bf = arena[node as usize].bf();
        if actual_bf != expected_bf {
            return Err(format!(
                "Balance factor mismatch: expected {expected_bf}, got {actual_bf}"
            ));
        }
        if !(-1..=1).contains(&actual_bf) {
            return Err("AVL balance violated".to_string());
        }

        Ok(())
    }

    validate_links_and_bf(arena, root)?;

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

/// Debug printer for AVL trees.
pub fn print<K, V, N>(arena: &[N], node: Option<u32>, tab: &str) -> String
where
    K: Debug,
    V: Debug,
    N: AvlNodeLike<K, V> + KvNode<K, V>,
{
    match node {
        None => "∅".to_string(),
        Some(i) => {
            let n = &arena[i as usize];
            let left = print::<K, V, N>(arena, n.l(), &format!("{tab}  "));
            let right = print::<K, V, N>(arena, n.r(), &format!("{tab}  "));
            format!(
                "Node[{i}] [bf={}] {{ {:?} = {:?} }}\n{tab}L={left}\n{tab}R={right}",
                n.bf(),
                n.key(),
                n.value()
            )
        }
    }
}
