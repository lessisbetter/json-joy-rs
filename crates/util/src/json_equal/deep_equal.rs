use serde_json::Value;

/// Performs a deep equality check between two JSON values.
///
/// This function compares values recursively, checking equality for:
/// - Primitives (null, bool, number, string)
/// - Arrays (element-by-element comparison)
/// - Objects (key-by-key comparison)
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_equal::deep_equal;
///
/// let a = json!({"foo": [1, 2, 3]});
/// let b = json!({"foo": [1, 2, 3]});
/// let c = json!({"foo": [1, 2, 4]});
///
/// assert!(deep_equal(&a, &b));
/// assert!(!deep_equal(&a, &c));
/// ```
pub fn deep_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,

        // Arrays
        (Value::Array(arr_a), Value::Array(arr_b)) => {
            if arr_a.len() != arr_b.len() {
                return false;
            }
            for i in 0..arr_a.len() {
                if !deep_equal(&arr_a[i], &arr_b[i]) {
                    return false;
                }
            }
            true
        }

        // Objects
        (Value::Object(obj_a), Value::Object(obj_b)) => {
            if obj_a.len() != obj_b.len() {
                return false;
            }
            for (key, val_a) in obj_a {
                match obj_b.get(key) {
                    Some(val_b) => {
                        if !deep_equal(val_a, val_b) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }

        // Different types are never equal
        _ => false,
    }
}

/// Performs a deep equality check between two values that may contain binary data.
///
/// This function extends `deep_equal` to support comparing binary data (Vec<u8>).
///
/// # Examples
///
/// ```
/// use json_joy_util::json_equal::{deep_equal_binary, JsonBinary};
///
/// let a = JsonBinary::Binary(vec![1, 2, 3]);
/// let b = JsonBinary::Binary(vec![1, 2, 3]);
/// let c = JsonBinary::Binary(vec![1, 2, 4]);
///
/// assert!(deep_equal_binary(&a, &b));
/// assert!(!deep_equal_binary(&a, &c));
/// ```
pub fn deep_equal_binary(
    a: &crate::json_clone::JsonBinary,
    b: &crate::json_clone::JsonBinary,
) -> bool {
    use crate::json_clone::JsonBinary;

    match (a, b) {
        (JsonBinary::Null, JsonBinary::Null) => true,
        (JsonBinary::Bool(a), JsonBinary::Bool(b)) => a == b,
        (JsonBinary::Number(a), JsonBinary::Number(b)) => a == b,
        (JsonBinary::String(a), JsonBinary::String(b)) => a == b,

        (JsonBinary::Binary(a), JsonBinary::Binary(b)) => a == b,

        (JsonBinary::Array(arr_a), JsonBinary::Array(arr_b)) => {
            if arr_a.len() != arr_b.len() {
                return false;
            }
            for i in 0..arr_a.len() {
                if !deep_equal_binary(&arr_a[i], &arr_b[i]) {
                    return false;
                }
            }
            true
        }

        (JsonBinary::Object(obj_a), JsonBinary::Object(obj_b)) => {
            if obj_a.len() != obj_b.len() {
                return false;
            }
            for (key, val_a) in obj_a {
                match obj_b.get(key) {
                    Some(val_b) => {
                        if !deep_equal_binary(val_a, val_b) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }

        // Different types are never equal
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_clone::JsonBinary;
    use serde_json::json;

    // Scalar tests
    #[test]
    fn test_equal_numbers() {
        assert!(deep_equal(&json!(1), &json!(1)));
    }

    #[test]
    fn test_not_equal_numbers() {
        assert!(!deep_equal(&json!(1), &json!(2)));
    }

    #[test]
    fn test_number_and_array_not_equal() {
        assert!(!deep_equal(&json!(1), &json!([])));
    }

    #[test]
    fn test_zero_and_null_not_equal() {
        assert!(!deep_equal(&json!(0), &json!(null)));
    }

    #[test]
    fn test_equal_strings() {
        assert!(deep_equal(&json!("a"), &json!("a")));
    }

    #[test]
    fn test_not_equal_strings() {
        assert!(!deep_equal(&json!("a"), &json!("b")));
    }

    #[test]
    fn test_empty_string_and_null_not_equal() {
        assert!(!deep_equal(&json!(""), &json!(null)));
    }

    #[test]
    fn test_null_equal_null() {
        assert!(deep_equal(&json!(null), &json!(null)));
    }

    #[test]
    fn test_equal_booleans_true() {
        assert!(deep_equal(&json!(true), &json!(true)));
    }

    #[test]
    fn test_equal_booleans_false() {
        assert!(deep_equal(&json!(false), &json!(false)));
    }

    #[test]
    fn test_not_equal_booleans() {
        assert!(!deep_equal(&json!(true), &json!(false)));
    }

    #[test]
    fn test_one_and_true_not_equal() {
        assert!(!deep_equal(&json!(1), &json!(true)));
    }

    #[test]
    fn test_zero_and_false_not_equal() {
        assert!(!deep_equal(&json!(0), &json!(false)));
    }

    // Object tests
    #[test]
    fn test_empty_objects_equal() {
        assert!(deep_equal(&json!({}), &json!({})));
    }

    #[test]
    fn test_equal_objects_same_order() {
        assert!(deep_equal(
            &json!({"a": 1, "b": "2"}),
            &json!({"a": 1, "b": "2"})
        ));
    }

    #[test]
    fn test_equal_objects_different_order() {
        assert!(deep_equal(
            &json!({"a": 1, "b": "2"}),
            &json!({"b": "2", "a": 1})
        ));
    }

    #[test]
    fn test_not_equal_objects_extra_property() {
        assert!(!deep_equal(
            &json!({"a": 1, "b": "2"}),
            &json!({"a": 1, "b": "2", "c": []})
        ));
    }

    #[test]
    fn test_not_equal_objects_different_values() {
        assert!(!deep_equal(
            &json!({"a": 1, "b": "2", "c": 3}),
            &json!({"a": 1, "b": "2", "c": 4})
        ));
    }

    #[test]
    fn test_not_equal_objects_different_properties() {
        assert!(!deep_equal(
            &json!({"a": 1, "b": "2", "c": 3}),
            &json!({"a": 1, "b": "2", "d": 3})
        ));
    }

    #[test]
    fn test_equal_nested_objects() {
        assert!(deep_equal(
            &json!({"a": [{"b": "c"}]}),
            &json!({"a": [{"b": "c"}]})
        ));
    }

    #[test]
    fn test_empty_object_and_array_not_equal() {
        assert!(!deep_equal(&json!({}), &json!([])));
    }

    // Array tests
    #[test]
    fn test_empty_arrays_equal() {
        assert!(deep_equal(&json!([]), &json!([])));
    }

    #[test]
    fn test_equal_arrays() {
        assert!(deep_equal(&json!([1, 2, 3]), &json!([1, 2, 3])));
    }

    #[test]
    fn test_not_equal_arrays_different_item() {
        assert!(!deep_equal(&json!([1, 2, 3]), &json!([1, 2, 4])));
    }

    #[test]
    fn test_not_equal_arrays_different_length() {
        assert!(!deep_equal(&json!([1, 2, 3]), &json!([1, 2])));
    }

    #[test]
    fn test_equal_arrays_of_objects() {
        assert!(deep_equal(
            &json!([{"a": "a"}, {"b": "b"}]),
            &json!([{"a": "a"}, {"b": "b"}])
        ));
    }

    #[test]
    fn test_not_equal_arrays_of_objects() {
        assert!(!deep_equal(
            &json!([{"a": "a"}, {"b": "b"}]),
            &json!([{"a": "a"}, {"b": "c"}])
        ));
    }

    // Binary tests
    #[test]
    fn test_binary_equal() {
        let a = JsonBinary::Binary(vec![1, 2, 3]);
        let b = JsonBinary::Binary(vec![1, 2, 3]);
        assert!(deep_equal_binary(&a, &b));
    }

    #[test]
    fn test_binary_not_equal() {
        let a = JsonBinary::Binary(vec![1, 2, 3]);
        let b = JsonBinary::Binary(vec![1, 2, 4]);
        assert!(!deep_equal_binary(&a, &b));
    }

    #[test]
    fn test_empty_binary_equal() {
        let a = JsonBinary::Binary(vec![]);
        let b = JsonBinary::Binary(vec![]);
        assert!(deep_equal_binary(&a, &b));
    }

    #[test]
    fn test_binary_and_array_not_equal() {
        let a = JsonBinary::Binary(vec![]);
        let b = JsonBinary::Array(vec![]);
        assert!(!deep_equal_binary(&a, &b));
    }

    // Complex tests
    #[test]
    fn test_big_object() {
        let a = json!({
            "prop1": "value1",
            "prop2": "value2",
            "prop3": "value3",
            "prop4": {
                "subProp1": "sub value1",
                "subProp2": {
                    "subSubProp1": "sub sub value1",
                    "subSubProp2": [1, 2, {"prop2": 1, "prop": 2}, 4, 5]
                }
            },
            "prop5": 1000
        });
        let b = json!({
            "prop5": 1000,
            "prop3": "value3",
            "prop1": "value1",
            "prop2": "value2",
            "prop4": {
                "subProp2": {
                    "subSubProp1": "sub sub value1",
                    "subSubProp2": [1, 2, {"prop2": 1, "prop": 2}, 4, 5]
                },
                "subProp1": "sub value1"
            }
        });
        assert!(deep_equal(&a, &b));
    }
}
