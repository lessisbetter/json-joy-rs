//! Constants for the JSON CRDT Patch protocol.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/constants.ts`.

pub use crate::json_crdt_patch::enums::{SESSION, SYSTEM_SESSION_TIME};

use crate::json_crdt_patch::clock::{ts, Ts};

/// The origin timestamp: `(SESSION::SYSTEM, SYSTEM_SESSION_TIME::ORIGIN)`.
///
/// Represents the root element or the bottom value of a logical timestamp.
/// Used as the default reference for the document root LWW-Register.
pub const ORIGIN: Ts = Ts::new(SESSION::SYSTEM, SYSTEM_SESSION_TIME::ORIGIN);
