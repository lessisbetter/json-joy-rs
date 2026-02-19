use crate::data_types::{MapTreeOps, SonicMap};

use super::types::RbNode;
use super::util;

pub struct RbOps;

impl<K, V> MapTreeOps<K, V, RbNode<K, V>> for RbOps {
    fn insert<C: Fn(&K, &K) -> i32>(
        arena: &mut Vec<RbNode<K, V>>,
        root: Option<u32>,
        node: u32,
        comparator: &C,
    ) -> Option<u32> {
        util::insert(arena, root, node, comparator)
    }

    fn insert_left(
        arena: &mut Vec<RbNode<K, V>>,
        root: Option<u32>,
        node: u32,
        parent: u32,
    ) -> Option<u32> {
        util::insert_left(arena, root, node, parent)
    }

    fn insert_right(
        arena: &mut Vec<RbNode<K, V>>,
        root: Option<u32>,
        node: u32,
        parent: u32,
    ) -> Option<u32> {
        util::insert_right(arena, root, node, parent)
    }

    fn remove(arena: &mut Vec<RbNode<K, V>>, root: Option<u32>, node: u32) -> Option<u32> {
        util::remove(arena, root, node)
    }
}

fn new_node<K, V>(k: K, v: V) -> RbNode<K, V> {
    RbNode::new(k, v)
}

fn default_comparator<K: PartialOrd>(a: &K, b: &K) -> i32 {
    if a == b {
        0
    } else if a < b {
        -1
    } else {
        1
    }
}

/// High-performance red-black tree map.
///
/// Mirrors upstream `red-black/RbMap.ts`.
pub struct RbMap<K, V, C = fn(&K, &K) -> i32>
where
    C: Fn(&K, &K) -> i32,
{
    inner: SonicMap<K, V, RbNode<K, V>, RbOps, C, fn(K, V) -> RbNode<K, V>>,
}

impl<K, V> RbMap<K, V, fn(&K, &K) -> i32>
where
    K: PartialOrd,
{
    pub fn new() -> Self {
        Self::with_comparator(default_comparator::<K>)
    }
}

impl<K, V> Default for RbMap<K, V, fn(&K, &K) -> i32>
where
    K: PartialOrd,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, C> RbMap<K, V, C>
where
    C: Fn(&K, &K) -> i32,
{
    pub fn with_comparator(comparator: C) -> Self {
        Self {
            inner: SonicMap::with(comparator, new_node::<K, V>),
        }
    }

    pub fn set(&mut self, key: K, value: V) -> u32 {
        self.inner.set(key, value)
    }

    pub fn find(&self, key: &K) -> Option<u32> {
        self.inner.find(key)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.inner.get_mut(key)
    }

    pub fn del(&mut self, key: &K) -> bool {
        self.inner.del(key)
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn has(&self, key: &K) -> bool {
        self.inner.has(key)
    }

    pub fn size(&self) -> usize {
        self.inner.size()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get_or_next_lower(&self, key: &K) -> Option<u32> {
        self.inner.get_or_next_lower(key)
    }

    pub fn first(&self) -> Option<u32> {
        self.inner.first()
    }

    pub fn last(&self) -> Option<u32> {
        self.inner.last()
    }

    pub fn next(&self, curr: u32) -> Option<u32> {
        self.inner.next(curr)
    }

    pub fn iterator0(&self) -> impl FnMut() -> Option<u32> + '_ {
        self.inner.iterator0()
    }

    pub fn iterator(&self) -> impl Iterator<Item = u32> + '_ {
        self.inner.iterator()
    }

    pub fn entries(&self) -> impl Iterator<Item = u32> + '_ {
        self.inner.entries()
    }

    pub fn for_each<G: FnMut(u32, &RbNode<K, V>)>(&self, f: G) {
        self.inner.for_each(f)
    }

    pub fn root_index(&self) -> Option<u32> {
        self.inner.root_index()
    }

    pub fn key(&self, idx: u32) -> &K {
        self.inner.key(idx)
    }

    pub fn value(&self, idx: u32) -> &V {
        self.inner.value(idx)
    }

    pub fn value_mut_by_index(&mut self, idx: u32) -> &mut V {
        self.inner.value_mut_by_index(idx)
    }

    pub fn assert_valid(&self) -> Result<(), String> {
        util::assert_red_black_tree(
            self.inner.arena(),
            self.inner.root_index(),
            self.inner.comparator(),
        )
    }
}
