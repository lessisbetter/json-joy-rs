use sonic_forest::Tree;

#[test]
fn tree_works_matrix() {
    let mut tree = Tree::<f64, &str>::new();
    assert_eq!(tree.size, 0);

    tree.set(1.0, "a");
    assert_eq!(tree.size, 1);
    assert_eq!(tree.get(&1.0), Some(&"a"));
    assert_eq!(tree.get_or_next_lower(&1.0), Some(&"a"));
    assert_eq!(tree.get_or_next_lower(&2.0), Some(&"a"));

    tree.set(5.0, "b");
    assert_eq!(tree.get(&1.0), Some(&"a"));
    assert_eq!(tree.get(&5.0), Some(&"b"));
    assert_eq!(tree.get_or_next_lower(&2.0), Some(&"a"));
    assert_eq!(tree.get_or_next_lower(&5.0), Some(&"b"));
    assert_eq!(tree.get_or_next_lower(&6.0), Some(&"b"));

    tree.set(6.0, "c");
    assert_eq!(tree.get_or_next_lower(&6.0), Some(&"c"));
    assert_eq!(tree.get_or_next_lower(&6.1), Some(&"c"));

    tree.set(5.5, "d");
    assert_eq!(tree.get_or_next_lower(&5.5), Some(&"d"));
    assert_eq!(tree.get_or_next_lower(&5.6), Some(&"d"));

    tree.set(5.4, "e");
    assert_eq!(tree.get_or_next_lower(&5.45), Some(&"e"));

    tree.set(5.45, "f");
    assert_eq!(tree.get_or_next_lower(&5.45), Some(&"f"));
}

#[test]
fn tree_set_fast_iterator_delete_matrix() {
    let mut tree = Tree::<i32, i32>::new();
    tree.set_fast(3, 30);
    tree.set_fast(1, 10);
    tree.set_fast(2, 20);

    assert_eq!(tree.size, 3);
    assert_eq!(tree.max(), Some(&30));

    let mut it = tree.iterator();
    assert_eq!(it(), Some(&10));
    assert_eq!(it(), Some(&20));
    assert_eq!(it(), Some(&30));
    assert_eq!(it(), None);
    drop(it);

    assert!(tree.has(&2));
    assert_eq!(tree.delete(&2), Some(20));
    assert!(!tree.has(&2));
    assert_eq!(tree.size, 2);
    assert_eq!(tree.delete(&2), None);
}
