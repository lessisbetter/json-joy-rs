//! JSON CRDT extension types.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/`.
//!
//! Extensions add higher-level semantics on top of the base CRDT node types.
//! Each extension wraps one or more underlying CRDT nodes and exposes a
//! domain-specific API (counters, multi-value registers, rich text, …).

pub mod cnt;
pub mod mval;
pub mod peritext;

/// Numeric IDs for each registered extension.
///
/// Mirrors the upstream `ExtensionId` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExtensionId {
    Mval = 0,
    Cnt = 1,
    Peritext = 2,
    Quill = 3,
    Prosemirror = 4,
    Slate = 5,
}
