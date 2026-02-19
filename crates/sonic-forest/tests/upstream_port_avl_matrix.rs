use sonic_forest::avl::{AvlBstNumNumMap, AvlMap, AvlMapOld, AvlSet};

#[test]
fn avl_map_smoke_matrix() {
    let mut map = AvlMap::<f64, i32>::new();
    map.set(1.0, 1);
    map.set(3.0, 5);
    map.set(4.0, 5);
    map.set(3.0, 15);
    map.set(4.1, 0);
    map.set(44.0, 123);

    assert_eq!(map.get(&44.0), Some(&123));

    let mut keys = Vec::new();
    map.for_each(|_i, n| keys.push(n.k));
    assert_eq!(keys, vec![1.0, 3.0, 4.0, 4.1, 44.0]);
    map.assert_valid().unwrap();
}

#[test]
fn avl_map_iteration_matrix() {
    let mut map = AvlMap::<String, i32>::new();
    assert_eq!(map.first(), None);

    map.set("a".to_string(), 1);
    map.set("b".to_string(), 2);
    map.set("c".to_string(), 3);

    let mut list = Vec::new();
    let mut entry = map.first();
    while let Some(i) = entry {
        list.push((map.key(i).clone(), *map.value(i)));
        entry = map.next(i);
    }
    assert_eq!(
        list,
        vec![
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3)
        ]
    );

    let from_iterator: Vec<(String, i32)> = map
        .iterator()
        .map(|i| (map.key(i).clone(), *map.value(i)))
        .collect();
    assert_eq!(
        from_iterator,
        vec![
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3)
        ]
    );

    let mut it0 = map.iterator0();
    assert_eq!(it0().map(|i| map.key(i).clone()), Some("a".to_string()));
    assert_eq!(it0().map(|i| map.key(i).clone()), Some("b".to_string()));
    assert_eq!(it0().map(|i| map.key(i).clone()), Some("c".to_string()));
    assert_eq!(it0(), None);
}

#[test]
fn avl_map_ladder_insert_delete_matrix() {
    let mut map = AvlMap::<i32, i32>::new();

    for i in 0..300 {
        map.set(i, i);
        map.assert_valid().unwrap();
    }
    assert_eq!(map.size(), 300);

    for i in (0..300).step_by(3) {
        assert!(map.del(&i));
        map.assert_valid().unwrap();
    }

    for i in 0..300 {
        if i % 3 == 0 {
            assert_eq!(map.get(&i), None);
        } else {
            assert_eq!(map.get(&i), Some(&i));
        }
    }
}

#[test]
fn avl_map_misc_api_matrix() {
    let mut map = AvlMap::<i32, i32>::new();
    assert!(map.is_empty());
    assert_eq!(map.size(), 0);
    assert_eq!(map.get_or_next_lower(&10), None);

    let i10 = map.set(10, 100);
    let i5 = map.set(5, 50);
    let i20 = map.set(20, 200);

    assert!(!map.is_empty());
    assert_eq!(map.find(&5), Some(i5));
    assert_eq!(map.get(&10), Some(&100));
    assert_eq!(map.first().map(|i| *map.key(i)), Some(5));
    assert_eq!(map.last().map(|i| *map.key(i)), Some(20));
    assert_eq!(map.get_or_next_lower(&4), None);
    assert_eq!(map.get_or_next_lower(&19).map(|i| *map.key(i)), Some(10));
    assert_eq!(map.get_or_next_lower(&21).map(|i| *map.key(i)), Some(20));

    *map.get_mut(&10).unwrap() = 101;
    *map.value_mut_by_index(i20) = 201;
    assert_eq!(map.get(&10), Some(&101));
    assert_eq!(map.get(&20), Some(&201));

    assert!(map.has(&10));
    assert!(map.del(&10));
    assert!(!map.del(&10));

    map.clear();
    assert!(map.is_empty());
    assert_eq!(map.size(), 0);
    assert_eq!(map.first(), None);

    let _ = i10;
}

#[test]
fn avl_set_matrix() {
    let mut set = AvlSet::<i32>::new();
    assert_eq!(set.size(), 0);
    assert!(!set.has(&1));

    set.add(1);
    set.add(24);
    set.add(42);
    set.add(42);
    assert_eq!(set.size(), 3);
    assert!(set.has(&1));
    assert!(set.has(&24));
    assert!(set.has(&42));
    assert!(!set.has(&25));

    let entries: Vec<i32> = set.entries().map(|i| *set.key(i)).collect();
    assert_eq!(entries, vec![1, 24, 42]);

    set.del(&24);
    set.del(&1);
    assert!(!set.has(&24));
    assert!(!set.has(&1));
    assert!(set.has(&42));
    assert_eq!(set.size(), 1);
    set.del(&42);
    assert!(set.is_empty());

    set.assert_valid().unwrap();
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pair(i32, i32);

#[test]
fn avl_set_custom_comparator_matrix() {
    let cmp = |a: &Pair, b: &Pair| {
        let dx = a.0 - b.0;
        if dx == 0 {
            a.1 - b.1
        } else {
            dx
        }
    };
    let mut set = AvlSet::<Pair, _>::with_comparator(cmp);
    set.add(Pair(0, 0));
    set.add(Pair(0, 1));
    set.add(Pair(2, 3));
    set.add(Pair(3, 3));
    assert_eq!(set.size(), 4);
    set.del(&Pair(0, 0));
    assert!(!set.has(&Pair(0, 0)));
    assert!(set.has(&Pair(0, 1)));
}

#[test]
fn avl_bst_num_num_map_smoke_matrix() {
    let mut map = AvlBstNumNumMap::new();
    map.set(1.0, 1.0);
    map.set(3.0, 5.0);
    map.set(4.0, 5.0);
    map.set(3.0, 15.0);
    map.set(4.1, 0.0);
    map.set(44.0, 123.0);

    assert_eq!(map.get(44.0), Some(123.0));
    let mut keys = Vec::new();
    map.for_each(|k, _v| keys.push(k));
    assert_eq!(keys, vec![1.0, 3.0, 4.0, 4.1, 44.0]);
    map.assert_valid().unwrap();
}

#[test]
fn avl_map_old_basic_compat_matrix() {
    let mut map = AvlMapOld::<i32, i32>::new();
    map.set(1, 10);
    map.set(2, 20);
    map.set(1, 11);
    assert_eq!(map.get(&1), Some(&11));
    assert_eq!(map.size(), 2);
    assert!(map.del(&2));
    assert_eq!(map.size(), 1);
    map.assert_valid().unwrap();
}
