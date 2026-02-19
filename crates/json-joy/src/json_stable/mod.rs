//! json-stable — deterministic JSON serialization with sorted object keys.
//!
//! Mirrors `packages/json-joy/src/json-stable/index.ts`.
//!
//! Unlike standard JSON serialization, this implementation sorts object keys
//! using insertion sort before serializing, ensuring a deterministic output
//! regardless of the order keys were inserted into the map.

use json_joy_util::insertion_sort_by;
use json_joy_util::strings::escape;
use serde_json::Value;

/// Serialize `value` to a deterministic JSON string with sorted object keys.
///
/// Objects have their keys sorted using insertion sort (stable, optimized for
/// small arrays — matches upstream behaviour). All other values follow standard
/// JSON serialization rules.
pub fn stringify(val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{}\"", escape(s)),
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_owned();
            }
            let mut s = String::from('[');
            let last = arr.len() - 1;
            for (i, item) in arr.iter().enumerate() {
                s.push_str(&stringify(item));
                if i < last {
                    s.push(',');
                }
            }
            s.push(']');
            s
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                return "{}".to_owned();
            }
            // Collect and sort keys with insertion sort (matches upstream)
            let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
            insertion_sort_by(&mut keys, |a, b| a.cmp(b));
            let mut s = String::from('{');
            let last = keys.len() - 1;
            for (i, key) in keys.iter().enumerate() {
                let prop_val = stringify(&obj[*key]);
                s.push('"');
                s.push_str(&escape(key));
                s.push_str("\":");
                s.push_str(&prop_val);
                if i < last {
                    s.push(',');
                }
            }
            s.push('}');
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_value() {
        assert_eq!(stringify(&json!(null)), "null");
    }

    #[test]
    fn bool_values() {
        assert_eq!(stringify(&json!(true)), "true");
        assert_eq!(stringify(&json!(false)), "false");
    }

    #[test]
    fn number_values() {
        assert_eq!(stringify(&json!(42)), "42");
        assert_eq!(stringify(&json!(-1)), "-1");
        assert_eq!(stringify(&json!(3.14)), "3.14");
    }

    #[test]
    fn string_value() {
        assert_eq!(stringify(&json!("hello")), r#""hello""#);
        assert_eq!(stringify(&json!("say \"hi\"")), r#""say \"hi\"""#);
    }

    #[test]
    fn empty_array() {
        assert_eq!(stringify(&json!([])), "[]");
    }

    #[test]
    fn array_values() {
        assert_eq!(stringify(&json!([1, 2, 3])), "[1,2,3]");
    }

    #[test]
    fn empty_object() {
        assert_eq!(stringify(&json!({})), "{}");
    }

    #[test]
    fn object_keys_sorted() {
        // Keys "b", "a", "c" → should appear in sorted order "a","b","c"
        let val = json!({"b": 2, "a": 1, "c": 3});
        assert_eq!(stringify(&val), r#"{"a":1,"b":2,"c":3}"#);
    }

    #[test]
    fn nested_object() {
        let val = json!({"z": {"b": 2, "a": 1}, "a": [3, 1, 2]});
        assert_eq!(stringify(&val), r#"{"a":[3,1,2],"z":{"a":1,"b":2}}"#);
    }
}
