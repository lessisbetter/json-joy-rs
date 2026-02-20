use std::collections::HashSet;

use sonic_forest::splay;
use sonic_forest::types::Node;
use sonic_forest::util::{
    find, find_or_next_lower, first, insert, insert_left, insert_right, last, next, prev, remove,
    size, swap,
};

#[derive(Clone, Debug)]
struct TestNode {
    p: Option<u32>,
    l: Option<u32>,
    r: Option<u32>,
    k: i32,
    v: i32,
}

impl TestNode {
    fn new(k: i32, v: i32) -> Self {
        Self {
            p: None,
            l: None,
            r: None,
            k,
            v,
        }
    }
}

impl Node for TestNode {
    fn p(&self) -> Option<u32> {
        self.p
    }

    fn l(&self) -> Option<u32> {
        self.l
    }

    fn r(&self) -> Option<u32> {
        self.r
    }

    fn set_p(&mut self, v: Option<u32>) {
        self.p = v;
    }

    fn set_l(&mut self, v: Option<u32>) {
        self.l = v;
    }

    fn set_r(&mut self, v: Option<u32>) {
        self.r = v;
    }
}

fn cmp_i32(a: &i32, b: &i32) -> i32 {
    a.cmp(b) as i32
}

fn key_of(node: &TestNode) -> &i32 {
    &node.k
}

fn inorder_keys(arena: &[TestNode], root: Option<u32>) -> Vec<i32> {
    let mut out = Vec::new();
    let mut curr = first(arena, root);
    while let Some(i) = curr {
        out.push(arena[i as usize].k);
        curr = next(arena, i);
    }
    out
}

fn fixture_tree() -> (Vec<TestNode>, Option<u32>) {
    //        10
    //      /    \
    //     5      20
    //      \    / \
    //       7  15  30
    let mut arena = vec![
        TestNode::new(10, 100),
        TestNode::new(5, 50),
        TestNode::new(20, 200),
        TestNode::new(7, 70),
        TestNode::new(15, 150),
        TestNode::new(30, 300),
    ];

    arena[0].l = Some(1);
    arena[0].r = Some(2);

    arena[1].p = Some(0);
    arena[1].r = Some(3);

    arena[2].p = Some(0);
    arena[2].l = Some(4);
    arena[2].r = Some(5);

    arena[3].p = Some(1);
    arena[4].p = Some(2);
    arena[5].p = Some(2);

    (arena, Some(0))
}

fn build_tree(keys: &[i32]) -> (Vec<TestNode>, Option<u32>) {
    let mut arena = Vec::<TestNode>::new();
    let mut root = None;
    for key in keys {
        arena.push(TestNode::new(*key, *key));
        let idx = (arena.len() - 1) as u32;
        root = insert(&mut arena, root, idx, key_of, cmp_i32);
    }
    (arena, root)
}

fn assert_tree_links(arena: &[TestNode], root: Option<u32>) {
    fn walk(
        arena: &[TestNode],
        idx: u32,
        expected_parent: Option<u32>,
        visited: &mut HashSet<u32>,
    ) {
        assert!(visited.insert(idx), "cycle detected at node {idx}");
        let node = &arena[idx as usize];

        assert_eq!(node.p, expected_parent);
        if let Some(parent) = node.p {
            let parent_node = &arena[parent as usize];
            assert!(parent_node.l == Some(idx) || parent_node.r == Some(idx));
        }

        if let Some(l) = node.l {
            assert_eq!(arena[l as usize].p, Some(idx));
            walk(arena, l, Some(idx), visited);
        }
        if let Some(r) = node.r {
            assert_eq!(arena[r as usize].p, Some(idx));
            walk(arena, r, Some(idx), visited);
        }
    }

    let mut visited = HashSet::<u32>::new();
    if let Some(root) = root {
        walk(arena, root, None, &mut visited);
    }
}

