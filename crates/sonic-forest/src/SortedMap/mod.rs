//! Mirrors upstream `SortedMap/*` family.

pub mod constants;
pub mod index;
#[path = "SortedMap.rs"]
#[allow(clippy::module_inception)]
pub mod sorted_map;
#[path = "SortedMapIterator.rs"]
pub mod sorted_map_iterator;
#[path = "SortedMapNode.rs"]
pub mod sorted_map_node;
pub mod util;

pub use constants::IteratorType;
pub use index::*;
pub use sorted_map::SortedMap;
pub use sorted_map_iterator::OrderedMapIterator;
pub use sorted_map_node::{SortedMapNode, SortedMapNodeEnableIndex};
