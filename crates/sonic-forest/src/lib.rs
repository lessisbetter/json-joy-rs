//! Rust port of [sonic-forest](https://github.com/streamich/sonic-forest).
//!
//! Provides arena-based splay-tree utilities for dual-tree data structures.
//! Used by the `json-joy` RGA (Replicated Growable Array), which keeps
//! chunks in two concurrent splay trees:
//!
//! - **Position tree** (`p` / `l` / `r`) — ordered by content position,
//!   supports O(log n) amortised `findChunk(position)`.
//! - **ID tree** (`p2` / `l2` / `r2`) — ordered by `(sid, time)`,
//!   supports O(log n) amortised `findById(ts)`.
//!
//! Instead of raw pointers (as in the TypeScript original), all "pointers"
//! are `Option<u32>` indices into a caller-owned `Vec<N>` arena.
//!
//! # Module layout
//!
//! | Module | Upstream file | Contents |
//! |--------|---------------|----------|
//! [`types`] | `types.ts` / `types2.ts` | [`Node`] and [`Node2`] traits |
//! [`splay`] | `splay/util.ts` | Position-tree splay rotations |
//! [`splay::util2`] | `splay/util2.ts` | ID-tree splay (`splay2`) |
//! [`util`] | `util/*` | Position-tree traversal, search, insert/remove/swap helpers |
//! [`util2`] | `util2.ts` | `insert2`, `remove2`, `next2` … for ID tree |

#[path = "avl/mod.rs"]
pub mod avl;
#[path = "data-types/mod.rs"]
pub mod data_types;
#[path = "llrb-tree/mod.rs"]
pub mod llrb_tree;
#[path = "print/mod.rs"]
pub mod print;
#[path = "radix/mod.rs"]
pub mod radix;
#[path = "red-black/mod.rs"]
pub mod red_black;
pub mod splay;
#[path = "trie/mod.rs"]
pub mod trie;
pub mod types;
pub mod util;
pub mod util2;

pub use avl::{AvlBstNumNumMap, AvlMap, AvlMapOld, AvlSet};
pub use llrb_tree::{LlrbNode, LlrbTree};
pub use print::{printBinary, printTree, print_binary, print_tree, PrintChild, Printable};
pub use radix::{BinaryRadixTree, BinaryTrieNode, RadixTree, Slice as RadixSlice};
pub use red_black::RbMap;
pub use splay::util2::splay2;
pub use splay::{l_splay, ll_splay, lr_splay, r_splay, rl_splay, rr_splay, splay};
pub use trie::TrieNode;
pub use types::{Node, Node2};
pub use util::{
    find, find_or_next_lower, first, insert, insert_left, insert_right, last, next, prev, remove,
    size, swap,
};
pub use util2::{first2, insert2, last2, next2, prev2, remove2};
