//! ID-tree (p2 / l2 / r2) utility functions.
//!
//! Mirrors `src/util2.ts`.

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
fn set_p2<N: Node2>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_p2(v);
}
#[inline]
fn set_l2<N: Node2>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_l2(v);
}
#[inline]
fn set_r2<N: Node2>(arena: &mut [N], idx: u32, v: Option<u32>) {
    arena[idx as usize].set_r2(v);
}

// ── traversal ─────────────────────────────────────────────────────────────

/// Leftmost node in the ID tree.  Mirrors `first2`.
pub fn first2<N: Node2>(arena: &[N], root: Option<u32>) -> Option<u32> {
    let mut curr = root;
    while let Some(idx) = curr {
        match get_l2(arena, idx) {
            Some(l) => curr = Some(l),
            None => return Some(idx),
        }
    }
    curr
}

/// Rightmost node in the ID tree.  Mirrors `last2`.
pub fn last2<N: Node2>(arena: &[N], root: Option<u32>) -> Option<u32> {
    let mut curr = root;
    while let Some(idx) = curr {
        match get_r2(arena, idx) {
            Some(r) => curr = Some(r),
            None => return Some(idx),
        }
    }
    curr
}

/// In-order successor in the ID tree.  Mirrors `next2`.
pub fn next2<N: Node2>(arena: &[N], node: u32) -> Option<u32> {
    if let Some(r) = get_r2(arena, node) {
        let mut curr = r;
        while let Some(l) = get_l2(arena, curr) {
            curr = l;
        }
        return Some(curr);
    }
    let mut curr = node;
    let mut p = get_p2(arena, node);
    while let Some(pi) = p {
        if get_r2(arena, pi) == Some(curr) {
            curr = pi;
            p = get_p2(arena, pi);
        } else {
            return Some(pi);
        }
    }
    None
}

/// In-order predecessor in the ID tree.  Mirrors `prev2`.
pub fn prev2<N: Node2>(arena: &[N], node: u32) -> Option<u32> {
    if let Some(l) = get_l2(arena, node) {
        let mut curr = l;
        while let Some(r) = get_r2(arena, curr) {
            curr = r;
        }
        return Some(curr);
    }
    let mut curr = node;
    let mut p = get_p2(arena, node);
    while let Some(pi) = p {
        if get_l2(arena, pi) == Some(curr) {
            curr = pi;
            p = get_p2(arena, pi);
        } else {
            return Some(pi);
        }
    }
    None
}

// ── mutation ──────────────────────────────────────────────────────────────

fn insert_right2<N: Node2>(arena: &mut [N], node: u32, p: u32) {
    let r = get_r2(arena, p);
    set_r2(arena, p, Some(node));
    set_p2(arena, node, Some(p));
    set_r2(arena, node, r);
    if let Some(r) = r {
        set_p2(arena, r, Some(node));
    }
}

fn insert_left2<N: Node2>(arena: &mut [N], node: u32, p: u32) {
    let l = get_l2(arena, p);
    set_l2(arena, p, Some(node));
    set_p2(arena, node, Some(p));
    set_l2(arena, node, l);
    if let Some(l) = l {
        set_p2(arena, l, Some(node));
    }
}

/// Insert `node` into the ID tree rooted at `root` using `comparator`.
///
/// Returns the (unchanged) root.  Mirrors `insert2` in `util2.ts`.
pub fn insert2<N, F>(arena: &mut [N], root: Option<u32>, node: u32, comparator: F) -> Option<u32>
where
    N: Node2,
    F: Fn(&N, &N) -> std::cmp::Ordering,
{
    let Some(root_idx) = root else {
        return Some(node);
    };
    let mut curr = root_idx;
    loop {
        let cmp = comparator(&arena[node as usize], &arena[curr as usize]);
        let next = if cmp.is_lt() {
            get_l2(arena, curr)
        } else {
            get_r2(arena, curr)
        };
        match next {
            None => {
                if cmp.is_lt() {
                    insert_left2(arena, node, curr);
                } else {
                    insert_right2(arena, node, curr);
                }
                break;
            }
            Some(n) => curr = n,
        }
    }
    root
}

