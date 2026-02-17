use super::json_size_fast::MaxEncodingOverhead;
use serde_json::Value;

/// Calculates the maximum encoding capacity for a JSON value.
///
/// This returns the worst-case size estimate for encoding a value,
/// useful for buffer allocation.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_size::max_encoding_capacity;
///
/// assert_eq!(max_encoding_capacity(&json!(null)), 4);
/// assert_eq!(max_encoding_capacity(&json!(true)), 5);
/// assert_eq!(max_encoding_capacity(&json!(42)), 22);
/// ```
pub fn max_encoding_capacity(value: &Value) -> usize {
    match value {
        Value::Null => MaxEncodingOverhead::NULL,
        Value::Bool(_) => MaxEncodingOverhead::BOOLEAN,
        Value::Number(_) => MaxEncodingOverhead::NUMBER,
        Value::String(s) => {
            MaxEncodingOverhead::STRING + s.len() * MaxEncodingOverhead::STRING_LENGTH_MULTIPLIER
        }
        Value::Array(arr) => {
            let mut size = MaxEncodingOverhead::ARRAY + arr.len() * MaxEncodingOverhead::ARRAY_ELEMENT;
            for elem in arr {
                size += max_encoding_capacity(elem);
            }
            size
        }
        Value::Object(obj) => {
            let mut size = MaxEncodingOverhead::OBJECT;
            for (key, val) in obj {
                size += MaxEncodingOverhead::OBJECT_ELEMENT;
                size += max_encoding_capacity(&Value::String(key.clone()));
                size += max_encoding_capacity(val);
            }
            size
        }
    }
}

/// Calculates the maximum encoding capacity for binary data.
pub fn max_encoding_capacity_binary(data: &[u8]) -> usize {
    MaxEncodingOverhead::BINARY + data.len() * MaxEncodingOverhead::BINARY_LENGTH_MULTIPLIER
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_max_encoding_capacity_null() {
        assert_eq!(max_encoding_capacity(&json!(null)), 4);
    }

    #[test]
    fn test_max_encoding_capacity_bool() {
        assert_eq!(max_encoding_capacity(&json!(true)), 5);
        assert_eq!(max_encoding_capacity(&json!(false)), 5);
    }

    #[test]
    fn test_max_encoding_capacity_number() {
        assert_eq!(max_encoding_capacity(&json!(0)), 22);
        assert_eq!(max_encoding_capacity(&json!(123)), 22);
        assert_eq!(max_encoding_capacity(&json!(123.456)), 22);
    }

    #[test]
    fn test_max_encoding_capacity_string() {
        // String: 5 + len * 5
        let empty = max_encoding_capacity(&json!(""));
        assert_eq!(empty, 5);

        let hello = max_encoding_capacity(&json!("hello"));
        assert_eq!(hello, 5 + 5 * 5); // 30
    }

    #[test]
    fn test_max_encoding_capacity_array() {
        // Empty array: 5 + 0 = 5
        let empty = max_encoding_capacity(&json!([]));
        assert_eq!(empty, 5);

        // [1]: 5 + 1 + 22 = 28
        let single = max_encoding_capacity(&json!([1]));
        assert_eq!(single, 28);
    }

    #[test]
    fn test_max_encoding_capacity_object() {
        // Empty object: 5
        let empty = max_encoding_capacity(&json!({}));
        assert_eq!(empty, 5);

        // {"a": 1}: OBJECT(5) + OBJECT_ELEMENT(2) + STRING(5) + len*5(5) + NUMBER(22) = 39
        let single = max_encoding_capacity(&json!({"a": 1}));
        assert_eq!(single, 39);
    }

    #[test]
    fn test_max_encoding_capacity_binary() {
        // Binary: 41 + len * 2
        assert_eq!(max_encoding_capacity_binary(&[]), 41);
        assert_eq!(max_encoding_capacity_binary(&[1, 2, 3]), 41 + 6); // 47
    }
}
