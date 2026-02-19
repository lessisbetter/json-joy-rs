//! Node trait definitions.
//!
//! Mirrors:
//! - `src/types.ts`  → [`Node`]  (position tree: p / l / r)
//! - `src/types2.ts` → [`Node2`] (ID tree:       p2 / l2 / r2)
//!
//! In the TypeScript original, nodes are JS objects with pointer fields.
//! In Rust, each "pointer" is an `Option<u32>` index into a [`Vec`]-backed
//! arena.  All tree-manipulation functions take the arena as `&mut Vec<N>`
//! and work with indices.

/// Position-tree links (`p`, `l`, `r`).
///
/// Mirrors `HeadlessNode` in `types.ts`.
pub trait Node {
    fn p(&self) -> Option<u32>;
    fn l(&self) -> Option<u32>;
    fn r(&self) -> Option<u32>;
    fn set_p(&mut self, v: Option<u32>);
    fn set_l(&mut self, v: Option<u32>);
    fn set_r(&mut self, v: Option<u32>);
}

/// ID-tree links (`p2`, `l2`, `r2`).
///
/// Mirrors `HeadlessNode2` in `types2.ts`.
pub trait Node2 {
    fn p2(&self) -> Option<u32>;
    fn l2(&self) -> Option<u32>;
    fn r2(&self) -> Option<u32>;
    fn set_p2(&mut self, v: Option<u32>);
    fn set_l2(&mut self, v: Option<u32>);
    fn set_r2(&mut self, v: Option<u32>);
}

/// Comparator used by map/tree structures.
///
/// Mirrors `Comparator<T>` in upstream `types.ts`.
pub type Comparator<K> = dyn Fn(&K, &K) -> i32;

/// Key/value node interface used by map-like structures.
///
/// Rust divergence: upstream accesses node fields directly (`k`, `v`), while
/// Rust uses trait methods to support arena-indexed generic nodes.
pub trait KvNode<K, V>: Node {
    fn key(&self) -> &K;
    fn value(&self) -> &V;
    fn value_mut(&mut self) -> &mut V;
    fn set_key(&mut self, key: K);
    fn set_value(&mut self, value: V);
}
