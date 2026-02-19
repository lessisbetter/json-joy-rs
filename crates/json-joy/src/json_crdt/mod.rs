//! JSON CRDT document model and node types.
//!
//! Mirrors `packages/json-joy/src/json-crdt/`.
//!
//! This module provides:
//! - A simplified in-memory CRDT document model ([`model::Model`])
//! - All JSON CRDT node types ([`nodes`])
//! - The UNDEFINED_TS / ORIGIN sentinel constants ([`constants`])

pub mod codec;
pub mod constants;
pub mod draft;
pub mod equal;
pub mod extensions;
pub mod json_patch_apply;
pub mod log;
pub mod model;
pub mod nodes;
pub mod partial_edit;
pub mod schema;

pub use constants::{ORIGIN, UNDEFINED_TS};
pub use extensions::{AnyExtension, ExtApi, ExtNode, Extensions};
pub use model::Model;
pub use model::ModelApi;
pub use nodes::{CrdtNode, NodeIndex};
