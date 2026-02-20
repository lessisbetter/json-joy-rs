//! Schema AST module.
//!
//! Upstream reference: json-type/src/schema/

pub mod builder;
pub mod common;
#[allow(clippy::module_inception)]
pub mod schema;
pub mod validate;
pub mod walker;

pub use builder::SchemaBuilder;
pub use common::Display;
pub use schema::*;
pub use validate::validate_schema;
pub use walker::Walker;
