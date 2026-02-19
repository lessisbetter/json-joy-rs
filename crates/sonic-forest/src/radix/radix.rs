use std::collections::BTreeMap;

use crate::trie::TrieNode;
use crate::util::{first, insert_left, insert_right, next, remove as plain_remove};

use super::radix_tree::RadixTree;

fn first_char(s: &str) -> Option<char> {
    s.chars().next()
}

fn char_rank(ch: Option<char>) -> i32 {
    ch.map(|c| c as i32).unwrap_or(-1)
}

fn common_prefix_length(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

fn substring_from(s: &str, start_chars: usize) -> String {
    s.chars().skip(start_chars).collect()
}

fn substring_range(s: &str, start_chars: usize, len_chars: usize) -> String {
    s.chars().skip(start_chars).take(len_chars).collect()
}

fn find_or_next_lower_by_first_char<V>(
    arena: &[TrieNode<V>],
    root: Option<u32>,
    target: Option<char>,
) -> Option<u32> {
    let mut curr = root;
    let mut result = None;

    while let Some(idx) = curr {
        let node_char = first_char(&arena[idx as usize].k);
        if char_rank(node_char) > char_rank(target) {
            curr = arena[idx as usize].l;
        } else {
            result = Some(idx);
            curr = arena[idx as usize].r;
        }
    }

    result
}

/**
 * Mirrors upstream `radix.insert(root, path, value)`.
 *
 * Returns number of newly created nodes (0 on overwrite, 1 on insert).
 */
pub fn insert<V>(tree: &mut RadixTree<V>, path: &str, value: V) -> usize {
    let mut curr = tree.root;
    let mut k = path.to_string();
    let mut value = Some(value);

    'main: loop {
        let child_root = tree.nodes[curr as usize].children;
        if child_root.is_none() {
            let node = tree.push_node(k, value.take());
            tree.nodes[curr as usize].children = Some(node);
            return 1;
        }

        let key_char = first_char(&k);
        let mut child = child_root;
        let mut prev_child = None;
        let mut cmp = false;

        while let Some(child_idx) = child {
            prev_child = Some(child_idx);
            let child_char = first_char(&tree.nodes[child_idx as usize].k);

            if child_char == key_char {
                let child_key = tree.nodes[child_idx as usize].k.clone();
                let common = common_prefix_length(&child_key, &k);
                let is_child_contained = common == child_key.chars().count();
                let is_k_contained = common == k.chars().count();

                if is_child_contained && is_k_contained {
                    tree.nodes[child_idx as usize].v = value.take();
                    return 0;
                }

                if is_child_contained {
                    k = substring_from(&k, common);
                    curr = child_idx;
                    continue 'main;
                }

                if is_k_contained {
                    let new_child_k = substring_from(&child_key, common);
                    let new_child_v = tree.nodes[child_idx as usize].v.take();
                    let new_child = tree.push_node(new_child_k, new_child_v);
                    let old_children = tree.nodes[child_idx as usize].children.take();
                    tree.nodes[new_child as usize].children = old_children;

                    tree.nodes[child_idx as usize].k = substring_range(&k, 0, common);
                    tree.nodes[child_idx as usize].v = value.take();
                    tree.nodes[child_idx as usize].children = Some(new_child);
                    return 1;
                }

                if common > 0 {
                    let new_child_k = substring_from(&child_key, common);
                    let new_child_v = tree.nodes[child_idx as usize].v.take();
                    let new_child = tree.push_node(new_child_k, new_child_v);
                    let old_children = tree.nodes[child_idx as usize].children.take();
                    tree.nodes[new_child as usize].children = old_children;

                    tree.nodes[child_idx as usize].k = substring_range(&child_key, 0, common);
                    tree.nodes[child_idx as usize].v = None;
                    tree.nodes[child_idx as usize].children = Some(new_child);
                    k = substring_from(&k, common);
                    curr = child_idx;
                    continue 'main;
                }
            }

            cmp = char_rank(child_char) > char_rank(key_char);
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
pub fn find<V>(tree: &RadixTree<V>, key: &str) -> Option<u32> {
    if key.is_empty() {
        return Some(tree.root);
    }

    let key_len = key.chars().count();
    let mut offset = 0usize;
    let mut node = Some(tree.root);

    while let Some(node_idx) = node {
        let remaining = substring_from(key, offset);
        if remaining.is_empty() {
            return Some(node_idx);
        }

        let child = find_or_next_lower_by_first_char(
            &tree.nodes,
            tree.nodes[node_idx as usize].children,
            first_char(&remaining),
        )?;

        let child_key = &tree.nodes[child as usize].k;
        let child_len = child_key.chars().count();
        let common = common_prefix_length(child_key, &remaining);

        if common == 0 {
            return None;
        }

        offset += common;
        if offset == key_len {
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
pub fn find_with_parents<V>(tree: &RadixTree<V>, key: &str) -> Option<Vec<u32>> {
    if key.is_empty() {
        return None;
    }

    let mut list = vec![tree.root];
    let key_len = key.chars().count();
    let mut offset = 0usize;
    let mut node = Some(tree.root);

    while let Some(node_idx) = node {
        let remaining = substring_from(key, offset);
        let child = find_or_next_lower_by_first_char(
            &tree.nodes,
            tree.nodes[node_idx as usize].children,
            first_char(&remaining),
        )?;

        let child_key = &tree.nodes[child as usize].k;
        let child_len = child_key.chars().count();
        let common = common_prefix_length(child_key, &remaining);

        if common == 0 {
            return None;
        }

        offset += common;
        if common < child_len {
            return None;
        }

        list.push(child);

        if offset == key_len {
            return Some(list);
        }

        node = Some(child);
    }

    None
}

pub fn remove<V>(tree: &mut RadixTree<V>, key: &str) -> bool {
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

fn to_record_impl<V: Clone>(
    tree: &RadixTree<V>,
    node: u32,
    prefix: &str,
    record: &mut BTreeMap<String, V>,
) {
    let n = &tree.nodes[node as usize];
    let mut current_prefix = String::from(prefix);
    current_prefix.push_str(&n.k);

    if let Some(v) = n.v.as_ref() {
        record.insert(current_prefix.clone(), v.clone());
    }

    let mut child = first(&tree.nodes, n.children);
    while let Some(child_idx) = child {
        to_record_impl(tree, child_idx, &current_prefix, record);
        child = next(&tree.nodes, child_idx);
    }
}

pub fn to_record<V: Clone>(tree: &RadixTree<V>) -> BTreeMap<String, V> {
    let mut out = BTreeMap::new();
    to_record_impl(tree, tree.root, "", &mut out);
    out
}

fn print_impl<V>(tree: &RadixTree<V>, node: u32, tab: &str) -> String {
    let n = &tree.nodes[node as usize];
    let value = if n.v.is_some() { " = [value]" } else { "" };
    let mut result = format!("TrieNode {:?}{value}", n.k);

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

pub fn print<V>(tree: &RadixTree<V>, node: u32, tab: &str) -> String {
    print_impl(tree, node, tab)
}
