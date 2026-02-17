use serde_json::Value;

/// Maximum encoding overhead constants for different JSON types.
///
/// These values represent the worst-case overhead for encoding each type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxEncodingOverhead;

impl MaxEncodingOverhead {
    /// Literal "null" = 4 bytes
    pub const NULL: usize = 4;
    /// Literal "false" = 5 bytes
    pub const BOOLEAN: usize = 5;
    /// Maximum number literal = 22 bytes (e.g., "1.1111111111111111e+21")
    pub const NUMBER: usize = 22;
    /// String overhead: 1 byte for type, 4 bytes for length
    pub const STRING: usize = 1 + 4;
    /// String length multiplier: 4x UTF-8 overhead + 1.3x Base64 overhead, plus 1 byte for non-ASCII
    pub const STRING_LENGTH_MULTIPLIER: usize = 5;
    /// Binary overhead: 2 quotes + 37 for "data:application/octet-stream;base64,'" + 2 for Base64 padding
    pub const BINARY: usize = 2 + 37 + 2;
    /// Binary length multiplier: 1.3x Base64 overhead
    pub const BINARY_LENGTH_MULTIPLIER: usize = 2;
    /// Array overhead: 1 byte for type, 4 bytes for length
    pub const ARRAY: usize = 1 + 4;
    /// Array element separator: 1 byte for ","
    pub const ARRAY_ELEMENT: usize = 1;
    /// Object overhead: 1 byte for type, 4 bytes for length
    pub const OBJECT: usize = 1 + 4;
    /// Object element: 1 byte for ":" and 1 byte for ","
    pub const OBJECT_ELEMENT: usize = 1 + 1;
    /// Undefined value: Binary + 2x length multiplier
    pub const UNDEFINED: usize = Self::BINARY + Self::BINARY_LENGTH_MULTIPLIER * 2;
}

/// Fast JSON size approximation optimized for MessagePack encoding.
///
/// This function uses heuristics to quickly estimate the encoded size:
///
/// - **Boolean**: 1 byte
/// - **Null**: 1 byte
/// - **Number**: 9 bytes (1 byte type + 8 bytes for the number)
/// - **String**: 4 bytes + string length
/// - **Array**: 2 bytes + sum of element sizes
/// - **Object**: 2 bytes + 2 bytes per key + key length + sum of value sizes
///
/// # Rationale
///
/// - Booleans and `null` are stored as one byte in MessagePack.
/// - Maximum size of a number in MessagePack is 9 bytes.
/// - Maximum overhead for string storage is 4 bytes in MessagePack.
/// - We use 2 bytes for array/object length, assuming most won't exceed 65,535 elements.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_size::json_size_fast;
///
/// assert_eq!(json_size_fast(&json!(null)), 1);
/// assert_eq!(json_size_fast(&json!(true)), 1);
/// assert_eq!(json_size_fast(&json!(42)), 9);
/// ```
pub fn json_size_fast(value: &Value) -> usize {
    match value {
        Value::Null => 1,
        Value::Bool(_) => 1,
        Value::Number(_) => 9,
        Value::String(s) => 4 + s.len(),
        Value::Array(arr) => {
            let mut size = 2; // Array overhead
            for elem in arr {
                size += json_size_fast(elem);
            }
            size
        }
        Value::Object(obj) => {
            let mut size = 2; // Object overhead
            for (key, val) in obj {
                size += 2 + key.len(); // Key overhead + key length
                size += json_size_fast(val);
            }
            size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_size_fast_null() {
        assert_eq!(json_size_fast(&json!(null)), 1);
    }

    #[test]
    fn test_json_size_fast_bool() {
        assert_eq!(json_size_fast(&json!(true)), 1);
        assert_eq!(json_size_fast(&json!(false)), 1);
    }

    #[test]
    fn test_json_size_fast_number() {
        assert_eq!(json_size_fast(&json!(0)), 9);
        assert_eq!(json_size_fast(&json!(123)), 9);
        assert_eq!(json_size_fast(&json!(123.456)), 9);
    }

    #[test]
    fn test_json_size_fast_string() {
        assert_eq!(json_size_fast(&json!("")), 4);
        assert_eq!(json_size_fast(&json!("hello")), 9); // 4 + 5
    }

    #[test]
    fn test_json_size_fast_array() {
        assert_eq!(json_size_fast(&json!([])), 2);
        // [1, 2] = 2 + 9 + 9 = 20
        assert_eq!(json_size_fast(&json!([1, 2])), 20);
    }

    #[test]
    fn test_json_size_fast_object() {
        assert_eq!(json_size_fast(&json!({})), 2);
        // {"a": 1} = 2 + 2 + 1 + 9 = 14
        assert_eq!(json_size_fast(&json!({"a": 1})), 14);
    }

    #[test]
    fn test_max_encoding_overhead_constants() {
        assert_eq!(MaxEncodingOverhead::NULL, 4);
        assert_eq!(MaxEncodingOverhead::BOOLEAN, 5);
        assert_eq!(MaxEncodingOverhead::NUMBER, 22);
        assert_eq!(MaxEncodingOverhead::STRING, 5);
        assert_eq!(MaxEncodingOverhead::ARRAY, 5);
        assert_eq!(MaxEncodingOverhead::OBJECT, 5);
    }
}
