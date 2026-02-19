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
//! [`util`] | `util/index.ts` | `first`, `next`, `remove` for position tree |
//! [`util2`] | `util2.ts` | `insert2`, `remove2`, `next2` … for ID tree |

pub mod splay;
pub mod types;
pub mod util;
pub mod util2;

pub use splay::util2::splay2;
pub use splay::{l_splay, ll_splay, lr_splay, r_splay, rl_splay, rr_splay, splay};
pub use types::{Node, Node2};
pub use util::{first, next, remove};
pub use util2::{first2, insert2, last2, next2, prev2, remove2};
