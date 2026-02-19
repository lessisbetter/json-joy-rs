//! JSON CRDT document codecs.
//!
//! Mirrors `packages/json-joy/src/json-crdt/codec/`.
//!
//! Three families of codecs:
//!
//! - [`structural`] — full document snapshot (compact, verbose, binary, compact-binary)
//! - [`indexed`] — each node separately in a field map
//! - [`sidecar`] — view bytes + metadata bytes split

pub mod indexed;
pub mod sidecar;
pub mod structural;
