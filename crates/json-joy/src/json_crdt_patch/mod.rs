//! JSON CRDT Patch protocol.
//!
//! The foundational layer for JSON CRDT collaboration. Defines:
//! - Clock types (`Ts`, `Tss`, `LogicalClock`, `ClockVector`, `ServerClockVector`)
//! - 16 CRDT operations (`Op` enum)
//! - `Patch` — an ordered sequence of operations
//! - `PatchBuilder` — fluent builder for constructing patches
//! - `Batch` — a sequence of patches from the same session
//! - Codecs: `binary`, `verbose`, `compact`, `compact_binary`
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/`.

pub mod batch;
pub mod clock;
pub mod codec;
pub mod compaction;
pub mod constants;
pub mod enums;
pub mod operations;
pub mod patch;
pub mod patch_builder;
pub mod schema;
pub mod util;

// ── Re-exports ─────────────────────────────────────────────────────────────

pub use batch::Batch;
pub use clock::{compare, contains, contains_id, equal, interval, print_ts, tick, ts, tss};
pub use clock::{ClockVector, LogicalClock, ServerClockVector, Ts, Tss};
pub use compaction::{combine, compact};
pub use constants::ORIGIN;
pub use enums::{
    JsonCrdtDataType, JsonCrdtPatchOpcode, OpcodeOverlay, SESSION, SYSTEM_SESSION_TIME,
};
pub use operations::{ConValue, Op};
pub use patch::Patch;
pub use patch_builder::PatchBuilder;
