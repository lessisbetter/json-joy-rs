//! Metaschema — describes the json-type schema system using itself.
//!
//! Upstream reference: json-type/src/metaschema/

#[allow(clippy::module_inception)]
pub mod metaschema;

pub use metaschema::module;
