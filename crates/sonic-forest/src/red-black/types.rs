use crate::types::{KvNode, Node};

/// Mirrors upstream `IRbTreeNode<K, V>`.
#[derive(Clone, Debug)]
pub struct RbNode<K, V> {
    pub p: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    pub k: K,
    pub v: V,
    /// Node color: `true` = black, `false` = red.
    pub b: bool,
}

impl<K, V> RbNode<K, V> {
    pub fn new(k: K, v: V) -> Self {
        Self {
            p: None,
            l: None,
            r: None,
            k,
            v,
            b: false,
        }
    }
}

impl<K, V> Node for RbNode<K, V> {
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

impl<K, V> KvNode<K, V> for RbNode<K, V> {
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

/// Red-black specific node behavior.
pub trait RbNodeLike<K, V>: KvNode<K, V> {
    fn is_black(&self) -> bool;
    fn set_black(&mut self, black: bool);
}

impl<K, V> RbNodeLike<K, V> for RbNode<K, V> {
    fn is_black(&self) -> bool {
        self.b
    }

    fn set_black(&mut self, black: bool) {
        self.b = black;
    }
}
