use crate::types::Node;

/// Mirrors upstream `TreeNode.ts` shape.
#[derive(Clone, Debug)]
pub struct TreeNode<K, V> {
    pub p: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    pub k: K,
    // Rust divergence: value is wrapped in Option to allow by-value deletes
    // in an arena-backed representation without moving nodes out of the arena.
    pub v: Option<V>,
}

impl<K, V> TreeNode<K, V> {
    pub fn new(k: K, v: V) -> Self {
        Self {
            p: None,
            l: None,
            r: None,
            k,
            v: Some(v),
        }
    }
}

impl<K, V> Node for TreeNode<K, V> {
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
