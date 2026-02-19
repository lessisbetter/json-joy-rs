use crate::splay::splay;
use crate::util::{find, find_or_next_lower, first, insert, last, next, remove};

use super::tree_node::TreeNode;

fn default_comparator<K: PartialOrd>(a: &K, b: &K) -> i32 {
    if a == b {
        0
    } else if a < b {
        -1
    } else {
        1
    }
}

/// Mirrors upstream `Tree.ts` API.
pub struct Tree<K, V, C = fn(&K, &K) -> i32>
where
    C: Fn(&K, &K) -> i32,
{
    pub root: Option<u32>,
    pub size: usize,
    pub comparator: C,
    arena: Vec<TreeNode<K, V>>,
}

impl<K, V> Tree<K, V, fn(&K, &K) -> i32>
where
    K: PartialOrd,
{
    pub fn new() -> Self {
        Self::with_comparator(default_comparator::<K>)
    }
}

impl<K, V> Default for Tree<K, V, fn(&K, &K) -> i32>
where
    K: PartialOrd,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, C> Tree<K, V, C>
where
    C: Fn(&K, &K) -> i32,
{
    pub fn with_comparator(comparator: C) -> Self {
        Self {
            root: None,
            size: 0,
            comparator,
            arena: Vec::new(),
        }
    }

    fn push_node(&mut self, key: K, value: V) -> u32 {
        self.arena.push(TreeNode::new(key, value));
        (self.arena.len() - 1) as u32
    }

    pub fn set(&mut self, key: K, value: V) {
        let node = self.push_node(key, value);
        self.root = insert(
            &mut self.arena,
            self.root,
            node,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        );
        self.root = splay(&mut self.arena, self.root, node, 15);
        self.size += 1;
    }

    pub fn set_fast(&mut self, key: K, value: V) {
        let node = self.push_node(key, value);
        self.root = insert(
            &mut self.arena,
            self.root,
            node,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        );
        self.size += 1;
    }

    #[allow(non_snake_case)]
    pub fn setFast(&mut self, key: K, value: V) {
        self.set_fast(key, value);
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        find(
            &self.arena,
            self.root,
            key,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        )
        .and_then(|idx| self.arena[idx as usize].v.as_ref())
    }

    pub fn get_or_next_lower(&self, key: &K) -> Option<&V> {
        find_or_next_lower(
            &self.arena,
            self.root,
            key,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        )
        .and_then(|idx| self.arena[idx as usize].v.as_ref())
    }

    #[allow(non_snake_case)]
    pub fn getOrNextLower(&self, key: &K) -> Option<&V> {
        self.get_or_next_lower(key)
    }

    pub fn has(&self, key: &K) -> bool {
        find(
            &self.arena,
            self.root,
            key,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        )
        .is_some()
    }

    pub fn delete(&mut self, key: &K) -> Option<V> {
        let node = find(
            &self.arena,
            self.root,
            key,
            |n| &n.k,
            |a, b| (self.comparator)(a, b),
        )?;

        self.root = remove(&mut self.arena, self.root, node);
        self.size -= 1;
        self.arena[node as usize].v.take()
    }

    pub fn max(&self) -> Option<&V> {
        last(&self.arena, self.root).and_then(|idx| self.arena[idx as usize].v.as_ref())
    }

    pub fn iterator<'a>(&'a self) -> impl FnMut() -> Option<&'a V> + 'a {
        let mut curr = first(&self.arena, self.root);
        move || {
            let out = curr.and_then(|idx| self.arena[idx as usize].v.as_ref());
            if let Some(idx) = curr {
                curr = next(&self.arena, idx);
            }
            out
        }
    }

    fn to_string_node(&self, node: u32, tab: &str, side: &str) -> String
    where
        K: std::fmt::Display,
    {
        let n = &self.arena[node as usize];
        let mut s = format!("\n{tab}{side} {} {}", "TreeNode", n.k);
        if let Some(l) = n.l {
            s.push_str(&self.to_string_node(l, &format!("{tab}  "), "←"));
        }
        if let Some(r) = n.r {
            s.push_str(&self.to_string_node(r, &format!("{tab}  "), "→"));
        }
        s
    }

    pub fn to_string(&self, tab: &str) -> String
    where
        K: std::fmt::Display,
    {
        match self.root {
            Some(root) => format!("Tree{}", self.to_string_node(root, tab, "└─")),
            None => "Tree ∅".to_string(),
        }
    }
}
