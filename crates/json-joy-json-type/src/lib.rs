//! `json-joy-json-type` — type-system and schema framework for the json-joy ecosystem.
//!
//! Upstream reference: `@jsonjoy.com/json-type` v17.67.0
//! Source: `/Users/nchapman/Code/json-joy/packages/json-type/src/`

pub mod codegen;
pub mod constants;
pub mod json_schema;
pub mod jtd;
pub mod metaschema;
pub mod random;
pub mod schema;
pub mod type_def;
pub mod typescript;
pub mod value;

// Re-export the most commonly used types at crate root
pub use codegen::validator::{validate, ErrorMode, ValidationResult, ValidatorOptions};
pub use constants::ValidationError;
pub use schema::{Schema, SchemaBase};
pub use type_def::{BaseInfo, ModuleType, TypeBuilder, TypeNode};
