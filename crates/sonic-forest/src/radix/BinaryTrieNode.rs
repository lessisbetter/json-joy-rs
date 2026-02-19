use crate::types::Node;

use super::slice::Slice;

/// Mirrors upstream `radix/BinaryTrieNode.ts` node shape.
#[derive(Clone, Debug)]
pub struct BinaryTrieNode<V> {
    pub p: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    pub children: Option<u32>,
    pub k: Slice,
    pub v: Option<V>,
}

impl<V> BinaryTrieNode<V> {
    pub fn new(k: Slice, v: Option<V>) -> Self {
        Self {
            p: None,
            l: None,
            r: None,
            children: None,
            k,
            v,
        }
    }
}

impl<V> Node for BinaryTrieNode<V> {
    fn p(&self) -> Option<u32> {
        self.p
    }

    fn l(&self) -> Option<u32> {
        self.l
    }

    fn r(&self) -> Option<u32> {
        self.r
    }

    fn set_p(&mut self, v: Option<u32>) {
        self.p = v;
    }

    fn set_l(&mut self, v: Option<u32>) {
        self.l = v;
    }

    fn set_r(&mut self, v: Option<u32>) {
        self.r = v;
    }
}
