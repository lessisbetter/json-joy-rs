use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

/// Check if an object (map) is empty (has no own properties).
///
/// This is the Rust equivalent of the TypeScript `isEmpty` function that checks
/// if an object has no enumerable own properties.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use json_joy_util::is_empty::is_empty;
///
/// let empty: BTreeMap<String, serde_json::Value> = BTreeMap::new();
///
/// let mut not_empty = BTreeMap::new();
/// not_empty.insert("foo".to_string(), serde_json::json!("bar"));
///
/// assert!(is_empty(&empty));
/// assert!(!is_empty(&not_empty));
/// ```
pub fn is_empty(obj: &BTreeMap<String, serde_json::Value>) -> bool {
    obj.is_empty()
}

/// Check if a HashMap is empty.
pub fn is_empty_hashmap(obj: &HashMap<String, serde_json::Value>) -> bool {
    obj.is_empty()
}

/// Check if a serde_json::Map is empty.
pub fn is_empty_map(obj: &Map<String, serde_json::Value>) -> bool {
    obj.is_empty()
}

/// Check if a serde_json::Value (object) is empty.
/// Returns true for non-object values (they have no properties).
pub fn is_empty_value(obj: &Value) -> bool {
    match obj {
        Value::Object(map) => map.is_empty(),
        // Non-objects are considered "empty" in terms of properties
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_empty_btree() {
        let empty: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        let mut not_empty = BTreeMap::new();
        not_empty.insert("foo".to_string(), json!("bar"));

        assert!(is_empty(&empty));
        assert!(!is_empty(&not_empty));
    }

    #[test]
    fn test_is_empty_hashmap() {
        let empty: HashMap<String, serde_json::Value> = HashMap::new();
        let mut not_empty = HashMap::new();
        not_empty.insert("foo".to_string(), json!("bar"));

        assert!(is_empty_hashmap(&empty));
        assert!(!is_empty_hashmap(&not_empty));
    }

    #[test]
    fn test_is_empty_map() {
        let empty: Map<String, serde_json::Value> = Map::new();
        let mut not_empty = Map::new();
        not_empty.insert("foo".to_string(), json!("bar"));

        assert!(is_empty_map(&empty));
        assert!(!is_empty_map(&not_empty));
    }

    #[test]
    fn test_is_empty_value() {
        // Empty object
        assert!(is_empty_value(&json!({})));

        // Non-empty object
        assert!(!is_empty_value(&json!({"foo": "bar"})));

        // Non-objects are considered empty
        assert!(is_empty_value(&json!(null)));
        assert!(is_empty_value(&json!(42)));
        assert!(is_empty_value(&json!("string")));
        assert!(is_empty_value(&json!([1, 2, 3])));
    }
}
