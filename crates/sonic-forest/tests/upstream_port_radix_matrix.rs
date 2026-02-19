use std::collections::BTreeMap;

use sonic_forest::radix::{BinaryRadixTree, RadixTree};

fn expected_str<V>(pairs: &[(&str, V)]) -> BTreeMap<String, V>
where
    V: Clone,
{
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

#[test]
fn radix_insert_record_matrix() {
    let mut tree = RadixTree::<i32>::new();
    let mut cnt = 0_i32;
    let mut next = || {
        let out = cnt;
        cnt += 1;
        out
    };

    tree.set("abc", next());
    tree.set("abcd", next());
    tree.set("abcde", next());
    tree.set("abcdx", next());
    tree.set("g", next());
    tree.set("gg", next());
    tree.set("ga", next());
    tree.set("gb", next());
    tree.set("gc", next());
    tree.set("gd", next());
    tree.set("ge", next());
    tree.set("gf", next());
    tree.set("gg", next());
    tree.set("gh", next());
    tree.set("gh", next());
    tree.set("aa", next());
    tree.set("aa", next());
    tree.set("aaa", next());
    tree.set("aaa", next());

    assert_eq!(
        tree.to_record(),
        expected_str(&[
            ("abc", 0),
            ("abcd", 1),
            ("abcde", 2),
            ("abcdx", 3),
            ("aa", 16),
            ("aaa", 18),
            ("g", 4),
            ("ga", 6),
            ("gb", 7),
            ("gc", 8),
            ("gd", 9),
            ("ge", 10),
            ("gf", 11),
            ("gg", 12),
            ("gh", 14),
        ])
    );
}

#[test]
fn radix_common_prefix_matrix() {
    let mut tree = RadixTree::<i32>::new();
    tree.set("GET /users/{user}", 1);
    tree.set("GET /posts/{post}", 2);

    let root = tree.root_index();
    let root_children = tree.children_in_order(root);
    assert_eq!(root_children.len(), 1);

    let child = root_children[0];
    let node = tree.node(child);
    assert_eq!(node.k, "GET /");
    assert_eq!(node.v, None);
    assert_eq!(node.p, None);
    assert_eq!(node.l, None);
    assert_eq!(node.r, None);
    assert!(node.children.is_some());

    let leaf_children = tree.children_in_order(child);
    assert_eq!(leaf_children.len(), 2);
    assert_eq!(tree.node(leaf_children[0]).k, "posts/{post}");
    assert_eq!(tree.node(leaf_children[1]).k, "users/{user}");
}

#[test]
fn radix_tree_set_get_delete_size_matrix() {
    let mut tree = RadixTree::<i32>::new();
    assert_eq!(tree.to_record(), BTreeMap::<String, i32>::new());

    tree.set("foo", 1);
    tree.set("fo", 2);
    tree.set("f", 3);
    tree.set("bar", 4);
    tree.set("b", 5);
    tree.set("barr", 6);

    assert_eq!(tree.size, 6);
    assert_eq!(tree.get("fo"), Some(&2));
    assert_eq!(tree.get("foo"), Some(&1));
    assert_eq!(tree.get("f"), Some(&3));
    assert_eq!(tree.get("bar"), Some(&4));
    assert_eq!(tree.get("b"), Some(&5));
    assert_eq!(tree.get("barr"), Some(&6));
    assert_eq!(tree.get("baz"), None);

    assert!(tree.delete("barr"));
    assert_eq!(tree.size, 5);
    assert_eq!(tree.get("barr"), None);

    assert!(!tree.delete("barr"));
    assert_eq!(tree.size, 5);
}

#[test]
fn radix_tree_empty_key_behavior_matrix() {
    let mut tree = RadixTree::<i32>::new();
    tree.set("", 1);
    tree.set("f", 2);
    tree.set("fo", 3);
    tree.set("foo", 4);

    assert_eq!(
        tree.to_record(),
        expected_str(&[("", 1), ("f", 2), ("fo", 3), ("foo", 4)])
    );
    // Mirrors upstream: `find('')` returns root, not the inserted empty-key child.
    assert_eq!(tree.get(""), None);
}

#[test]
fn radix_tree_for_children_matrix() {
    let mut tree = RadixTree::<i32>::new();
    tree.set("foo", 1);
    tree.set("fo", 2);
    tree.set("bar", 3);

    let mut keys = Vec::new();
    tree.for_children(|child, _| keys.push(child.k.clone()));

    assert_eq!(keys, vec!["bar".to_string(), "fo".to_string()]);
}

#[test]
fn radix_tree_router_matrix() {
    let mut tree = RadixTree::<i32>::new();
    tree.set("GET /users", 1);
    tree.set("GET /files", 2);
    tree.set("PUT /files", 3);
    tree.set("POST /files", 4);
    tree.set("POST /posts", 5);

    assert_eq!(tree.size, 5);
    assert_eq!(tree.get("GET /users"), Some(&1));
    assert_eq!(tree.get("GET /files"), Some(&2));
    assert_eq!(tree.get("PUT /files"), Some(&3));
    assert_eq!(tree.get("POST /files"), Some(&4));
    assert_eq!(tree.get("POST /posts"), Some(&5));
}

fn expected_bytes<V>(pairs: &[(&str, V)]) -> BTreeMap<String, V>
where
    V: Clone,
{
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

#[test]
fn binary_radix_insert_record_matrix() {
    let mut tree = BinaryRadixTree::<i32>::new();
    let mut cnt = 0_i32;
    let mut next = || {
        let out = cnt;
        cnt += 1;
        out
    };

    tree.set([1, 2, 3], next());
    tree.set([1, 2, 3, 4], next());
    tree.set([1, 2, 3, 4, 5], next());
    tree.set([1, 2, 3, 4, 255], next());
    tree.set([100], next());
    tree.set([100, 100], next());
    tree.set([100, 1], next());
    tree.set([100, 2], next());
    tree.set([100, 3], next());
    tree.set([100, 4], next());
    tree.set([100, 5], next());
    tree.set([100, 6], next());
    tree.set([100, 100], next());
    tree.set([100, 7], next());
    tree.set([100, 7], next());
    tree.set([1, 1], next());
    tree.set([1, 1], next());
    tree.set([1, 1, 1], next());
    tree.set([1, 1, 1], next());

    assert_eq!(
        tree.to_record(),
        expected_bytes(&[
            ("1,2,3", 0),
            ("1,2,3,4", 1),
            ("1,2,3,4,5", 2),
            ("1,2,3,4,255", 3),
            ("1,1", 16),
            ("1,1,1", 18),
            ("100", 4),
            ("100,1", 6),
            ("100,2", 7),
            ("100,3", 8),
            ("100,4", 9),
            ("100,5", 10),
            ("100,6", 11),
            ("100,100", 12),
            ("100,7", 14),
        ])
    );
}

#[test]
fn binary_radix_common_prefix_matrix() {
    let mut tree = BinaryRadixTree::<i32>::new();
    tree.set([71, 69, 84, 32, 47, 117, 115, 101, 114, 115], 1);
    tree.set([71, 69, 84, 32, 47, 112, 111, 115, 116, 115], 2);

    let root = tree.root_index();
    let root_children = tree.children_in_order(root);
    assert_eq!(root_children.len(), 1);

    let child = root_children[0];
    let node = tree.node(child);
    assert_eq!(node.k.to_uint8_array(), vec![71, 69, 84, 32, 47]);
    assert_eq!(node.v, None);
    assert_eq!(node.p, None);
    assert_eq!(node.l, None);
    assert_eq!(node.r, None);
    assert!(node.children.is_some());

    let leaf_children = tree.children_in_order(child);
    assert_eq!(leaf_children.len(), 2);
    assert_eq!(
        tree.node(leaf_children[0]).k.to_uint8_array(),
        vec![112, 111, 115, 116, 115]
    );
    assert_eq!(
        tree.node(leaf_children[1]).k.to_uint8_array(),
        vec![117, 115, 101, 114, 115]
    );
}

#[test]
fn binary_radix_tree_set_get_delete_size_matrix() {
    let mut tree = BinaryRadixTree::<i32>::new();
    assert_eq!(tree.to_record(), BTreeMap::<String, i32>::new());

    tree.set([1, 2], 1);
    tree.set([1, 2, 3], 2);
    tree.set([1], 3);
    tree.set([4, 5, 6], 4);
    tree.set([4], 5);
    tree.set([4, 5, 6, 7], 6);

    assert_eq!(tree.size, 6);
    assert_eq!(tree.get([1, 2]), Some(&1));
    assert_eq!(tree.get([1, 2, 3]), Some(&2));
    assert_eq!(tree.get([1]), Some(&3));
    assert_eq!(tree.get([4, 5, 6]), Some(&4));
    assert_eq!(tree.get([4]), Some(&5));
    assert_eq!(tree.get([4, 5, 6, 7]), Some(&6));
    assert_eq!(tree.get([9, 9, 9]), None);

    assert!(tree.delete([4, 5, 6, 7]));
    assert_eq!(tree.size, 5);
    assert_eq!(tree.get([4, 5, 6, 7]), None);

    assert!(!tree.delete([4, 5, 6, 7]));
    assert_eq!(tree.size, 5);
}

#[test]
fn binary_radix_protocol_and_arbitrary_bytes_matrix() {
    let mut tree = BinaryRadixTree::<&str>::new();
    tree.set([0x47, 0x45, 0x54, 0x20], "GET ");
    tree.set([0x50, 0x4f, 0x53, 0x54], "POST");
    tree.set([0x50, 0x55, 0x54, 0x20], "PUT ");

    assert_eq!(tree.size, 3);
    assert_eq!(tree.get([0x47, 0x45, 0x54, 0x20]), Some(&"GET "));
    assert_eq!(tree.get([0x50, 0x4f, 0x53, 0x54]), Some(&"POST"));
    assert_eq!(tree.get([0x50, 0x55, 0x54, 0x20]), Some(&"PUT "));

    tree.set([0x00, 0xff, 0x80], "binary1");
    tree.set([0x00, 0xff, 0x81], "binary2");
    tree.set([0x00, 0xff], "prefix");

    assert_eq!(tree.get([0x00, 0xff, 0x80]), Some(&"binary1"));
    assert_eq!(tree.get([0x00, 0xff, 0x81]), Some(&"binary2"));
    assert_eq!(tree.get([0x00, 0xff]), Some(&"prefix"));
}

fn generate_test_key(index: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut num = index + 1;
    while num > 0 {
        bytes.push(((num % 5) + 1) as u8);
        num /= 5;
    }
    bytes.truncate(3);
    bytes
}

#[test]
fn binary_radix_fuzzing_deterministic_matrix() {
    let mut tree = BinaryRadixTree::<usize>::new();
    let mut shadow = BTreeMap::<Vec<u8>, usize>::new();

    for i in 0..20 {
        let key = generate_test_key(i);
        let value = i * 10;
        tree.set(&key, value);
        shadow.insert(key.clone(), value);

        assert_eq!(tree.size, shadow.len());
        assert_eq!(tree.get(&key), Some(&value));
    }

    for (k, v) in &shadow {
        assert_eq!(tree.get(k), Some(v));
    }
    assert_eq!(tree.size, shadow.len());
}

#[test]
fn binary_radix_fuzzing_prefix_matrix() {
    let mut tree = BinaryRadixTree::<String>::new();
    let mut shadow = BTreeMap::<Vec<u8>, String>::new();

    let keys = [
        vec![1],
        vec![1, 2],
        vec![1, 2, 3],
        vec![1, 2, 3, 4],
        vec![2],
        vec![2, 1],
        vec![2, 1, 3],
    ];

    for (i, key) in keys.iter().enumerate() {
        let value = format!("prefix-{i}");
        tree.set(key, value.clone());
        shadow.insert(key.clone(), value);

        assert_eq!(tree.size, shadow.len());
        assert_eq!(tree.get(key), shadow.get(key));
    }

    for (k, v) in &shadow {
        assert_eq!(tree.get(k), Some(v));
    }
}
