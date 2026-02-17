//! Type definitions for JSON Pointer.

use serde_json::Value;

/// A step in a JSON Pointer path.
///
/// Can be either a string (object key) or number (array index).
pub type PathStep = String;

/// A JSON Pointer path.
pub type Path = Vec<PathStep>;

/// A reference to a value in a JSON document.
///
/// Contains the target value, the object that contains it, and the key used
/// to access it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The target value. `None` if the value doesn't exist or is null.
    pub val: Option<Value>,
    /// The object or array containing the target value.
    pub obj: Option<Value>,
    /// The key (string for objects, index for arrays) used to access the value.
    pub key: Option<String>,
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

    /// Check if this reference points to the end of an array.
    ///
    /// Returns true if the key is equal to the array length.
    pub fn is_array_end(&self) -> bool {
        if let (Some(Value::Array(arr)), Some(key)) = (&self.obj, &self.key) {
            if let Ok(idx) = key.parse::<usize>() {
                return idx == arr.len();
            }
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
        self.key.as_ref().and_then(|k| k.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_reference_is_array() {
        let doc = json!([1, 2, 3]);
        let ref_val = Reference {
            val: Some(json!(2)),
            obj: Some(doc.clone()),
            key: Some("1".to_string()),
        };
        assert!(ref_val.is_array_reference());
        assert!(!ref_val.is_object_reference());
        assert_eq!(ref_val.index(), Some(1));
    }

    #[test]
    fn test_reference_is_object() {
        let doc = json!({"foo": "bar"});
        let ref_val = Reference {
            val: Some(json!("bar")),
            obj: Some(doc.clone()),
            key: Some("foo".to_string()),
        };
        assert!(ref_val.is_object_reference());
        assert!(!ref_val.is_array_reference());
        assert_eq!(ref_val.index(), None);
    }

    #[test]
    fn test_reference_array_end() {
        let doc = json!([1, 2, 3]);
        let ref_val = Reference {
            val: None,
            obj: Some(doc.clone()),
            key: Some("3".to_string()),
        };
        assert!(ref_val.is_array_end());
    }
}
