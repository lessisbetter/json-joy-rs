use sonic_forest::red_black::RbMap;

#[test]
fn rb_map_smoke_matrix() {
    let mut map = RbMap::<f64, i32>::new();
    map.set(1.0, 1);
    map.set(3.0, 5);
    map.set(4.0, 5);
    map.set(3.0, 15);
    map.set(4.1, 0);
    map.set(44.0, 123);

    assert_eq!(map.get(&44.0), Some(&123));

    let mut keys = Vec::new();
    let mut curr = map.first();
    while let Some(i) = curr {
        keys.push(*map.key(i));
        curr = map.next(i);
    }

    assert_eq!(keys, vec![1.0, 3.0, 4.0, 4.1, 44.0]);
    map.assert_valid().unwrap();
}

#[test]
fn rb_map_first_next_iteration_matrix() {
    let mut map = RbMap::<String, i32>::new();
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
    map.assert_valid().unwrap();
}

#[test]
fn rb_map_iterator0_matrix() {
    let mut map = RbMap::<String, i32>::new();
    {
        let mut empty_it = map.iterator0();
        assert_eq!(empty_it(), None);
    }

    map.set("a".to_string(), 1);
    map.set("b".to_string(), 2);
    map.set("c".to_string(), 3);

    let mut list = Vec::new();
    let mut it = map.iterator0();
    while let Some(i) = it() {
        list.push((map.key(i).clone(), *map.value(i)));
    }

    assert_eq!(
        list,
        vec![
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3)
        ]
    );
    map.assert_valid().unwrap();
}

#[test]
fn rb_map_ladder_insert_delete_matrix() {
    let mut map = RbMap::<i32, i32>::new();

    for i in 0..200 {
        map.set(i, i);
        assert_eq!(map.get(&i), Some(&i));
        map.assert_valid().unwrap();
    }

    assert_eq!(map.size(), 200);

    for i in (0..200).step_by(2) {
        assert!(map.del(&i));
        map.assert_valid().unwrap();
    }

    assert_eq!(map.size(), 100);

    for i in 0..200 {
        if i % 2 == 0 {
            assert_eq!(map.get(&i), None);
        } else {
            assert_eq!(map.get(&i), Some(&i));
        }
    }
}

#[test]
fn rb_map_iterator_entries_and_for_each_matrix() {
    let mut map = RbMap::<i32, i32>::new();
    map.set(3, 30);
    map.set(1, 10);
    map.set(2, 20);

    let from_iterator: Vec<(i32, i32)> = map
        .iterator()
        .map(|i| (*map.key(i), *map.value(i)))
        .collect();
    assert_eq!(from_iterator, vec![(1, 10), (2, 20), (3, 30)]);

    let from_entries: Vec<(i32, i32)> = map
        .entries()
        .map(|i| (*map.key(i), *map.value(i)))
        .collect();
    assert_eq!(from_entries, vec![(1, 10), (2, 20), (3, 30)]);

    let mut from_for_each = Vec::new();
    map.for_each(|_i, n| from_for_each.push((n.k, n.v)));
    assert_eq!(from_for_each, vec![(1, 10), (2, 20), (3, 30)]);
}

#[test]
fn rb_map_misc_api_matrix() {
    let mut map = RbMap::<i32, i32>::new();
    assert!(map.is_empty());
    assert_eq!(map.size(), 0);
    assert_eq!(map.first(), None);
    assert_eq!(map.last(), None);
    assert_eq!(map.get_or_next_lower(&10), None);
    assert!(!map.has(&10));

    map.set(10, 100);
    let i5 = map.set(5, 50);
    let i20 = map.set(20, 200);

    assert!(!map.is_empty());
    assert_eq!(map.size(), 3);
    assert_eq!(map.find(&5), Some(i5));
    assert_eq!(map.get(&10), Some(&100));
    assert!(map.root_index().is_some());

    *map.get_mut(&10).unwrap() = 101;
    assert_eq!(map.get(&10), Some(&101));

    *map.value_mut_by_index(i20) = 201;
    assert_eq!(map.get(&20), Some(&201));

    assert_eq!(map.get_or_next_lower(&4), None);
    assert_eq!(map.get_or_next_lower(&5).map(|i| *map.key(i)), Some(5));
    assert_eq!(map.get_or_next_lower(&19).map(|i| *map.key(i)), Some(10));
    assert_eq!(map.get_or_next_lower(&21).map(|i| *map.key(i)), Some(20));

    assert_eq!(map.first().map(|i| *map.key(i)), Some(5));
    assert_eq!(map.last().map(|i| *map.key(i)), Some(20));

    assert!(map.del(&10));
    assert!(!map.del(&10));
    assert_eq!(map.size(), 2);

    map.clear();
    assert!(map.is_empty());
    assert_eq!(map.size(), 0);
    assert_eq!(map.first(), None);
    assert_eq!(map.last(), None);
}

#[test]
fn rb_map_custom_comparator_matrix() {
    let mut map = RbMap::<i32, i32, _>::with_comparator(|a, b| {
        if a == b {
            0
        } else if a > b {
            -1
        } else {
            1
        }
    });
    map.set(1, 10);
    map.set(3, 30);
    map.set(2, 20);

    let keys: Vec<i32> = map.entries().map(|i| *map.key(i)).collect();
    assert_eq!(keys, vec![3, 2, 1]);
    map.assert_valid().unwrap();
}

#[test]
fn rb_map_trace_subset_matrix() {
    let mut map = RbMap::<i32, i32>::new();

    let trace: &[(char, i32)] = &[
        ('i', 47),
        ('i', 20),
        ('i', 14),
        ('i', 88),
        ('i', 71),
        ('i', 100),
        ('i', 8),
        ('i', 53),
        ('i', 46),
        ('i', 52),
        ('d', 41),
        ('d', 41),
        ('d', 36),
        ('d', 67),
        ('d', 68),
        ('d', 0),
        ('d', 77),
        ('d', 27),
        ('d', 7),
        ('d', 75),
        ('d', 62),
        ('d', 11),
        ('d', 31),
        ('d', 1),
        ('d', 79),
        ('d', 80),
        ('d', 96),
        ('d', 14),
    ];

    for (idx, (op, key)) in trace.iter().enumerate() {
        match op {
            'i' => {
                map.set(*key, *key);
            }
            'd' => {
                map.del(key);
            }
            _ => unreachable!(),
        }
        if let Err(err) = map.assert_valid() {
            panic!("trace failure at step {idx} ({op},{key}): {err}");
        }
    }
}