#[test]
fn util_first_next_last_prev_matrix() {
    let (arena, root) = fixture_tree();
    assert_eq!(first(&arena, root).map(|i| arena[i as usize].k), Some(5));
    assert_eq!(last(&arena, root).map(|i| arena[i as usize].k), Some(30));

    let keys = inorder_keys(&arena, root);
    assert_eq!(keys, vec![5, 7, 10, 15, 20, 30]);

    let node_20 = find(&arena, root, &20, key_of, cmp_i32).unwrap();
    assert_eq!(arena[node_20 as usize].v, 200);
    assert_eq!(prev(&arena, node_20).map(|i| arena[i as usize].k), Some(15));

    let node_5 = find(&arena, root, &5, key_of, cmp_i32).unwrap();
    assert_eq!(prev(&arena, node_5), None);
}

#[test]
fn util_size_find_and_floor_matrix() {
    let (arena, root) = fixture_tree();

    assert_eq!(size(&arena, root), 6);
    assert_eq!(find(&arena, root, &15, key_of, cmp_i32), Some(4));
    assert_eq!(find(&arena, root, &999, key_of, cmp_i32), None);

    let floor_17 = find_or_next_lower(&arena, root, &17, key_of, cmp_i32).unwrap();
    assert_eq!(arena[floor_17 as usize].k, 15);

    let floor_1 = find_or_next_lower(&arena, root, &1, key_of, cmp_i32);
    assert!(floor_1.is_none());
}

#[test]
fn util_insert_shape_matrix() {
    let (arena0, root0) = build_tree(&[1]);
    assert_eq!(root0, Some(0));
    assert_tree_links(&arena0, root0);

    let (arena1, root1) = build_tree(&[1, 2]);
    assert_eq!(root1, Some(0));
    assert_eq!(arena1[0].r, Some(1));
    assert_tree_links(&arena1, root1);

    let (arena2, root2) = build_tree(&[1, 2, -1]);
    assert_eq!(root2, Some(0));
    assert_eq!(arena2[0].l, Some(2));
    assert_tree_links(&arena2, root2);

    let (arena3, root3) = build_tree(&[1, 0, -1]);
    assert_eq!(root3, Some(0));
    assert_eq!(arena3[0].l, Some(1));
    assert_eq!(arena3[1].l, Some(2));
    assert_tree_links(&arena3, root3);

    let (arena4, root4) = build_tree(&[1, 2, 3]);
    assert_eq!(root4, Some(0));
    assert_eq!(arena4[0].r, Some(1));
    assert_eq!(arena4[1].r, Some(2));
    assert_tree_links(&arena4, root4);

    let (arena5, root5) = build_tree(&[1, 3, 2]);
    assert_eq!(root5, Some(0));
    assert_eq!(arena5[0].r, Some(1));
    assert_eq!(arena5[1].l, Some(2));
    assert_tree_links(&arena5, root5);

    let (arena6, root6) = build_tree(&[1, 5, 2, 3]);
    assert_eq!(root6, Some(0));
    assert_eq!(arena6[0].r, Some(1));
    assert_eq!(arena6[1].l, Some(2));
    assert_eq!(arena6[2].r, Some(3));
    assert_tree_links(&arena6, root6);
}

#[test]
fn util_insert_left_right_and_insert_matrix() {
    let mut arena = vec![
        TestNode::new(10, 100),
        TestNode::new(5, 50),
        TestNode::new(20, 200),
    ];

    let mut root = Some(0);
    insert_left(&mut arena, 1, 0);
    insert_right(&mut arena, 2, 0);
    assert_eq!(inorder_keys(&arena, root), vec![5, 10, 20]);

    arena.push(TestNode::new(15, 150));
    let idx_15 = (arena.len() - 1) as u32;
    root = insert(&mut arena, root, idx_15, key_of, cmp_i32);

    arena.push(TestNode::new(30, 300));
    let idx_30 = (arena.len() - 1) as u32;
    root = insert(&mut arena, root, idx_30, key_of, cmp_i32);

    assert_eq!(inorder_keys(&arena, root), vec![5, 10, 15, 20, 30]);
}

