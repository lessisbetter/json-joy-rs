use std::fmt::Debug;

use crate::types::KvNode;

use super::super::types::RbNodeLike;

/// Debug printer for red-black trees.
///
/// Mirrors upstream `red-black/util/print.ts` output intent.
pub fn print<K, V, N>(arena: &[N], node: Option<u32>, tab: &str) -> String
where
    K: Debug,
    V: Debug,
    N: RbNodeLike<K, V> + KvNode<K, V>,
{
    match node {
        None => "∅".to_string(),
        Some(i) => {
            let n = &arena[i as usize];
            let color = if n.is_black() { "black" } else { "red" };
            let left = print::<K, V, N>(arena, n.l(), &format!("{tab}  "));
            let right = print::<K, V, N>(arena, n.r(), &format!("{tab}  "));
            format!(
                "Node[{i}] {color} {{ {:?} = {:?} }}\n{tab}L={left}\n{tab}R={right}",
                n.key(),
                n.value()
            )
        }
    }
}
