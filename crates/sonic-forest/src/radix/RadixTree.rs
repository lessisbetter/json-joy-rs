use std::collections::BTreeMap;

use crate::print::Printable;
use crate::trie::TrieNode;
use crate::util::{first, next};

use super::radix_impl::{find, insert, print, remove, to_record};

/// Mirrors upstream `radix/RadixTree.ts` public API.
pub struct RadixTree<V = ()> {
    pub size: usize,
    pub(crate) nodes: Vec<TrieNode<V>>,
    pub(crate) root: u32,
}

impl<V> RadixTree<V> {
    pub fn new() -> Self {
        let nodes = vec![TrieNode::new(String::new(), None)];
        Self {
            size: 0,
            nodes,
            root: 0,
        }
    }

    pub(crate) fn push_node(&mut self, k: String, v: Option<V>) -> u32 {
        self.nodes.push(TrieNode::new(k, v));
        (self.nodes.len() - 1) as u32
    }

    pub fn set<K: AsRef<str>>(&mut self, key: K, value: V) {
        self.size += insert(self, key.as_ref(), value);
    }

    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<&V> {
        find(self, key.as_ref()).and_then(|idx| self.nodes[idx as usize].v.as_ref())
    }

    pub fn delete<K: AsRef<str>>(&mut self, key: K) -> bool {
        let removed = remove(self, key.as_ref());
        if removed {
            self.size -= 1;
        }
        removed
    }

    pub fn to_record(&self) -> BTreeMap<String, V>
    where
        V: Clone,
    {
        to_record(self)
    }

    pub fn root_index(&self) -> u32 {
        self.root
    }

    pub fn node(&self, idx: u32) -> &TrieNode<V> {
        &self.nodes[idx as usize]
    }

    pub fn children_in_order(&self, parent: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut child = first(&self.nodes, self.nodes[parent as usize].children);
        while let Some(idx) = child {
            out.push(idx);
            child = next(&self.nodes, idx);
        }
        out
    }

    pub fn for_children<F: FnMut(&TrieNode<V>, usize)>(&self, mut callback: F) {
        let mut child = first(&self.nodes, self.nodes[self.root as usize].children);
        let mut i = 0;
        while let Some(idx) = child {
            // Mirrors upstream `TrieNode.forChildren` callback index behavior.
            callback(&self.nodes[idx as usize], 0);
            i += 1;
            child = next(&self.nodes, idx);
        }
        let _ = i;
    }
}

impl<V> Default for RadixTree<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Printable for RadixTree<V> {
    fn to_string_with_tab(&self, tab: Option<&str>) -> String {
        print(self, self.root, tab.unwrap_or(""))
    }
}
