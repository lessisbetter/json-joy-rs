use std::marker::PhantomData;

use crate::types::KvNode;
use crate::util::{find_or_next_lower, first, last, next, prev};

/// Tree operation callbacks required by [`SonicMap`].
///
/// Mirrors the callback contract in upstream `data-types/map.ts`.
pub trait MapTreeOps<K, V, N>
where
    N: KvNode<K, V>,
{
    fn insert<C: Fn(&K, &K) -> i32>(
        arena: &mut Vec<N>,
        root: Option<u32>,
        node: u32,
        comparator: &C,
    ) -> Option<u32>;

    fn insert_left(arena: &mut Vec<N>, root: Option<u32>, node: u32, parent: u32) -> Option<u32>;

    fn insert_right(arena: &mut Vec<N>, root: Option<u32>, node: u32, parent: u32) -> Option<u32>;

    fn remove(arena: &mut Vec<N>, root: Option<u32>, node: u32) -> Option<u32>;
}

/// Arena-backed sorted map core.
///
/// Rust divergence note: upstream returns mutable node object references.
/// Rust returns stable arena indices (`u32`) and exposes accessors.
pub struct SonicMap<K, V, N, O, C, F>
where
    N: KvNode<K, V>,
    O: MapTreeOps<K, V, N>,
    C: Fn(&K, &K) -> i32,
    F: Fn(K, V) -> N,
{
    arena: Vec<N>,
    root: Option<u32>,
    min: Option<u32>,
    max: Option<u32>,
    comparator: C,
    new_node: F,
    len: usize,
    _kv: PhantomData<(K, V)>,
    _ops: PhantomData<O>,
}

