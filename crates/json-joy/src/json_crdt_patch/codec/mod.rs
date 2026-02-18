//! Codecs for the JSON CRDT Patch protocol.
//!
//! Four wire formats are supported:
//! - `binary` — compact binary format (primary wire format)
//! - `verbose` — human-readable JSON object format
//! - `compact` — space-efficient JSON array format
//! - `compact_binary` — CBOR-encoded compact format

pub mod binary;
pub mod compact;
pub mod compact_binary;
pub mod verbose;
