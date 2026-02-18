//! Slice types for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/`.

pub mod constants;
pub mod types;
pub mod slice;
pub mod slices;

pub use constants::SliceStacking;
pub use types::{SliceType, TypeTag};
pub use slice::Slice;
pub use slices::Slices;
