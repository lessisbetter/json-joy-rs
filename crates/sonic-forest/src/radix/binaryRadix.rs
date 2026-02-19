use std::collections::BTreeMap;

use crate::util::{first, insert_left, insert_right, next, remove as plain_remove};

use super::binary_radix_tree::BinaryRadixTree;
use super::slice::Slice;

fn first_byte(slice: &Slice) -> i16 {
    if slice.length == 0 {
        -1
    } else {
        slice.at(0) as i16
    }
}

fn find_or_next_lower_by_first_byte<V>(
    arena: &[super::binary_trie_node::BinaryTrieNode<V>],
    root: Option<u32>,
    target_first_byte: i16,
) -> Option<u32> {
    let mut curr = root;
    let mut result = None;

    while let Some(idx) = curr {
        let byte = first_byte(&arena[idx as usize].k);
        if byte > target_first_byte {
            curr = arena[idx as usize].l;
        } else {
            result = Some(idx);
            curr = arena[idx as usize].r;
        }
    }

    result
}

/**
 * Mirrors upstream `binaryRadix.insert(root, path, value)`.
 *
 * Returns number of newly created nodes (0 on overwrite, 1 on insert).
 */
pub fn insert<V>(tree: &mut BinaryRadixTree<V>, path: &[u8], value: V) -> usize {
    let mut curr = tree.root;
    let mut k = Slice::from_uint8_array(path.to_vec());
    let mut value = Some(value);

    'main: loop {
        let child_root = tree.nodes[curr as usize].children;
        if child_root.is_none() {
            let node = tree.push_node(k, value.take());
            tree.nodes[curr as usize].children = Some(node);
            return 1;
        }

        let first = first_byte(&k);
        let mut child = child_root;
        let mut prev_child = None;
        let mut cmp = false;

        while let Some(child_idx) = child {
            prev_child = Some(child_idx);
            let child_first_byte = first_byte(&tree.nodes[child_idx as usize].k);

            if child_first_byte == first {
                let child_key = tree.nodes[child_idx as usize].k.clone();
                let common = child_key.get_common_prefix_length(&k);
                let is_child_contained = common == child_key.length;
                let is_k_contained = common == k.length;

                if is_child_contained && is_k_contained {
                    tree.nodes[child_idx as usize].v = value.take();
                    return 0;
                }

                if is_child_contained {
                    k = k.substring(common, None);
                    curr = child_idx;
                    continue 'main;
                }

                if is_k_contained {
                    let new_child_k = child_key.substring(common, None);
                    let new_child_v = tree.nodes[child_idx as usize].v.take();
                    let new_child = tree.push_node(new_child_k, new_child_v);
                    let old_children = tree.nodes[child_idx as usize].children.take();
                    tree.nodes[new_child as usize].children = old_children;

                    tree.nodes[child_idx as usize].k = k.substring(0, Some(common));
                    tree.nodes[child_idx as usize].v = value.take();
                    tree.nodes[child_idx as usize].children = Some(new_child);
                    return 1;
                }

                if common > 0 {
                    let new_child_k = child_key.substring(common, None);
                    let new_child_v = tree.nodes[child_idx as usize].v.take();
                    let new_child = tree.push_node(new_child_k, new_child_v);
                    let old_children = tree.nodes[child_idx as usize].children.take();
                    tree.nodes[new_child as usize].children = old_children;

                    tree.nodes[child_idx as usize].k = child_key.substring(0, Some(common));
                    tree.nodes[child_idx as usize].v = None;
                    tree.nodes[child_idx as usize].children = Some(new_child);
                    k = k.substring(common, None);
                    curr = child_idx;
                    continue 'main;
                }
            }

            cmp = child_first_byte > first;
            child = if cmp {
                tree.nodes[child_idx as usize].l
            } else {
                tree.nodes[child_idx as usize].r
            };
        }

        if let Some(prev) = prev_child {
            let node = tree.push_node(k, value.take());
            if cmp {
                insert_left(&mut tree.nodes, node, prev);
            } else {
                insert_right(&mut tree.nodes, node, prev);
            }
            return 1;
        }

        break;
    }

    0
}

