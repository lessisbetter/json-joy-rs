//! JSON text encoder runtime port.
//!
//! Upstream reference: `json-type/src/codegen/json/JsonTextCodegen.ts`.

pub mod json_text_codegen;

pub use json_text_codegen::{JsonEncoderFn, JsonTextCodegen};
