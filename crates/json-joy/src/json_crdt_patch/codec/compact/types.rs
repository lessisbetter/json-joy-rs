//! Types for the compact JSON codec.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/compact/types.ts`.

use serde_json::Value;

/// Compact patch: `[[id, meta?], ...ops]`.
pub type CompactPatch = Vec<Value>;
