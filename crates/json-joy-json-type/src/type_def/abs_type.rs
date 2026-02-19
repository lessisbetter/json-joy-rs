//! Base type node and shared info.
//!
//! Upstream reference: json-type/src/type/classes/AbsType.ts

use serde_json::Value;
use std::sync::Arc;

use super::module_type::ModuleType;

/// Validator function type: receives a JSON value, returns None if ok or Some(error) if invalid.
pub type ValidatorFn = Arc<dyn Fn(&Value) -> Option<String> + Send + Sync>;

/// Shared fields for all type nodes (metadata + validators + system reference).
#[derive(Clone, Default)]
pub struct BaseInfo {
    pub title: Option<String>,
    pub intro: Option<String>,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub examples: Vec<Value>,
    pub validators: Vec<(ValidatorFn, Option<String>)>,
    pub system: Option<Arc<ModuleType>>,
}

impl std::fmt::Debug for BaseInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseInfo")
            .field("title", &self.title)
            .field("intro", &self.intro)
            .field("description", &self.description)
            .field("default", &self.default)
            .field("validators_count", &self.validators.len())
            .finish()
    }
}

impl BaseInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_system(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.system = system;
        self
    }
}
