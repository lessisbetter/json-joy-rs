use sonic_forest::red_black::{assert_red_black_tree, insert, remove, RbNode};
use sonic_forest::util::{find, size};

fn rb_cmp(a: &i32, b: &i32) -> i32 {
    a.cmp(b) as i32
}

fn rb_key(node: &RbNode<i32, i32>) -> &i32 {
    &node.k
}

fn n(value: i32) -> RbNode<i32, i32> {
    RbNode::new(value, value)
}

fn insert_value(arena: &mut Vec<RbNode<i32, i32>>, root: Option<u32>, value: i32) -> Option<u32> {
    arena.push(n(value));
    let idx = (arena.len() - 1) as u32;
    let root = insert(arena, root, idx, &rb_cmp);
    if let Err(err) = assert_red_black_tree(arena, root, &rb_cmp) {
        panic!("invalid red-black tree after insert({value}): {err}");
    }
    root
}

fn delete_value(arena: &mut [RbNode<i32, i32>], root: Option<u32>, value: i32) -> Option<u32> {
    if let Some(idx) = find(arena, root, &value, rb_key, rb_cmp) {
        let root = remove(arena, root, idx);
        if let Err(err) = assert_red_black_tree(arena, root, &rb_cmp) {
            panic!("invalid red-black tree after delete({value}): {err}");
        }
        root
    } else {
        root
    }
}

#[test]
fn rb_util_insert_delete_various_numbers_matrix() {
    let mut arena = Vec::<RbNode<i32, i32>>::new();
    let mut root = None;

    for value in [10, 11, 12, 50, 60, 25, 100, 88, 33, 22, 55, 59, 51] {
        root = insert_value(&mut arena, root, value);
    }
    assert_eq!(size(&arena, root), 13);

    root = delete_value(&mut arena, root, 100);
    assert_eq!(size(&arena, root), 12);

    root = delete_value(&mut arena, root, 33);
    root = delete_value(&mut arena, root, 33);
    assert_eq!(size(&arena, root), 11);

    root = delete_value(&mut arena, root, 10);
    assert_eq!(size(&arena, root), 10);

    root = delete_value(&mut arena, root, 60);
    assert_eq!(size(&arena, root), 9);

    root = delete_value(&mut arena, root, 22);
    assert_eq!(size(&arena, root), 8);
}

#[test]
fn rb_util_numbers_from_0_to_100_matrix() {
    let mut arena = Vec::<RbNode<i32, i32>>::new();
    let mut root = None;

    for i in 0..=100 {
        root = insert_value(&mut arena, root, i);
        assert_eq!(size(&arena, root), (i + 1) as usize);
    }
    for i in 0..=100 {
        root = delete_value(&mut arena, root, i);
        assert_eq!(size(&arena, root), (100 - i) as usize);
    }
}

#[test]
fn rb_util_numbers_from_100_to_11_matrix() {
    let mut arena = Vec::<RbNode<i32, i32>>::new();
    let mut root = None;

    for i in (11..=100).rev() {
        root = insert_value(&mut arena, root, i);
    }
    for i in (11..=100).rev() {
        root = delete_value(&mut arena, root, i);
    }
    assert_eq!(root, None);
}

#[test]
fn rb_util_numbers_both_directions_from_50_matrix() {
    let mut arena = Vec::<RbNode<i32, i32>>::new();
    let mut root = None;

    for i in 0..=100 {
        root = insert_value(&mut arena, root, 50 + i);
        root = insert_value(&mut arena, root, 50 - i);
        assert_eq!(size(&arena, root), (i * 2 + 2) as usize);
    }
    for i in 0..=100 {
        root = delete_value(&mut arena, root, 50 - i);
        root = delete_value(&mut arena, root, 50 + i);
    }
    assert_eq!(root, None);
}
