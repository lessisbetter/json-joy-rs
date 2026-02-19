use core::ptr::NonNull;

/// Mirrors upstream `SortedMap/SortedMapNode.ts` exports.
///
/// Rust divergence:
/// - Uses raw links (`NonNull`) to support upstream-style node-local rotation
///   methods (`rRotate`, `lRotate`) that mutate parent/child links directly.
/// - The rest of the crate may use arena-indexed links for map implementations;
///   this type stays pointer-based to mirror upstream `TreeNode` behavior.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct SortedMapNode<K, V> {
    pub l: Option<NonNull<SortedMapNode<K, V>>>,
    pub r: Option<NonNull<SortedMapNode<K, V>>>,
    pub p: Option<NonNull<SortedMapNode<K, V>>>,
    pub k: K,
    pub v: V,
    pub b: bool,
}

impl<K, V> SortedMapNode<K, V> {
    pub fn new(k: K, v: V, b: bool) -> Self {
        Self {
            l: None,
            r: None,
            p: None,
            k,
            v,
            b,
        }
    }

    /// In-order predecessor. Mirrors `TreeNode.prev()` in upstream TS.
    pub fn prev(&self) -> NonNull<Self> {
        // SAFETY: Caller-maintained links must form a valid RB tree/header graph.
        unsafe {
            let mut prev = NonNull::from(self);
            let parent = prev
                .as_ref()
                .p
                .expect("SortedMapNode.prev requires parent link");
            let is_root_or_header = parent.as_ref().p == Some(prev);
            if is_root_or_header && !prev.as_ref().b {
                prev = prev
                    .as_ref()
                    .r
                    .expect("header/root predecessor requires right link");
            } else if let Some(l) = prev.as_ref().l {
                prev = l;
                while let Some(r) = prev.as_ref().r {
                    prev = r;
                }
            } else {
                if is_root_or_header {
                    return parent;
                }
                let mut v = parent;
                while v.as_ref().l == Some(prev) {
                    prev = v;
                    v = prev
                        .as_ref()
                        .p
                        .expect("ancestor chain must terminate at header");
                }
                prev = v;
            }
            prev
        }
    }

    /// In-order successor. Mirrors `TreeNode.next()` in upstream TS.
    pub fn next(&self) -> NonNull<Self> {
        // SAFETY: Caller-maintained links must form a valid RB tree/header graph.
        unsafe {
            let mut next = NonNull::from(self);
            if let Some(r) = next.as_ref().r {
                next = r;
                while let Some(l) = next.as_ref().l {
                    next = l;
                }
                return next;
            }

            let mut v = next
                .as_ref()
                .p
                .expect("SortedMapNode.next requires parent link");
            while v.as_ref().r == Some(next) {
                next = v;
                v = next
                    .as_ref()
                    .p
                    .expect("ancestor chain must terminate at header");
            }
            if next.as_ref().r != Some(v) {
                v
            } else {
                next
            }
        }
    }

    /// Rotate left around `self` (upstream name: `rRotate`).
    pub fn r_rotate(&mut self) -> NonNull<Self> {
        // SAFETY: Caller must provide valid parent/child links for rotation.
        unsafe {
            let self_ptr = NonNull::from(&mut *self);
            let mut p = self.p.expect("r_rotate requires node to have parent");
            let mut r = self.r.expect("r_rotate requires right child");
            let l = r.as_ref().l;

            if p.as_ref().p == Some(self_ptr) {
                p.as_mut().p = Some(r);
            } else if p.as_ref().l == Some(self_ptr) {
                p.as_mut().l = Some(r);
            } else {
                p.as_mut().r = Some(r);
            }
            r.as_mut().p = Some(p);
            r.as_mut().l = Some(self_ptr);
            self.p = Some(r);
            self.r = l;
            if let Some(mut l) = l {
                l.as_mut().p = Some(self_ptr);
            }
            r
        }
    }

