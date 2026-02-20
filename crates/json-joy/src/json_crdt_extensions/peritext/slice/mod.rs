//! Slice types for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/`.

pub mod constants;
#[allow(clippy::module_inception)]
pub mod slice;
pub mod slices;
pub mod types;

pub use constants::SliceStacking;
pub use slice::Slice;
pub use slices::Slices;
pub use types::{SliceType, TypeTag};
