//! Structured template-based random JSON generation.

pub mod template_json;
pub mod templates;
pub mod types;

pub use template_json::{TemplateJson, TemplateJsonOpts};
pub use types::{ObjectTemplateField, Template};
