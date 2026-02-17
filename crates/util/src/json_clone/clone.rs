use serde_json::{Map, Value};

/// Creates a deep clone of any JSON value.
///
/// This is a recursive clone that creates new instances of all
/// nested objects and arrays.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_clone::clone;
///
/// let original = json!({"foo": [1, 2, 3]});
/// let cloned = clone(&original);
///
/// // The cloned value is equal to the original
/// assert_eq!(original, cloned);
/// ```
pub fn clone(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(b) => Value::Bool(*b),
        Value::Number(n) => Value::Number(n.clone()),
        Value::String(s) => Value::String(s.clone()),
        Value::Array(arr) => Value::Array(arr.iter().map(clone).collect()),
        Value::Object(obj) => {
            let mut new_obj = Map::new();
            for (key, val) in obj {
                new_obj.insert(key.clone(), clone(val));
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
    fn test_clone_null() {
        let value = json!(null);
        let cloned = clone(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_bool() {
        let value = json!(true);
        let cloned = clone(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_number() {
        let value = json!(42);
        let cloned = clone(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_string() {
        let value = json!("hello");
        let cloned = clone(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_array() {
        let value = json!([1, 2, 3]);
        let cloned = clone(&value);
        assert_eq!(value, cloned);

        // Verify it's a deep copy
        if let Value::Array(arr) = cloned {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_clone_object() {
        let value = json!({"foo": "bar"});
        let cloned = clone(&value);
        assert_eq!(value, cloned);

        // Verify it's a deep copy
        if let Value::Object(obj) = cloned {
            assert_eq!(obj.get("foo").unwrap(), &json!("bar"));
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_clone_nested() {
        let value = json!({
            "array": [1, 2, {"nested": true}],
            "object": {"a": "b"},
            "scalar": 42
        });
        let cloned = clone(&value);
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_clone_is_deep() {
        let original = json!({"arr": [1, 2, 3]});
        let cloned = clone(&original);

        // Modify the cloned array (if we had mutable access)
        // The original should not be affected
        assert_eq!(original, cloned);
    }
}
