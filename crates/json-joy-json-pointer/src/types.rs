//! Type definitions for JSON Pointer.

use serde_json::Value;

/// A step in a JSON Pointer path.
///
/// Can be either a string (object key) or number (array index).
pub type PathStep = String;

/// A JSON Pointer path.
pub type Path = Vec<PathStep>;

/// The key used to reference a value within its container.
///
/// Mirrors the upstream TypeScript `Reference.key` which is typed as
/// `string | number` — a `String` variant for object properties and an
/// `Index` variant for array elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKey {
    /// An object property key.
    String(String),
    /// A numeric array index (including the end-of-array sentinel).
    Index(usize),
}

impl ReferenceKey {
    /// Return the key as a string slice, regardless of variant.
    pub fn as_str(&self) -> &str {
        match self {
            ReferenceKey::String(s) => s.as_str(),
            ReferenceKey::Index(_) => "",
        }
    }

    /// Return the numeric index, if this is an `Index` variant.
    pub fn as_index(&self) -> Option<usize> {
        match self {
            ReferenceKey::Index(n) => Some(*n),
            ReferenceKey::String(_) => None,
        }
    }

    /// Return true if this is a string key (object property).
    pub fn is_string(&self) -> bool {
        matches!(self, ReferenceKey::String(_))
    }

    /// Return true if this is a numeric index (array element).
    pub fn is_index(&self) -> bool {
        matches!(self, ReferenceKey::Index(_))
    }
}

impl std::fmt::Display for ReferenceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceKey::String(s) => write!(f, "{}", s),
            ReferenceKey::Index(n) => write!(f, "{}", n),
        }
    }
}

/// A reference to a value in a JSON document.
///
/// Contains the target value, the object that contains it, and the key used
/// to access it.  Mirrors the upstream TypeScript `Reference` interface from
/// `find.ts`:
///
/// ```ts
/// interface Reference {
///   val: unknown;
///   obj?: unknown;
///   key?: string | number;
/// }
/// ```
///
/// Key semantics:
/// - `key` is `ReferenceKey::String` when the container is an object.
/// - `key` is `ReferenceKey::Index` when the container is an array.
/// - `val` is `None` only when the location truly does not exist (missing
///   object key or out-of-bounds array index).  An explicit JSON `null` is
///   returned as `Some(Value::Null)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The target value.  `None` when the location does not exist.
    /// An explicit `null` in the document is `Some(Value::Null)`.
    pub val: Option<Value>,
    /// The object or array containing the target value.
    pub obj: Option<Value>,
    /// The key used to access the value in the container.
    pub key: Option<ReferenceKey>,
}

impl Reference {
    /// Check if this reference points to an array element.
    pub fn is_array_reference(&self) -> bool {
        matches!(&self.obj, Some(Value::Array(_)))
    }

    /// Check if this reference points to an object property.
    pub fn is_object_reference(&self) -> bool {
        matches!(&self.obj, Some(Value::Object(_)))
    }

    /// Check if this reference points to the end of an array (one past last).
    pub fn is_array_end(&self) -> bool {
        if let (Some(Value::Array(arr)), Some(ReferenceKey::Index(idx))) = (&self.obj, &self.key) {
            return *idx == arr.len();
        }
        false
    }

    /// Get the array containing the referenced value.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match &self.obj {
            Some(Value::Array(arr)) => Some(arr),
            _ => None,
        }
    }

    /// Get the object containing the referenced value.
    pub fn as_object(&self) -> Option<&serde_json::Map<String, Value>> {
        match &self.obj {
            Some(Value::Object(map)) => Some(map),
            _ => None,
        }
    }

    /// Get the numeric index if this is an array reference.
    pub fn index(&self) -> Option<usize> {
        match &self.key {
            Some(ReferenceKey::Index(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get the string key if this is an object reference.
    pub fn string_key(&self) -> Option<&str> {
        match &self.key {
            Some(ReferenceKey::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_reference_key_index() {
        let key = ReferenceKey::Index(1);
        assert!(key.is_index());
        assert!(!key.is_string());
        assert_eq!(key.as_index(), Some(1));
        assert_eq!(key.to_string(), "1");
    }

    #[test]
    fn test_reference_key_string() {
        let key = ReferenceKey::String("foo".to_string());
        assert!(key.is_string());
        assert!(!key.is_index());
        assert_eq!(key.as_index(), None);
        assert_eq!(key.as_str(), "foo");
    }

    #[test]
    fn test_reference_is_array() {
        let doc = json!([1, 2, 3]);
        let ref_val = Reference {
            val: Some(json!(2)),
            obj: Some(doc.clone()),
            key: Some(ReferenceKey::Index(1)),
        };
        assert!(ref_val.is_array_reference());
        assert!(!ref_val.is_object_reference());
        assert_eq!(ref_val.index(), Some(1));
        assert_eq!(ref_val.string_key(), None);
    }

    #[test]
    fn test_reference_is_object() {
        let doc = json!({"foo": "bar"});
        let ref_val = Reference {
            val: Some(json!("bar")),
            obj: Some(doc.clone()),
            key: Some(ReferenceKey::String("foo".to_string())),
        };
        assert!(ref_val.is_object_reference());
        assert!(!ref_val.is_array_reference());
        assert_eq!(ref_val.index(), None);
        assert_eq!(ref_val.string_key(), Some("foo"));
    }

    #[test]
    fn test_reference_array_end() {
        let doc = json!([1, 2, 3]);
        let ref_val = Reference {
            val: None,
            obj: Some(doc.clone()),
            key: Some(ReferenceKey::Index(3)),
        };
        assert!(ref_val.is_array_end());
    }

    #[test]
    fn test_reference_array_not_end() {
        let doc = json!([1, 2, 3]);
        let ref_val = Reference {
            val: Some(json!(2)),
            obj: Some(doc.clone()),
            key: Some(ReferenceKey::Index(1)),
        };
        assert!(!ref_val.is_array_end());
    }
}