    /// Rotate right around `self` (upstream name: `lRotate`).
    pub fn l_rotate(&mut self) -> NonNull<Self> {
        // SAFETY: Caller must provide valid parent/child links for rotation.
        unsafe {
            let self_ptr = NonNull::from(&mut *self);
            let mut p = self.p.expect("l_rotate requires node to have parent");
            let mut l = self.l.expect("l_rotate requires left child");
            let r = l.as_ref().r;

            if p.as_ref().p == Some(self_ptr) {
                p.as_mut().p = Some(l);
            } else if p.as_ref().l == Some(self_ptr) {
                p.as_mut().l = Some(l);
            } else {
                p.as_mut().r = Some(l);
            }
            l.as_mut().p = Some(p);
            l.as_mut().r = Some(self_ptr);
            self.p = Some(l);
            self.l = r;
            if let Some(mut r) = r {
                r.as_mut().p = Some(self_ptr);
            }
            l
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SortedMapNodeEnableIndex<K, V> {
    pub base: SortedMapNode<K, V>,
    pub _size: usize,
}

impl<K, V> SortedMapNodeEnableIndex<K, V> {
    pub fn new(k: K, v: V, b: bool) -> Self {
        Self {
            base: SortedMapNode::new(k, v, b),
            _size: 1,
        }
    }

    #[inline]
    fn as_enable(ptr: NonNull<SortedMapNode<K, V>>) -> NonNull<Self> {
        ptr.cast::<Self>()
    }

    pub fn r_rotate(&mut self) -> NonNull<Self> {
        let base_parent = self.base.r_rotate();
        self.compute();
        // SAFETY: When enable-index mode is used, all links point to
        // `SortedMapNodeEnableIndex` nodes whose `base` is the first field.
        unsafe {
            let mut parent = Self::as_enable(base_parent);
            parent.as_mut().compute();
            parent
        }
    }

    pub fn l_rotate(&mut self) -> NonNull<Self> {
        let base_parent = self.base.l_rotate();
        self.compute();
        // SAFETY: Same layout assumption as `r_rotate`.
        unsafe {
            let mut parent = Self::as_enable(base_parent);
            parent.as_mut().compute();
            parent
        }
    }

    pub fn compute(&mut self) {
        self._size = 1;
        // SAFETY: See `as_enable` notes above.
        unsafe {
            if let Some(l) = self.base.l {
                self._size += Self::as_enable(l).as_ref()._size;
            }
            if let Some(r) = self.base.r {
                self._size += Self::as_enable(r).as_ref()._size;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SortedMapNode, SortedMapNodeEnableIndex};
    use core::ptr::NonNull;

    fn ptr_of<T>(b: &mut Box<T>) -> NonNull<T> {
        NonNull::from(b.as_mut())
    }

    #[test]
    fn prev_and_next_follow_in_order_links() {
        let mut header = Box::new(SortedMapNode::new(0, 0, true));
        let mut n1 = Box::new(SortedMapNode::new(1, 1, true));
        let mut n2 = Box::new(SortedMapNode::new(2, 2, true));
        let mut n3 = Box::new(SortedMapNode::new(3, 3, true));

        let header_p = ptr_of(&mut header);
        let n1_p = ptr_of(&mut n1);
        let n2_p = ptr_of(&mut n2);
        let n3_p = ptr_of(&mut n3);

        header.p = Some(n2_p);
        header.l = Some(n1_p);
        header.r = Some(n3_p);

        n2.p = Some(header_p);
        n2.l = Some(n1_p);
        n2.r = Some(n3_p);
        n1.p = Some(n2_p);
        n3.p = Some(n2_p);

        assert_eq!(n2.prev(), n1_p);
        assert_eq!(n2.next(), n3_p);
        assert_eq!(n1.prev(), header_p);
        assert_eq!(n3.next(), header_p);
    }

    #[test]
    fn r_rotate_rewires_parent_and_child_links() {
        let mut header = Box::new(SortedMapNode::new(0, 0, true));
        let mut x = Box::new(SortedMapNode::new(10, 10, true));
        let mut y = Box::new(SortedMapNode::new(20, 20, true));
        let mut b = Box::new(SortedMapNode::new(15, 15, true));

        let header_p = ptr_of(&mut header);
        let x_p = ptr_of(&mut x);
        let y_p = ptr_of(&mut y);
        let b_p = ptr_of(&mut b);

        header.p = Some(x_p);
        x.p = Some(header_p);
        x.r = Some(y_p);
        y.p = Some(x_p);
        y.l = Some(b_p);
        b.p = Some(y_p);

        let parent = x.r_rotate();
        assert_eq!(parent, y_p);
        assert_eq!(header.p, Some(y_p));
        assert_eq!(y.l, Some(x_p));
        assert_eq!(x.p, Some(y_p));
        assert_eq!(x.r, Some(b_p));
        assert_eq!(b.p, Some(x_p));
    }

    #[test]
    fn l_rotate_rewires_parent_and_child_links() {
        let mut header = Box::new(SortedMapNode::new(0, 0, true));
        let mut x = Box::new(SortedMapNode::new(10, 10, true));
        let mut y = Box::new(SortedMapNode::new(5, 5, true));
        let mut b = Box::new(SortedMapNode::new(7, 7, true));

        let header_p = ptr_of(&mut header);
        let x_p = ptr_of(&mut x);
        let y_p = ptr_of(&mut y);
        let b_p = ptr_of(&mut b);

        header.p = Some(x_p);
        x.p = Some(header_p);
        x.l = Some(y_p);
        y.p = Some(x_p);
        y.r = Some(b_p);
        b.p = Some(y_p);

        let parent = x.l_rotate();
        assert_eq!(parent, y_p);
        assert_eq!(header.p, Some(y_p));
        assert_eq!(y.r, Some(x_p));
        assert_eq!(x.p, Some(y_p));
        assert_eq!(x.l, Some(b_p));
        assert_eq!(b.p, Some(x_p));
    }

    #[test]
    fn enable_index_compute_aggregates_child_sizes() {
        let mut root = Box::new(SortedMapNodeEnableIndex::new(2, 2, true));
        let mut left = Box::new(SortedMapNodeEnableIndex::new(1, 1, true));
        let mut right = Box::new(SortedMapNodeEnableIndex::new(3, 3, true));

        let root_p = ptr_of(&mut root);
        let left_p = ptr_of(&mut left);
        let right_p = ptr_of(&mut right);

        root.base.l = Some(left_p.cast());
        root.base.r = Some(right_p.cast());
        left.base.p = Some(root_p.cast());
        right.base.p = Some(root_p.cast());
        left._size = 2;
        right._size = 3;

        root.compute();
        assert_eq!(root._size, 6);
    }
}
