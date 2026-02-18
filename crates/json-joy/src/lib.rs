//! json-joy — Rust port of the json-joy TypeScript library.
//!
//! Provides the full CRDT document model, patch protocol, diff, extensions,
//! JSON patch/OT, hashing, and JSON utilities.
//!
//! Sub-modules mirror the upstream TypeScript package layout under
//! `packages/json-joy/src/`.

// Slice 1 — Leaf utilities (no internal deps)
pub mod json_walk;
pub mod json_pretty;
pub mod json_stable;
pub mod json_size;
pub mod json_ml;

pub mod json_crdt_patch;       // Slice 2
pub mod util_inner;            // Slice 3
pub mod json_patch;            // Slice 4
pub mod json_patch_diff;       // Slice 4
pub mod json_ot;               // Slice 4
pub mod json_patch_ot;         // Slice 4

pub mod json_hash;             // Slice 5
pub mod json_crdt;             // Slice 5
pub mod json_crdt_diff;        // Slice 6
// pub mod json_crdt_extensions; // Slice 7
// pub mod json_crdt_peritext_ui; // Slice 8
// pub mod json_cli;          // Slice 9