/// Remove `node` from the ID tree rooted at `root`.
///
/// Returns the new root.  Mirrors `remove2` in `util2.ts`.
pub fn remove2<N: Node2>(arena: &mut [N], root: Option<u32>, node: u32) -> Option<u32> {
    let p = get_p2(arena, node);
    let l = get_l2(arena, node);
    let r = get_r2(arena, node);
    set_p2(arena, node, None);
    set_l2(arena, node, None);
    set_r2(arena, node, None);

    match (l, r) {
        (None, None) => {
            if let Some(p) = p {
                if get_l2(arena, p) == Some(node) {
                    set_l2(arena, p, None);
                } else {
                    set_r2(arena, p, None);
                }
            }
            if p.is_none() {
                None
            } else {
                root
            }
        }
        (Some(l), Some(r)) => {
            // Find rightmost child of left subtree and attach r there.
            let mut most_right = l;
            while let Some(rr) = get_r2(arena, most_right) {
                most_right = rr;
            }
            set_r2(arena, most_right, Some(r));
            set_p2(arena, r, Some(most_right));
            if let Some(p) = p {
                if get_l2(arena, p) == Some(node) {
                    set_l2(arena, p, Some(l));
                } else {
                    set_r2(arena, p, Some(l));
                }
                set_p2(arena, l, Some(p));
                root
            } else {
                set_p2(arena, l, None);
                Some(l)
            }
        }
        _ => {
            let child = l.or(r).unwrap();
            set_p2(arena, child, p);
            if let Some(p) = p {
                if get_l2(arena, p) == Some(node) {
                    set_l2(arena, p, Some(child));
                } else {
                    set_r2(arena, p, Some(child));
                }
                root
            } else {
                Some(child)
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splay::util2::splay2;

    /// Minimal node with only ID-tree links and a key for ordering.
    #[derive(Debug, Clone, Default)]
    struct N {
        key: u64,
        p2: Option<u32>,
        l2: Option<u32>,
        r2: Option<u32>,
    }

    impl Node2 for N {
        fn p2(&self) -> Option<u32> {
            self.p2
        }
        fn l2(&self) -> Option<u32> {
            self.l2
        }
        fn r2(&self) -> Option<u32> {
            self.r2
        }
        fn set_p2(&mut self, v: Option<u32>) {
            self.p2 = v;
        }
        fn set_l2(&mut self, v: Option<u32>) {
            self.l2 = v;
        }
        fn set_r2(&mut self, v: Option<u32>) {
            self.r2 = v;
        }
    }

    fn cmp(a: &N, b: &N) -> std::cmp::Ordering {
        a.key.cmp(&b.key)
    }

    fn node(key: u64) -> N {
        N {
            key,
            ..Default::default()
        }
    }

    /// Build a tree from sorted keys and collect in-order traversal.
    fn collect_inorder(arena: &[N], root: Option<u32>) -> Vec<u64> {
        let mut result = Vec::new();
        let mut curr = first2(arena, root);
        while let Some(idx) = curr {
            result.push(arena[idx as usize].key);
            curr = next2(arena, idx);
        }
        result
    }

    #[test]
    fn insert2_and_traverse_in_order() {
        let mut arena: Vec<N> = vec![node(5), node(2), node(8), node(1), node(4)];
        let mut root: Option<u32> = None;
        for i in 0..arena.len() as u32 {
            root = insert2(&mut arena, root, i, cmp);
        }
        assert_eq!(collect_inorder(&arena, root), vec![1, 2, 4, 5, 8]);
    }

    #[test]
    fn remove2_leaf() {
        let mut arena: Vec<N> = vec![node(5), node(2), node(8)];
        let mut root: Option<u32> = None;
        for i in 0..arena.len() as u32 {
            root = insert2(&mut arena, root, i, cmp);
        }
        // Remove node 2 (key=2, index=1)
        root = remove2(&mut arena, root, 1);
        assert_eq!(collect_inorder(&arena, root), vec![5, 8]);
    }

    #[test]
    fn remove2_root() {
        let mut arena: Vec<N> = vec![node(5), node(2), node(8)];
        let mut root: Option<u32> = None;
        for i in 0..arena.len() as u32 {
            root = insert2(&mut arena, root, i, cmp);
        }
        // Remove the current root
        let root_idx = root.unwrap();
        root = remove2(&mut arena, root, root_idx);
        assert_eq!(collect_inorder(&arena, root), vec![2, 8]);
    }

    #[test]
    fn splay2_brings_node_to_root() {
        let mut arena: Vec<N> = vec![node(1), node(2), node(3), node(4), node(5)];
        let mut root: Option<u32> = None;
        for i in 0..arena.len() as u32 {
            root = insert2(&mut arena, root, i, cmp);
        }
        // Splay node 0 (key=1, leftmost) to root
        root = splay2(&mut arena, root, 0);
        assert_eq!(root, Some(0));
        assert!(arena[0].p2.is_none());
        // In-order traversal must still be correct.
        assert_eq!(collect_inorder(&arena, root), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn next2_and_prev2() {
        let mut arena: Vec<N> = vec![node(10), node(20), node(30)];
        let mut root: Option<u32> = None;
        for i in 0..arena.len() as u32 {
            root = insert2(&mut arena, root, i, cmp);
        }
        let first = first2(&arena, root).unwrap();
        let second = next2(&arena, first).unwrap();
        let third = next2(&arena, second).unwrap();
        assert_eq!(arena[first as usize].key, 10);
        assert_eq!(arena[second as usize].key, 20);
        assert_eq!(arena[third as usize].key, 30);
        assert!(next2(&arena, third).is_none());
        assert_eq!(prev2(&arena, third), Some(second));
    }
}
