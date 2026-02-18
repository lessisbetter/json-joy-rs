//! RGA position types for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/rga/`.

pub mod constants;
pub mod point;
pub mod range;

pub use constants::Anchor;
pub use point::Point;
pub use range::Range;
