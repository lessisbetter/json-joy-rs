use super::avl_map::AvlMap;

/// Compatibility alias for upstream `AvlMapOld`.
///
/// Rust divergence: `AvlMapOld` forwards to the same implementation as `AvlMap`
/// because node-stable arena indexing already avoids the historical issues the
/// upstream old/new split addressed.
pub type AvlMapOld<K, V, C = fn(&K, &K) -> i32> = AvlMap<K, V, C>;
