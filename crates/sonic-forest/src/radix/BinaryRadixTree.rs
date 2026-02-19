use std::collections::BTreeMap;

use crate::print::Printable;
use crate::util::{first, next};

use super::binary_radix::{find, insert, print, remove, to_record};
use super::binary_trie_node::BinaryTrieNode;
use super::slice::Slice;

/// Mirrors upstream `radix/BinaryRadixTree.ts` public API.
pub struct BinaryRadixTree<V = ()> {
    pub size: usize,
    pub(crate) nodes: Vec<BinaryTrieNode<V>>,
    pub(crate) root: u32,
}

impl<V> BinaryRadixTree<V> {
    pub fn new() -> Self {
        let nodes = vec![BinaryTrieNode::new(
            Slice::from_uint8_array(Vec::new()),
            None,
        )];
        Self {
            size: 0,
            nodes,
            root: 0,
        }
    }

    pub(crate) fn push_node(&mut self, k: Slice, v: Option<V>) -> u32 {
        self.nodes.push(BinaryTrieNode::new(k, v));
        (self.nodes.len() - 1) as u32
    }

    pub fn set<K: AsRef<[u8]>>(&mut self, key: K, value: V) {
        self.size += insert(self, key.as_ref(), value);
    }

    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<&V> {
        find(self, key.as_ref()).and_then(|idx| self.nodes[idx as usize].v.as_ref())
    }

    pub fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> bool {
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

    pub fn node(&self, idx: u32) -> &BinaryTrieNode<V> {
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

    pub fn for_children<F: FnMut(&BinaryTrieNode<V>, usize)>(&self, mut callback: F) {
        let mut child = first(&self.nodes, self.nodes[self.root as usize].children);
        let mut i = 0;
        while let Some(idx) = child {
            callback(&self.nodes[idx as usize], i);
            i += 1;
            child = next(&self.nodes, idx);
        }
    }
}

impl<V> Default for BinaryRadixTree<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Printable for BinaryRadixTree<V> {
    fn to_string_with_tab(&self, tab: Option<&str>) -> String {
        print(self, self.root, tab.unwrap_or(""))
    }
}