/// Finds the node index which matches `key`, if any.
pub fn find<V>(tree: &BinaryRadixTree<V>, key: &[u8]) -> Option<u32> {
    if key.is_empty() {
        return Some(tree.root);
    }

    let key_slice = Slice::from_uint8_array(key.to_vec());
    let mut offset = 0usize;
    let mut node = Some(tree.root);

    while let Some(node_idx) = node {
        let remaining = key_slice.substring(offset, None);
        if remaining.length == 0 {
            return Some(node_idx);
        }

        let child = find_or_next_lower_by_first_byte(
            &tree.nodes,
            tree.nodes[node_idx as usize].children,
            first_byte(&remaining),
        )?;

        let child_key = &tree.nodes[child as usize].k;
        let child_len = child_key.length;
        let mut common = 0usize;
        let limit = child_len.min(remaining.length);
        while common < limit && child_key.at(common) == remaining.at(common) {
            common += 1;
        }

        if common == 0 {
            return None;
        }

        offset += common;
        if offset == key.len() {
            return Some(child);
        }
        if common < child_len {
            return None;
        }

        node = Some(child);
    }

    None
}

/// Finds matching node and all parents, including root, for non-empty keys.
pub fn find_with_parents<V>(tree: &BinaryRadixTree<V>, key: &[u8]) -> Option<Vec<u32>> {
    if key.is_empty() {
        return None;
    }

    let mut list = vec![tree.root];
    let key_slice = Slice::from_uint8_array(key.to_vec());
    let mut offset = 0usize;
    let mut node = Some(tree.root);

    while let Some(node_idx) = node {
        let remaining = key_slice.substring(offset, None);

        let child = find_or_next_lower_by_first_byte(
            &tree.nodes,
            tree.nodes[node_idx as usize].children,
            first_byte(&remaining),
        )?;

        let child_key = &tree.nodes[child as usize].k;
        let child_len = child_key.length;
        let mut common = 0usize;
        let limit = child_len.min(remaining.length);
        while common < limit && child_key.at(common) == remaining.at(common) {
            common += 1;
        }

        if common == 0 {
            return None;
        }

        offset += common;
        if common < child_len {
            return None;
        }

        list.push(child);

        if offset == key.len() {
            return Some(list);
        }

        node = Some(child);
    }

    None
}

pub fn remove<V>(tree: &mut BinaryRadixTree<V>, key: &[u8]) -> bool {
    if key.is_empty() {
        let deleted = tree.nodes[tree.root as usize].v.is_some();
        tree.nodes[tree.root as usize].v = None;
        return deleted;
    }

    let Some(list) = find_with_parents(tree, key) else {
        return false;
    };

    let last_index = list.len() - 1;
    let last = list[last_index];
    let deleted = tree.nodes[last as usize].v.is_some();
    tree.nodes[last as usize].v = None;

    for i in (1..=last_index).rev() {
        let child = list[i];
        let parent = list[i - 1];
        if tree.nodes[child as usize].v.is_some() || tree.nodes[child as usize].children.is_some() {
            break;
        }
        let children_root = tree.nodes[parent as usize].children;
        tree.nodes[parent as usize].children = plain_remove(&mut tree.nodes, children_root, child);
    }

    deleted
}

fn bytes_key(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn to_record_impl<V: Clone>(
    tree: &BinaryRadixTree<V>,
    node: u32,
    prefix: &[u8],
    record: &mut BTreeMap<String, V>,
) {
    let n = &tree.nodes[node as usize];
    let mut current_prefix = prefix.to_vec();
    current_prefix.extend(n.k.to_uint8_array());

    if let Some(v) = n.v.as_ref() {
        record.insert(bytes_key(&current_prefix), v.clone());
    }

    let mut child = first(&tree.nodes, n.children);
    while let Some(child_idx) = child {
        to_record_impl(tree, child_idx, &current_prefix, record);
        child = next(&tree.nodes, child_idx);
    }
}

pub fn to_record<V: Clone>(tree: &BinaryRadixTree<V>) -> BTreeMap<String, V> {
    let mut out = BTreeMap::new();
    to_record_impl(tree, tree.root, &[], &mut out);
    out
}

fn print_impl<V>(tree: &BinaryRadixTree<V>, node: u32, tab: &str) -> String {
    let n = &tree.nodes[node as usize];
    let value = if n.v.is_some() { " = [value]" } else { "" };
    let mut result = format!("BinaryTrieNode {}{value}", n.k);

    let children = tree.children_in_order(node);
    if !children.is_empty() {
        result.push('\n');
        for (i, child) in children.iter().enumerate() {
            let is_last = i + 1 == children.len();
            let branch = if is_last { "└── " } else { "├── " };
            let child_tab = format!("{tab}{}", if is_last { "    " } else { "│   " });
            let child_str = print_impl(tree, *child, &child_tab);
            result.push_str(tab);
            result.push_str(branch);
            result.push_str(&child_str.replace('\n', &format!("\n{child_tab}")));
            if !is_last {
                result.push('\n');
            }
        }
    }

    result
}

pub fn print<V>(tree: &BinaryRadixTree<V>, node: u32, tab: &str) -> String {
    print_impl(tree, node, tab)
}
