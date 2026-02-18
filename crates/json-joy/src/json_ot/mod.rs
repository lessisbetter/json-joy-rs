//! JSON Operational Transformation types.
//!
//! Mirrors `packages/json-joy/src/json-ot/`.
//!
//! Provides OT algorithms for strings, binary data, and JSON documents.

pub mod types;

pub use types::ot_string;
pub use types::ot_string_irrev;
pub use types::ot_binary_irrev;
pub use types::ot_json;
