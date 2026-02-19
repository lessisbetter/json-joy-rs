use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A JSON value that may contain binary data (Uint8Array).
///
/// This is represented as a tagged enum to distinguish binary data
/// from regular arrays.
///
/// # Round-Trip Warning
///
/// Converting `JsonBinary::Binary` to `Value` encodes the binary data as an
/// array of numbers (e.g., `[1, 2, 3]`). This means `Value -> JsonBinary -> Value`
/// does **not** preserve the distinction between binary data and numeric arrays.
/// Use `JsonBinary` directly when you need to maintain this distinction.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonBinary {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<JsonBinary>),
    Object(BTreeMap<String, JsonBinary>),
    /// Binary data (equivalent to Uint8Array in JavaScript)
    Binary(Vec<u8>),
}

impl From<&Value> for JsonBinary {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => JsonBinary::Null,
            Value::Bool(b) => JsonBinary::Bool(*b),
            Value::Number(n) => JsonBinary::Number(n.clone()),
            Value::String(s) => JsonBinary::String(s.clone()),
            Value::Array(arr) => JsonBinary::Array(arr.iter().map(JsonBinary::from).collect()),
            Value::Object(obj) => {
                let mut new_obj = BTreeMap::new();
                for (key, val) in obj {
                    new_obj.insert(key.clone(), JsonBinary::from(val));
                }
                JsonBinary::Object(new_obj)
            }
        }
    }
}

impl From<JsonBinary> for Value {
    /// Converts a `JsonBinary` to a `Value`.
    ///
    /// **Note:** Binary data is encoded as an array of numbers. This loses
    /// the type distinction - see the struct-level documentation for details.
    fn from(binary: JsonBinary) -> Self {
        match binary {
            JsonBinary::Null => Value::Null,
            JsonBinary::Bool(b) => Value::Bool(b),
            JsonBinary::Number(n) => Value::Number(n),
            JsonBinary::String(s) => Value::String(s),
            JsonBinary::Array(arr) => Value::Array(arr.into_iter().map(Value::from).collect()),
            JsonBinary::Object(obj) => {
                let mut new_obj = Map::new();
                for (key, val) in obj {
                    new_obj.insert(key, Value::from(val));
                }
                Value::Object(new_obj)
            }
            JsonBinary::Binary(bytes) => {
                // Encode binary as array of numbers
                // WARNING: This loses the distinction between binary and numeric arrays
                Value::Array(bytes.into_iter().map(|b| Value::Number(b.into())).collect())
            }
        }
    }
}

/// Creates a deep clone of a JSON value that may contain binary data.
///
/// Binary data (Vec<u8>) is cloned as a new Vec<u8>.
///
/// # Examples
///
/// ```
/// use json_joy_util::json_clone::{clone_binary, JsonBinary};
///
/// let original = JsonBinary::Binary(vec![1, 2, 3]);
/// let cloned = clone_binary(&original);
///
/// if let (JsonBinary::Binary(a), JsonBinary::Binary(b)) = (&original, &cloned) {
///     assert_eq!(a, b);
/// }
/// ```
pub fn clone_binary(value: &JsonBinary) -> JsonBinary {
    match value {
        JsonBinary::Null => JsonBinary::Null,
        JsonBinary::Bool(b) => JsonBinary::Bool(*b),
        JsonBinary::Number(n) => JsonBinary::Number(n.clone()),
        JsonBinary::String(s) => JsonBinary::String(s.clone()),
        JsonBinary::Array(arr) => JsonBinary::Array(arr.iter().map(clone_binary).collect()),
        JsonBinary::Object(obj) => {
            let mut new_obj = BTreeMap::new();
            for (key, val) in obj {
                new_obj.insert(key.clone(), clone_binary(val));
            }
            JsonBinary::Object(new_obj)
        }
        JsonBinary::Binary(bytes) => JsonBinary::Binary(bytes.clone()),
    }
}

