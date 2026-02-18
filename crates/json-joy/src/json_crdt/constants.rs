//! JSON CRDT constants.
//!
//! Mirrors `packages/json-joy/src/json-crdt/constants.ts`.

use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::enums::{SESSION, SYSTEM_SESSION_TIME};

/// The "undefined" timestamp sentinel — the default value of an empty register.
///
/// In the upstream TypeScript, the root `ValNode` starts pointing at
/// `ORIGIN` (the special undefined/null constant node).  In our simplified
/// model we use a distinct sentinel so we can tell "no value set yet".
pub const UNDEFINED_TS: Ts = Ts::new(SESSION::SYSTEM, SYSTEM_SESSION_TIME::UNDEFINED);

/// The ORIGIN timestamp — the bottom of the timestamp lattice.
///
/// Re-exported from `json_crdt_patch` for convenience inside this module.
pub use crate::json_crdt_patch::constants::ORIGIN;
