//! Mirrors upstream `radix/*` family (incremental port).

pub mod index;
#[path = "Slice.rs"]
pub mod slice;

pub use slice::Slice;

// Pending upstream files in this family (ported in later slices):
// - BinaryRadixTree.ts
// - BinaryTrieNode.ts
// - RadixTree.ts
// - binaryRadix.ts
// - radix.ts
