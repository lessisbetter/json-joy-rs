//! Types for the verbose JSON codec.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/verbose/types.ts`.

use serde_json::Value;

/// A timestamp in verbose format: either a bare number (for server session)
/// or a `[sessionId, time]` array.
pub type VerboseTs = Value; // number | [number, number]

/// A timespan in verbose format: `[sessionId, time, span]` or `[time, span]`.
pub type VerboseTss = Value;

/// The verbose patch representation.
#[derive(Debug, Clone)]
pub struct VerbosePatch {
    /// Patch ID as `[sessionId, time]` or just `time` for server session.
    pub id: VerboseTs,
    /// List of operation objects.
    pub ops: Vec<VerboseOp>,
    /// Optional metadata.
    pub meta: Option<Value>,
}

/// A single verbose operation.
#[derive(Debug, Clone)]
pub struct VerboseOp {
    pub op: String,
    pub fields: std::collections::HashMap<String, Value>,
}
