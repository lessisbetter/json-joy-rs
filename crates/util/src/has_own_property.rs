use serde_json::Map;
use std::collections::{BTreeMap, HashMap};

/// Check if an object has an own property with the given key.
///
/// This is the Rust equivalent of `Object.prototype.hasOwnProperty.call(obj, key)`.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use json_joy_util::has_own_property::has_own_property;
///
/// let mut map = BTreeMap::new();
/// map.insert("foo".to_string(), 1);
/// map.insert("bar".to_string(), 2);
///
/// assert!(has_own_property(&map, "foo"));
/// assert!(!has_own_property(&map, "baz"));
/// ```
pub fn has_own_property<V>(obj: &BTreeMap<String, V>, key: &str) -> bool {
    obj.contains_key(key)
}

/// Check if a HashMap has an own property with the given key.
pub fn has_own_property_hashmap<V>(obj: &HashMap<String, V>, key: &str) -> bool {
    obj.contains_key(key)
}

/// Check if a serde_json::Map has an own property with the given key.
pub fn has_own_property_map(obj: &Map<String, serde_json::Value>, key: &str) -> bool {
    obj.contains_key(key)
}

/// Check if a serde_json::Value (object) has an own property with the given key.
/// Returns false if the value is not an object.
pub fn has_own_property_value(obj: &serde_json::Value, key: &str) -> bool {
    match obj {
        serde_json::Value::Object(map) => map.contains_key(key),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has_own_property_btree() {
        let mut map = BTreeMap::new();
        map.insert("foo".to_string(), 1);
        map.insert("bar".to_string(), 2);

        assert!(has_own_property(&map, "foo"));
        assert!(has_own_property(&map, "bar"));
        assert!(!has_own_property(&map, "baz"));
    }

    #[test]
    fn test_has_own_property_hashmap() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), 1);
        map.insert("bar".to_string(), 2);

        assert!(has_own_property_hashmap(&map, "foo"));
        assert!(has_own_property_hashmap(&map, "bar"));
        assert!(!has_own_property_hashmap(&map, "baz"));
    }

    #[test]
    fn test_has_own_property_map() {
        let mut map = Map::new();
        map.insert("foo".to_string(), json!(1));
        map.insert("bar".to_string(), json!(2));

        assert!(has_own_property_map(&map, "foo"));
        assert!(has_own_property_map(&map, "bar"));
        assert!(!has_own_property_map(&map, "baz"));
    }

    #[test]
    fn test_has_own_property_value() {
        let obj = json!({"foo": "bar", "baz": 42});

        assert!(has_own_property_value(&obj, "foo"));
        assert!(has_own_property_value(&obj, "baz"));
        assert!(!has_own_property_value(&obj, "qux"));

        // Non-object values return false
        assert!(!has_own_property_value(&json!(null), "foo"));
        assert!(!has_own_property_value(&json!(42), "foo"));
        assert!(!has_own_property_value(&json!("string"), "foo"));
        assert!(!has_own_property_value(&json!([1, 2, 3]), "length"));
    }
}
