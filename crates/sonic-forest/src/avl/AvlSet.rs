use super::avl_map::AvlMap;

fn default_comparator<V: PartialOrd>(a: &V, b: &V) -> i32 {
    if a == b {
        0
    } else if a < b {
        -1
    } else {
        1
    }
}

/// AVL tree set backed by [`AvlMap<V, ()>`].
///
/// Rust divergence: exposes stable arena indices for entry traversal.
pub struct AvlSet<V, C = fn(&V, &V) -> i32>
where
    C: Fn(&V, &V) -> i32,
{
    inner: AvlMap<V, (), C>,
}

impl<V> AvlSet<V, fn(&V, &V) -> i32>
where
    V: PartialOrd,
{
    pub fn new() -> Self {
        Self::with_comparator(default_comparator::<V>)
    }
}

impl<V> Default for AvlSet<V, fn(&V, &V) -> i32>
where
    V: PartialOrd,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<V, C> AvlSet<V, C>
where
    C: Fn(&V, &V) -> i32,
{
    pub fn with_comparator(comparator: C) -> Self {
        Self {
            inner: AvlMap::with_comparator(comparator),
        }
    }

    pub fn add(&mut self, value: V) -> u32 {
        self.inner.set(value, ())
    }

    pub fn del(&mut self, value: &V) -> bool {
        self.inner.del(value)
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn has(&self, value: &V) -> bool {
        self.inner.has(value)
    }

    pub fn size(&self) -> usize {
        self.inner.size()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get_or_next_lower(&self, value: &V) -> Option<u32> {
        self.inner.get_or_next_lower(value)
    }

    pub fn for_each<G: FnMut(u32, &V)>(&self, mut f: G) {
        self.inner.for_each(|i, n| f(i, &n.k));
    }

    pub fn first(&self) -> Option<u32> {
        self.inner.first()
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

    pub fn key(&self, idx: u32) -> &V {
        self.inner.key(idx)
    }

    pub fn assert_valid(&self) -> Result<(), String> {
        self.inner.assert_valid()
    }
}
