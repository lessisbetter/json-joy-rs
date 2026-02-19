//! Structural codecs — encode the full CRDT document as a self-contained snapshot.
//!
//! Mirrors `packages/json-joy/src/json-crdt/codec/structural/`.

pub mod binary;
pub mod compact;
pub mod compact_binary;
pub mod verbose;
