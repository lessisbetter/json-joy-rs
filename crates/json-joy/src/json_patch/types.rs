//! Core types for the JSON Patch module.
//!
//! Mirrors `packages/json-joy/src/json-patch/types.ts`,
//! `packages/json-joy/src/json-patch/constants.ts`, and
//! the Op class hierarchy in `packages/json-joy/src/json-patch/op/`.

use serde_json::{Map, Value};
use thiserror::Error;

pub use json_joy_json_pointer::Path;

// ── Error ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error, PartialEq)]
pub enum PatchError {
    #[error("NOT_FOUND")]
    NotFound,
    #[error("TEST")]
    Test,
    #[error("NOT_A_STRING")]
    NotAString,
    #[error("INVALID_INDEX")]
    InvalidIndex,
    #[error("INVALID_TARGET")]
    InvalidTarget,
    #[error("INVALID_OP: {0}")]
    InvalidOp(String),
}

// ── Type enum for test_type / type operations ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPatchType {
    String,
    Number,
    Boolean,
    Object,
    Integer,
    Array,
    Null,
}

impl JsonPatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JsonPatchType::String => "string",
            JsonPatchType::Number => "number",
            JsonPatchType::Boolean => "boolean",
            JsonPatchType::Object => "object",
            JsonPatchType::Integer => "integer",
            JsonPatchType::Array => "array",
            JsonPatchType::Null => "null",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, PatchError> {
        match s {
            "string" => Ok(JsonPatchType::String),
            "number" => Ok(JsonPatchType::Number),
            "boolean" => Ok(JsonPatchType::Boolean),
            "object" => Ok(JsonPatchType::Object),
            "integer" => Ok(JsonPatchType::Integer),
            "array" => Ok(JsonPatchType::Array),
            "null" => Ok(JsonPatchType::Null),
            other => Err(PatchError::InvalidOp(format!("unknown type: {other}"))),
        }
    }

    /// Returns true if the given JSON value matches this type.
    pub fn matches_value(&self, val: &Value) -> bool {
        match self {
            JsonPatchType::String => val.is_string(),
            JsonPatchType::Number => val.is_number(),
            JsonPatchType::Boolean => val.is_boolean(),
            JsonPatchType::Object => val.is_object(),
            JsonPatchType::Integer => val.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false),
            JsonPatchType::Array => val.is_array(),
            JsonPatchType::Null => val.is_null(),
        }
    }
}

// ── Op enum ───────────────────────────────────────────────────────────────

/// A JSON Patch operation.
///
/// Mirrors the class hierarchy in `packages/json-joy/src/json-patch/op/`.
#[derive(Debug, Clone)]
pub enum Op {
    // ── RFC 6902 operations ───────────────────────────────────────────────
    Add {
        path: Path,
        value: Value,
    },
    Remove {
        path: Path,
        old_value: Option<Value>,
    },
    Replace {
        path: Path,
        value: Value,
        old_value: Option<Value>,
    },
    Copy {
        path: Path,
        from: Path,
    },
    Move {
        path: Path,
        from: Path,
    },
    Test {
        path: Path,
        value: Value,
        not: bool,
    },

    // ── Extended operations ───────────────────────────────────────────────
    StrIns {
        path: Path,
        pos: usize,
        str_val: String,
    },
    StrDel {
        path: Path,
        pos: usize,
        str_val: Option<String>,
        len: Option<usize>,
    },
    Flip {
        path: Path,
    },
    Inc {
        path: Path,
        inc: f64,
    },
    Split {
        path: Path,
        pos: usize,
        props: Option<Value>,
    },
    Merge {
        path: Path,
        pos: usize,
        props: Option<Value>,
    },
    Extend {
        path: Path,
        props: Map<String, Value>,
        delete_null: bool,
    },

    // ── First-order predicate operations ─────────────────────────────────
    Defined {
        path: Path,
    },
    Undefined {
        path: Path,
    },
    Contains {
        path: Path,
        value: String,
        ignore_case: bool,
    },
    Ends {
        path: Path,
        value: String,
        ignore_case: bool,
    },
    Starts {
        path: Path,
        value: String,
        ignore_case: bool,
    },
    In {
        path: Path,
        value: Vec<Value>,
    },
    Less {
        path: Path,
        value: f64,
    },
    More {
        path: Path,
        value: f64,
    },
    Matches {
        path: Path,
        value: String,
        ignore_case: bool,
    },
    TestType {
        path: Path,
        type_vals: Vec<JsonPatchType>,
    },
    TestString {
        path: Path,
        pos: usize,
        str_val: String,
        not: bool,
    },
    TestStringLen {
        path: Path,
        len: usize,
        not: bool,
    },
    Type {
        path: Path,
        value: JsonPatchType,
    },