impl<K, V, N, O, C, F> SonicMap<K, V, N, O, C, F>
where
    N: KvNode<K, V>,
    O: MapTreeOps<K, V, N>,
    C: Fn(&K, &K) -> i32,
    F: Fn(K, V) -> N,
{
    pub fn with(comparator: C, new_node: F) -> Self {
        Self {
            arena: Vec::new(),
            root: None,
            min: None,
            max: None,
            comparator,
            new_node,
            len: 0,
            _kv: PhantomData,
            _ops: PhantomData,
        }
    }

    pub fn root_index(&self) -> Option<u32> {
        self.root
    }

    pub fn arena(&self) -> &[N] {
        &self.arena
    }

    pub fn comparator(&self) -> &C {
        &self.comparator
    }

    pub fn min_index(&self) -> Option<u32> {
        self.min
    }

    pub fn max_index(&self) -> Option<u32> {
        self.max
    }

    pub fn node(&self, idx: u32) -> &N {
        &self.arena[idx as usize]
    }

    pub fn node_mut(&mut self, idx: u32) -> &mut N {
        &mut self.arena[idx as usize]
    }

    pub fn key(&self, idx: u32) -> &K {
        self.node(idx).key()
    }

    pub fn value(&self, idx: u32) -> &V {
        self.node(idx).value()
    }

    pub fn value_mut_by_index(&mut self, idx: u32) -> &mut V {
        self.node_mut(idx).value_mut()
    }

    pub fn set(&mut self, key: K, value: V) -> u32 {
        if self.root.is_none() {
            self.arena.push((self.new_node)(key, value));
            let idx = (self.arena.len() - 1) as u32;
            self.root = O::insert(&mut self.arena, None, idx, &self.comparator);
            self.min = self.root;
            self.max = self.root;
            self.len = 1;
            return idx;
        }

        let root = self.root.expect("root exists");

        let max = self.max.expect("max exists");
        let max_cmp = (self.comparator)(&key, self.arena[max as usize].key());
        if max_cmp == 0 {
            self.arena[max as usize].set_value(value);
            return max;
        }
        if max_cmp > 0 {
            self.arena.push((self.new_node)(key, value));
            let idx = (self.arena.len() - 1) as u32;
            self.root = O::insert_right(&mut self.arena, Some(root), idx, max);
            self.max = Some(idx);
            self.len += 1;
            return idx;
        }

        let min = self.min.expect("min exists");
        let min_cmp = (self.comparator)(&key, self.arena[min as usize].key());
        if min_cmp == 0 {
            self.arena[min as usize].set_value(value);
            return min;
        }
        if min_cmp < 0 {
            self.arena.push((self.new_node)(key, value));
            let idx = (self.arena.len() - 1) as u32;
            self.root = O::insert_left(&mut self.arena, Some(root), idx, min);
            self.min = Some(idx);
            self.len += 1;
            return idx;
        }

        let mut curr = root;
        loop {
            let cmp = (self.comparator)(&key, self.arena[curr as usize].key());
            if cmp == 0 {
                self.arena[curr as usize].set_value(value);
                return curr;
            } else if cmp > 0 {
                let right = self.arena[curr as usize].r();
                if let Some(next) = right {
                    curr = next;
                } else {
                    self.arena.push((self.new_node)(key, value));
                    let idx = (self.arena.len() - 1) as u32;
                    self.root = O::insert_right(&mut self.arena, self.root, idx, curr);
                    self.len += 1;
                    return idx;
                }
            } else {
                let left = self.arena[curr as usize].l();
                if let Some(next) = left {
                    curr = next;
                } else {
                    self.arena.push((self.new_node)(key, value));
                    let idx = (self.arena.len() - 1) as u32;
                    self.root = O::insert_left(&mut self.arena, self.root, idx, curr);
                    self.len += 1;
                    return idx;
                }
            }
        }
    }

    pub fn find(&self, key: &K) -> Option<u32> {
        let cmp = &self.comparator;
        let mut curr = self.root;
        while let Some(i) = curr {
            let c = cmp(key, self.arena[i as usize].key());
            if c == 0 {
                return Some(i);
            }
            curr = if c < 0 {
                self.arena[i as usize].l()
            } else {
                self.arena[i as usize].r()
            };
        }
        None
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.find(key).map(|i| self.arena[i as usize].value())
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = self.find(key)?;
        Some(self.arena[idx as usize].value_mut())
    }

    pub fn del(&mut self, key: &K) -> bool {
        let node = match self.find(key) {
            Some(node) => node,
            None => return false,
        };

        if self.max == Some(node) {
            self.max = prev(&self.arena, node);
        }
        if self.min == Some(node) {
            self.min = next(&self.arena, node);
        }

        self.root = O::remove(&mut self.arena, self.root, node);
        if self.len > 0 {
            self.len -= 1;
        }

        if self.root.is_none() {
            self.min = None;
            self.max = None;
            self.len = 0;
        } else {
            if self.min.is_none() {
                self.min = first(&self.arena, self.root);
            }
            if self.max.is_none() {
                self.max = last(&self.arena, self.root);
            }
        }

        true
    }

    pub fn clear(&mut self) {
        self.arena.clear();
        self.root = None;
        self.min = None;
        self.max = None;
        self.len = 0;
    }

    pub fn has(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    pub fn size(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.min.is_none()
    }

    pub fn get_or_next_lower(&self, key: &K) -> Option<u32> {
        find_or_next_lower(
            &self.arena,
            self.root,
            key,
            |n| n.key(),
            |a, b| (self.comparator)(a, b),
        )
    }

    pub fn first(&self) -> Option<u32> {
        self.min
    }

    pub fn last(&self) -> Option<u32> {
        self.max
    }

    pub fn next(&self, curr: u32) -> Option<u32> {
        next(&self.arena, curr)
    }

    pub fn iterator0(&self) -> impl FnMut() -> Option<u32> + '_ {
        let mut curr = self.first();
        move || {
            let out = curr;
            if let Some(i) = curr {
                curr = self.next(i);
            }
            out
        }
    }

    /// Rust parity alias for upstream `iterator()`.
    pub fn iterator(&self) -> SonicMapIndexIter<'_, K, V, N, O, C, F> {
        self.iter_indices()
    }

    /// Rust parity alias for upstream `entries()`.
    pub fn entries(&self) -> SonicMapIndexIter<'_, K, V, N, O, C, F> {
        self.iterator()
    }

    pub fn iter_indices(&self) -> SonicMapIndexIter<'_, K, V, N, O, C, F> {
        SonicMapIndexIter {
            map: self,
            curr: self.first(),
            _kv: PhantomData,
        }
    }

    pub fn for_each<G: FnMut(u32, &N)>(&self, mut f: G) {
        let mut curr = self.first();
        while let Some(i) = curr {
            f(i, &self.arena[i as usize]);
            curr = self.next(i);
        }
    }
}

pub struct SonicMapIndexIter<'a, K, V, N, O, C, F>
where
    N: KvNode<K, V>,
    O: MapTreeOps<K, V, N>,
    C: Fn(&K, &K) -> i32,
    F: Fn(K, V) -> N,
{
    map: &'a SonicMap<K, V, N, O, C, F>,
    curr: Option<u32>,
    _kv: PhantomData<(K, V)>,
}

impl<'a, K, V, N, O, C, F> Iterator for SonicMapIndexIter<'a, K, V, N, O, C, F>
where
    N: KvNode<K, V>,
    O: MapTreeOps<K, V, N>,
    C: Fn(&K, &K) -> i32,
    F: Fn(K, V) -> N,
{
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let out = self.curr;
        if let Some(i) = self.curr {
            self.curr = self.map.next(i);
        }
        out
    }
}