/// Clone a serde_json::Value deeply, handling binary data encoded as arrays.
///
/// This function checks if an array looks like it could be binary data
/// (all numbers in range 0-255) and clones it appropriately.
pub fn clone_value_with_binary(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(b) => Value::Bool(*b),
        Value::Number(n) => Value::Number(n.clone()),
        Value::String(s) => Value::String(s.clone()),
        Value::Array(arr) => {
            // Check if this looks like binary data (array of small integers)
            if arr
                .iter()
                .all(|v| matches!(v, Value::Number(n) if n.as_u64().map_or(false, |n| n <= 255)))
            {
                Value::Array(arr.clone())
            } else {
                Value::Array(arr.iter().map(clone_value_with_binary).collect())
            }
        }
        Value::Object(obj) => {
            let mut new_obj = Map::new();
            for (key, val) in obj {
                new_obj.insert(key.clone(), clone_value_with_binary(val));
            }
            Value::Object(new_obj)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_clone_binary_null() {
        let value = JsonBinary::Null;
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_bool() {
        let value = JsonBinary::Bool(true);
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_number() {
        let value = JsonBinary::Number(42.into());
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_string() {
        let value = JsonBinary::String("hello".to_string());
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_array() {
        let value = JsonBinary::Array(vec![
            JsonBinary::Number(1.into()),
            JsonBinary::Number(2.into()),
        ]);
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_object() {
        let mut obj = BTreeMap::new();
        obj.insert("key".to_string(), JsonBinary::String("value".to_string()));
        let value = JsonBinary::Object(obj);
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_bytes() {
        let value = JsonBinary::Binary(vec![1, 2, 3, 255]);
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_binary_nested() {
        let mut obj = BTreeMap::new();
        obj.insert("binary".to_string(), JsonBinary::Binary(vec![1, 2, 3]));
        obj.insert(
            "nested".to_string(),
            JsonBinary::Object({
                let mut inner = BTreeMap::new();
                inner.insert("arr".to_string(), JsonBinary::Array(vec![]));
                inner
            }),
        );
        let value = JsonBinary::Object(obj);
        let cloned = clone_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_value_with_binary() {
        let value = json!({
            "data": [1, 2, 3, 255],
            "nested": {
                "more": [0, 128, 255]
            }
        });
        let cloned = clone_value_with_binary(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_from_value() {
        let value = json!({
            "string": "hello",
            "number": 42,
            "bool": true,
            "null": null,
            "array": [1, 2, 3],
            "object": {"nested": "value"}
        });

        let binary = JsonBinary::from(&value);
        let back = Value::from(binary);
        assert_eq!(value, back);
    }

    /// Test that documents the round-trip limitation for binary data.
    /// Binary data is encoded as an array of numbers, which loses the type distinction.
    #[test]
    fn test_binary_to_value_round_trip_limitation() {
        // Create binary data
        let binary = JsonBinary::Binary(vec![1, 2, 3]);

        // Convert to Value - binary becomes array of numbers
        let value = Value::from(binary);
        assert_eq!(value, json!([1, 2, 3]));

        // Converting back gives an Array, not Binary (type info lost)
        let back = JsonBinary::from(&value);
        assert!(matches!(back, JsonBinary::Array(_)));
        assert!(!matches!(back, JsonBinary::Binary(_)));
    }

    /// Test that non-binary types preserve round-trip correctly.
    #[test]
    fn test_non_binary_round_trip() {
        let original = JsonBinary::Object({
            let mut obj = BTreeMap::new();
            obj.insert("string".to_string(), JsonBinary::String("test".to_string()));
            obj.insert("number".to_string(), JsonBinary::Number(42.into()));
            obj.insert("bool".to_string(), JsonBinary::Bool(true));
            obj.insert("null".to_string(), JsonBinary::Null);
            obj.insert(
                "array".to_string(),
                JsonBinary::Array(vec![
                    JsonBinary::Number(1.into()),
                    JsonBinary::Number(2.into()),
                ]),
            );
            obj
        });

        let value = Value::from(original.clone());
        let back = JsonBinary::from(&value);
        assert_eq!(original, back);
    }
}
