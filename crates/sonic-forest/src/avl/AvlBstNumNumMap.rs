use super::avl_map::AvlMap;

fn comparator(a: &f64, b: &f64) -> i32 {
    if a == b {
        0
    } else if a < b {
        -1
    } else {
        1
    }
}

/// Numeric-specialized AVL map mirroring upstream `AvlBstNumNumMap`.
pub struct AvlBstNumNumMap {
    inner: AvlMap<f64, f64, fn(&f64, &f64) -> i32>,
}

impl AvlBstNumNumMap {
    pub fn new() -> Self {
        Self {
            inner: AvlMap::with_comparator(comparator),
        }
    }

    pub fn insert(&mut self, k: f64, v: f64) -> u32 {
        self.inner.set(k, v)
    }

    pub fn set(&mut self, k: f64, v: f64) -> u32 {
        self.inner.set(k, v)
    }

    pub fn find(&self, k: f64) -> Option<u32> {
        self.inner.find(&k)
    }

    pub fn get(&self, k: f64) -> Option<f64> {
        self.inner.get(&k).copied()
    }

    pub fn has(&self, k: f64) -> bool {
        self.inner.has(&k)
    }

    pub fn get_or_next_lower(&self, k: f64) -> Option<u32> {
        self.inner.get_or_next_lower(&k)
    }

    pub fn for_each<G: FnMut(f64, f64)>(&self, mut f: G) {
        self.inner.for_each(|_i, n| f(n.k, n.v));
    }

    pub fn assert_valid(&self) -> Result<(), String> {
        self.inner.assert_valid()
    }
}

impl Default for AvlBstNumNumMap {
    fn default() -> Self {
        Self::new()
    }
}