#[test]
fn util_remove_matrix() {
    // Leaf removal
    let (mut arena, mut root) = fixture_tree();
    let leaf = find(&arena, root, &7, key_of, cmp_i32).unwrap();
    root = remove(&mut arena, root, leaf);
    assert_eq!(inorder_keys(&arena, root), vec![5, 10, 15, 20, 30]);

    // Root with two children removal
    let (mut arena2, mut root2) = fixture_tree();
    let root_node = find(&arena2, root2, &10, key_of, cmp_i32).unwrap();
    root2 = remove(&mut arena2, root2, root_node);
    assert_eq!(inorder_keys(&arena2, root2), vec![5, 7, 15, 20, 30]);

    // Single-child removal
    let mut arena3 = vec![TestNode::new(10, 100), TestNode::new(5, 50)];
    arena3[0].l = Some(1);
    arena3[1].p = Some(0);
    let mut root3 = Some(0);
    root3 = remove(&mut arena3, root3, 0);
    assert_eq!(root3, Some(1));
    assert_eq!(arena3[1].p, None);
}

#[test]
fn util_splay_matrix() {
    let mut arena = vec![
        TestNode::new(0, 0),
        TestNode::new(3, 3),
        TestNode::new(2, 2),
        TestNode::new(4, 4),
    ];
    let mut root = Some(0);

    insert_right(&mut arena, 1, 0);
    insert_left(&mut arena, 2, 1);
    insert_right(&mut arena, 3, 1);

    root = splay(&mut arena, root, 3, 5);
    assert_eq!(size(&arena, root), 4);
    assert_tree_links(&arena, root);
}

#[test]
fn util_swap_immediate_left_child_root_matrix() {
    let mut arena = vec![TestNode::new(0, 0), TestNode::new(1, 1)];
    arena[0].l = Some(1);
    arena[1].p = Some(0);
    let new_root = swap(&mut arena, 0, 0, 1);
    assert_eq!(new_root, 1);
    assert_eq!(arena[1].l, Some(0));
    assert_eq!(arena[0].p, Some(1));
    assert_eq!(arena[0].l, None);
    assert_eq!(arena[1].p, None);
    assert_tree_links(&arena, Some(new_root));
}

#[test]
fn util_swap_immediate_right_child_root_matrix() {
    let mut arena = vec![TestNode::new(0, 0), TestNode::new(1, 1)];
    arena[0].r = Some(1);
    arena[1].p = Some(0);
    let new_root = swap(&mut arena, 0, 0, 1);
    assert_eq!(new_root, 1);
    assert_eq!(arena[1].r, Some(0));
    assert_eq!(arena[0].p, Some(1));
    assert_eq!(arena[0].r, None);
    assert_eq!(arena[1].p, None);
    assert_tree_links(&arena, Some(new_root));
}

#[test]
fn util_swap_immediate_left_child_not_root_matrix() {
    let mut arena = vec![
        TestNode::new(0, 0),
        TestNode::new(1, 1),
        TestNode::new(2, 2),
    ];
    // 0
    // /
    //   1
    //  /
    // 2
    arena[0].l = Some(1);
    arena[1].p = Some(0);
    arena[1].l = Some(2);
    arena[2].p = Some(1);
    let new_root = swap(&mut arena, 0, 1, 2);
    assert_eq!(new_root, 0);
    assert_eq!(arena[2].l, Some(1));
    assert_eq!(arena[1].p, Some(2));
    assert_eq!(arena[1].l, None);
    assert_eq!(arena[2].p, Some(0));
    assert_tree_links(&arena, Some(new_root));
}

#[test]
fn util_swap_root_and_leaf_matrix() {
    let (mut arena, root) = fixture_tree();

    // Swap root(10) with leaf(15).
    let new_root = swap(&mut arena, root.unwrap(), 0, 4);
    assert_eq!(new_root, 4);
    assert_eq!(arena[4].p, None);
    assert_tree_links(&arena, Some(new_root));
}
