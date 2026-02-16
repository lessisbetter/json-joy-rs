use serde_json::Value;

use super::PathStep;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeEventOrigin {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEvent {
    pub origin: ChangeEventOrigin,
    pub patch_id: Option<(u64, u64)>,
    pub before: Value,
    pub after: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchChangeEvent {
    pub patch_ids: Vec<(u64, u64)>,
    pub before: Value,
    pub after: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScopedChangeEvent {
    pub path: Vec<PathStep>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub patch_id: Option<(u64, u64)>,
    pub origin: ChangeEventOrigin,
}