    // ── Second-order predicate operations ─────────────────────────────────
    And {
        path: Path,
        ops: Vec<Op>,
    },
    Not {
        path: Path,
        ops: Vec<Op>,
    },
    Or {
        path: Path,
        ops: Vec<Op>,
    },
}

impl Op {
    /// Returns the operation name string (matching the TypeScript `op()` method).
    pub fn op_name(&self) -> &'static str {
        match self {
            Op::Add { .. } => "add",
            Op::Remove { .. } => "remove",
            Op::Replace { .. } => "replace",
            Op::Copy { .. } => "copy",
            Op::Move { .. } => "move",
            Op::Test { .. } => "test",
            Op::StrIns { .. } => "str_ins",
            Op::StrDel { .. } => "str_del",
            Op::Flip { .. } => "flip",
            Op::Inc { .. } => "inc",
            Op::Split { .. } => "split",
            Op::Merge { .. } => "merge",
            Op::Extend { .. } => "extend",
            Op::Defined { .. } => "defined",
            Op::Undefined { .. } => "undefined",
            Op::Contains { .. } => "contains",
            Op::Ends { .. } => "ends",
            Op::Starts { .. } => "starts",
            Op::In { .. } => "in",
            Op::Less { .. } => "less",
            Op::More { .. } => "more",
            Op::Matches { .. } => "matches",
            Op::TestType { .. } => "test_type",
            Op::TestString { .. } => "test_string",
            Op::TestStringLen { .. } => "test_string_len",
            Op::Type { .. } => "type",
            Op::And { .. } => "and",
            Op::Not { .. } => "not",
            Op::Or { .. } => "or",
        }
    }

    /// Returns the path of the operation.
    pub fn path(&self) -> &Path {
        match self {
            Op::Add { path, .. } => path,
            Op::Remove { path, .. } => path,
            Op::Replace { path, .. } => path,
            Op::Copy { path, .. } => path,
            Op::Move { path, .. } => path,
            Op::Test { path, .. } => path,
            Op::StrIns { path, .. } => path,
            Op::StrDel { path, .. } => path,
            Op::Flip { path, .. } => path,
            Op::Inc { path, .. } => path,
            Op::Split { path, .. } => path,
            Op::Merge { path, .. } => path,
            Op::Extend { path, .. } => path,
            Op::Defined { path, .. } => path,
            Op::Undefined { path, .. } => path,
            Op::Contains { path, .. } => path,
            Op::Ends { path, .. } => path,
            Op::Starts { path, .. } => path,
            Op::In { path, .. } => path,
            Op::Less { path, .. } => path,
            Op::More { path, .. } => path,
            Op::Matches { path, .. } => path,
            Op::TestType { path, .. } => path,
            Op::TestString { path, .. } => path,
            Op::TestStringLen { path, .. } => path,
            Op::Type { path, .. } => path,
            Op::And { path, .. } => path,
            Op::Not { path, .. } => path,
            Op::Or { path, .. } => path,
        }
    }

    /// Returns true if this is a predicate operation.
    pub fn is_predicate(&self) -> bool {
        matches!(
            self,
            Op::Test { .. }
                | Op::Defined { .. }
                | Op::Undefined { .. }
                | Op::Contains { .. }
                | Op::Ends { .. }
                | Op::Starts { .. }
                | Op::In { .. }
                | Op::Less { .. }
                | Op::More { .. }
                | Op::Matches { .. }
                | Op::TestType { .. }
                | Op::TestString { .. }
                | Op::TestStringLen { .. }
                | Op::Type { .. }
                | Op::And { .. }
                | Op::Not { .. }
                | Op::Or { .. }
        )
    }
}

// ── Result types ──────────────────────────────────────────────────────────

/// Result of applying a single operation.
#[derive(Debug, Clone)]
pub struct OpResult {
    /// The document after applying the operation.
    pub doc: Value,
    /// The value at the path before the operation, if applicable.
    pub old: Option<Value>,
}

/// Result of applying a full patch.
#[derive(Debug, Clone)]
pub struct PatchResult {
    pub doc: Value,
    pub res: Vec<OpResult>,
}

/// Options for `apply_patch`.
#[derive(Debug, Clone)]
pub struct ApplyPatchOptions {
    /// If true, mutate the document in place (passed by value).
    /// If false, clone the document before applying.
    pub mutate: bool,
}

impl Default for ApplyPatchOptions {
    fn default() -> Self {
        Self { mutate: false }
    }
}
