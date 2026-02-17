use serde_json::Value;

/// Computes the UTF-8 size of a string in bytes.
///
/// # Examples
///
/// ```
/// use json_joy_util::json_size::utf8_size;
///
/// assert_eq!(utf8_size("hello"), 5);
/// assert_eq!(utf8_size("hello\x00"), 6);  // null byte
/// assert_eq!(utf8_size("héllo"), 6);      // é is 2 bytes in UTF-8
/// ```
pub fn utf8_size(s: &str) -> usize {
    s.len()
}

/// Computes the JSON-encoded size of a number.
fn number_size(n: &serde_json::Number) -> usize {
    // Use serde_json's to_string to get exact representation
    n.to_string().len()
}

/// Computes the JSON-encoded size of a string including quotes and escape sequences.
fn string_size(s: &str) -> usize {
    let mut size = 2; // Opening and closing quotes

    for ch in s.chars() {
        match ch {
            // Escape sequences add one extra character
            '\u{0008}' | // \b
            '\t' |       // \t
            '\n' |       // \n
            '\u{000C}' | // \f
            '\r' |       // \r
            '"' |        // \"
            '\\' =>      // \\
            {
                size += 2;
            }
            // Control characters need \uXXXX encoding
            c if c.is_control() => {
                size += 6; // \uXXXX
            }
            // ASCII characters
            c if (c as u32) < 128 => {
                size += 1;
            }
            // Multi-byte UTF-8 characters
            c => {
                size += c.len_utf8();
            }
        }
    }

    size
}

/// Computes the exact JSON size as would be output from `serde_json::to_string()`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_size::json_size;
///
/// assert_eq!(json_size(&json!(null)), 4);
/// assert_eq!(json_size(&json!(true)), 4);
/// assert_eq!(json_size(&json!(false)), 5);
/// assert_eq!(json_size(&json!("hello")), 7); // "hello"
/// assert_eq!(json_size(&json!(123)), 3);
/// ```
pub fn json_size(value: &Value) -> usize {
    match value {
        Value::Null => 4, // "null"
        Value::Bool(b) => {
            if *b {
                4 // "true"
            } else {
                5 // "false"
            }
        }
        Value::Number(n) => number_size(n),
        Value::String(s) => string_size(s),
        Value::Array(arr) => {
            let mut size = 2; // [ ]
            let len = arr.len();
            for (i, elem) in arr.iter().enumerate() {
                size += json_size(elem);
                if i < len - 1 {
                    size += 1; // comma
                }
            }
            size
        }
        Value::Object(obj) => {
            let mut size = 2; // { }
            let len = obj.len();
            for (i, (key, val)) in obj.iter().enumerate() {
                size += string_size(key); // key with quotes
                size += 1; // colon
                size += json_size(val);
                if i < len - 1 {
                    size += 1; // comma
                }
            }
            size
        }
    }
}

/// Approximates the JSON size, using string length instead of exact UTF-8 calculation.
///
/// This is faster than `json_size` but may underestimate the size for strings
/// with escape sequences or overestimate for ASCII-only strings.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use json_joy_util::json_size::json_size_approx;
///
/// let value = json!({"name": "test"});
/// let size = json_size_approx(&value);
/// assert!(size > 0);
/// ```
pub fn json_size_approx(value: &Value) -> usize {
    match value {
        Value::Null => 4,
        Value::Bool(b) => {
            if *b {
                4
            } else {
                5
            }
        }
        Value::Number(n) => number_size(n),
        Value::String(s) => s.len() + 2, // Approximate: length + 2 quotes
        Value::Array(arr) => {
            let mut size = 2;
            let len = arr.len();
            for (i, elem) in arr.iter().enumerate() {
                size += json_size_approx(elem);
                if i < len - 1 {
                    size += 1;
                }
            }
            size
        }
        Value::Object(obj) => {
            let mut size = 2;
            let len = obj.len();
            for (i, (key, val)) in obj.iter().enumerate() {
                size += key.len() + 2; // Approximate key
                size += 1; // colon
                size += json_size_approx(val);
                if i < len - 1 {
                    size += 1; // comma
                }
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
    fn test_utf8_size_ascii() {
        assert_eq!(utf8_size("hello"), 5);
        assert_eq!(utf8_size(""), 0);
    }

    #[test]
    fn test_utf8_size_unicode() {
        assert_eq!(utf8_size("héllo"), 6); // é is 2 bytes
        assert_eq!(utf8_size("日本語"), 9); // Each char is 3 bytes
    }

    #[test]
    fn test_json_size_null() {
        assert_eq!(json_size(&json!(null)), 4);
    }

    #[test]
    fn test_json_size_bool() {
        assert_eq!(json_size(&json!(true)), 4);
        assert_eq!(json_size(&json!(false)), 5);
    }

    #[test]
    fn test_json_size_number() {
        assert_eq!(json_size(&json!(123)), 3);
        assert_eq!(json_size(&json!(0)), 1);
        assert_eq!(json_size(&json!(-123)), 4);
    }

    #[test]
    fn test_json_size_string() {
        assert_eq!(json_size(&json!("hello")), 7); // "hello"
        assert_eq!(json_size(&json!("")), 2); // ""
        assert_eq!(json_size(&json!("a\tb")), 6); // "a\tb" = "a\tb"
    }

    #[test]
    fn test_json_size_array() {
        assert_eq!(json_size(&json!([])), 2);
        assert_eq!(json_size(&json!([1, 2, 3])), 7); // [1,2,3] = 7 chars
    }

    #[test]
    fn test_json_size_object() {
        assert_eq!(json_size(&json!({})), 2);
        assert_eq!(json_size(&json!({"a": 1})), 7); // {"a":1}
    }

    #[test]
    fn test_json_size_complex() {
        let value = json!({
            "name": "test",
            "values": [1, 2, 3],
            "nested": {"a": true}
        });
        let serialized = serde_json::to_string(&value).unwrap();
        assert_eq!(json_size(&value), serialized.len());
    }

    #[test]
    fn test_json_size_approx_reasonable() {
        let value = json!({"name": "test"});
        let exact = json_size(&value);
        let approx = json_size_approx(&value);
        // Approx should be close to exact
        assert!(approx >= exact - 2 && approx <= exact + 2);
    }
}
