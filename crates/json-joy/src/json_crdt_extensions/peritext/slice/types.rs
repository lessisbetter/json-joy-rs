//! Slice type system for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/types.ts`.
//!
//! Slice types identify *what kind* of annotation a slice represents.
//! A type can be:
//! - a single integer tag (e.g. `TYPE_BOLD = -3`)
//! - a single string tag (e.g. `"bold"`, `"link"`)
//! - a sequence of steps for nested block structure (e.g. `["ul", "li"]`)

use json_joy_json_pack::PackValue;
use serde_json::Value;

// ── TypeTag ───────────────────────────────────────────────────────────────

/// A single annotation type tag — either a well-known integer constant or a
/// free-form string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeTag {
    /// Numeric tag (e.g. `TYPE_BOLD = -3`, `TYPE_P = 0`).
    Int(i64),
    /// String tag (e.g. `"bold"`, `"link"`, `"<b>"`).
    Str(String),
}

impl TypeTag {
    /// Convert to a [`PackValue`] for storage in a ConNode.
    pub fn to_pack(&self) -> PackValue {
        match self {
            TypeTag::Int(n) => PackValue::Integer(*n),
            TypeTag::Str(s) => PackValue::Str(s.clone()),
        }
    }

    /// Try to deserialise from a [`PackValue`].
    pub fn from_pack(pv: &PackValue) -> Option<Self> {
        match pv {
            PackValue::Integer(n) => Some(TypeTag::Int(*n)),
            PackValue::UInteger(n) => Some(TypeTag::Int(*n as i64)),
            PackValue::Str(s) => Some(TypeTag::Str(s.clone())),
            _ => None,
        }
    }
}

impl From<i64> for TypeTag {
    fn from(n: i64) -> Self {
        TypeTag::Int(n)
    }
}
impl From<&str> for TypeTag {
    fn from(s: &str) -> Self {
        TypeTag::Str(s.to_string())
    }
}
impl From<String> for TypeTag {
    fn from(s: String) -> Self {
        TypeTag::Str(s)
    }
}

// ── SliceType ─────────────────────────────────────────────────────────────

/// The full type of a slice annotation.
///
/// A simple inline annotation (bold, italic, …) uses `Simple(tag)`.  A nested
/// block structure (e.g. a list item inside a list) uses `Steps(vec![...])`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceType {
    /// A single type tag (most annotations).
    Simple(TypeTag),
    /// An ordered path of tags describing nested block structure.
    Steps(Vec<TypeTag>),
}

impl SliceType {
    /// Convert to a [`PackValue`] for storage (integer or array of integers/strings).
    pub fn to_pack(&self) -> PackValue {
        match self {
            SliceType::Simple(tag) => tag.to_pack(),
            SliceType::Steps(steps) => {
                PackValue::Array(steps.iter().map(TypeTag::to_pack).collect())
            }
        }
    }

    /// Try to deserialise from a [`PackValue`].
    pub fn from_pack(pv: &PackValue) -> Option<Self> {
        match pv {
            PackValue::Array(items) => {
                let steps: Option<Vec<TypeTag>> = items.iter().map(TypeTag::from_pack).collect();
                steps.map(SliceType::Steps)
            }
            other => TypeTag::from_pack(other).map(SliceType::Simple),
        }
    }
}

impl From<i64> for SliceType {
    fn from(n: i64) -> Self {
        SliceType::Simple(TypeTag::Int(n))
    }
}
impl From<&str> for SliceType {
    fn from(s: &str) -> Self {
        SliceType::Simple(TypeTag::Str(s.to_string()))
    }
}
impl From<String> for SliceType {
    fn from(s: String) -> Self {
        SliceType::Simple(TypeTag::Str(s))
    }
}
impl From<TypeTag> for SliceType {
    fn from(tag: TypeTag) -> Self {
        SliceType::Simple(tag)
    }
}
