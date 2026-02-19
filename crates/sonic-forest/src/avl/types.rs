use crate::types::{KvNode, Node};

/// Mirrors upstream `IAvlTreeNode<K, V>`.
#[derive(Clone, Debug)]
pub struct AvlNode<K, V> {
    pub p: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    pub k: K,
    pub v: V,
    /// Balance factor, `height(left) - height(right)`.
    pub bf: i32,
}

impl<K, V> AvlNode<K, V> {
    pub fn new(k: K, v: V) -> Self {
        Self {
            p: None,
            l: None,
            r: None,
            k,
            v,
            bf: 0,
        }
    }
}

impl<K, V> Node for AvlNode<K, V> {
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

impl<K, V> KvNode<K, V> for AvlNode<K, V> {
    fn key(&self) -> &K {
        &self.k
    }

    fn value(&self) -> &V {
        &self.v
    }

    fn value_mut(&mut self) -> &mut V {
        &mut self.v
    }

    fn set_key(&mut self, key: K) {
        self.k = key;
    }

    fn set_value(&mut self, value: V) {
        self.v = value;
    }
}

/// AVL-specific node behavior.
pub trait AvlNodeLike<K, V>: KvNode<K, V> {
    fn bf(&self) -> i32;
    fn set_bf(&mut self, bf: i32);
}

impl<K, V> AvlNodeLike<K, V> for AvlNode<K, V> {
    fn bf(&self) -> i32 {
        self.bf
    }

    fn set_bf(&mut self, bf: i32) {
        self.bf = bf;
    }
}
