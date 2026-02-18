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

pub mod clock;
pub mod enums;
pub mod constants;
pub mod operations;
pub mod patch;
pub mod patch_builder;
pub mod batch;
pub mod compaction;
pub mod schema;
pub mod util;
pub mod codec;

// ── Re-exports ─────────────────────────────────────────────────────────────

pub use clock::{ts, tss, tick, equal, compare, contains, contains_id, interval, print_ts};
pub use clock::{Ts, Tss, LogicalClock, ClockVector, ServerClockVector};
pub use constants::ORIGIN;
pub use enums::{JsonCrdtDataType, JsonCrdtPatchOpcode, OpcodeOverlay, SESSION, SYSTEM_SESSION_TIME};
pub use operations::{ConValue, Op};
pub use patch::Patch;
pub use patch_builder::PatchBuilder;
pub use batch::Batch;
pub use compaction::{combine, compact};
