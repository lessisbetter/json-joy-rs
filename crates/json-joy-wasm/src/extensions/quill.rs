use serde_json::{Map, Value};

fn is_empty_obj(obj: &Map<String, Value>) -> bool {
    obj.is_empty()
}

fn erase_attributes(attr: Option<&Map<String, Value>>) -> Option<Map<String, Value>> {
    let attr = attr?;
    if attr.is_empty() {
        return None;
    }
    let mut erased = Map::new();
    for key in attr.keys() {
        erased.insert(key.clone(), Value::Null);
    }
    Some(erased)
}

pub fn remove_quill_erasures(attr: Option<&Map<String, Value>>) -> Option<Map<String, Value>> {
    let attr = attr?;
    if attr.is_empty() {
        return None;
    }
    let mut cleaned = Map::new();
    for (key, value) in attr {
        if !value.is_null() {
            cleaned.insert(key.clone(), value.clone());
        }
    }
    if is_empty_obj(&cleaned) {
        None
    } else {
        Some(cleaned)
    }
}

pub fn diff_quill_attributes(
    old_attributes: Option<&Map<String, Value>>,
    new_attributes: Option<&Map<String, Value>>,
) -> Option<Map<String, Value>> {
    let old_attributes = old_attributes.filter(|m| !m.is_empty());
    let new_attributes = new_attributes.filter(|m| !m.is_empty());

    match (old_attributes, new_attributes) {
        (None, None) => None,
        (None, Some(new_map)) => remove_quill_erasures(Some(new_map)),
        (Some(old_map), None) => erase_attributes(Some(old_map)),
        (Some(old_map), Some(new_map)) => {
            let mut diff = Map::new();

            for (key, new_value) in new_map {
                match old_map.get(key) {
                    Some(old_value) if old_value == new_value => {}
                    _ => {
                        diff.insert(key.clone(), new_value.clone());
                    }
                }
            }

            for key in old_map.keys() {
                if !new_map.contains_key(key) {
                    diff.insert(key.clone(), Value::Null);
                }
            }

            if diff.is_empty() {
                None
            } else {
                Some(diff)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{diff_quill_attributes, remove_quill_erasures};
    use serde_json::{json, Map, Value};

    fn as_obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn remove_erasures_drops_null_entries() {
        let attrs = as_obj(json!({"bold": true, "color": null, "size": "12px"}));
        let out = remove_quill_erasures(Some(&attrs)).unwrap();
        assert_eq!(out, as_obj(json!({"bold": true, "size": "12px"})));
    }

    #[test]
    fn remove_erasures_returns_none_when_all_null() {
        let attrs = as_obj(json!({"bold": null}));
        assert!(remove_quill_erasures(Some(&attrs)).is_none());
    }

    #[test]
    fn diff_returns_additions_and_changes() {
        let old_attrs = as_obj(json!({"bold": true, "size": "12px"}));
        let new_attrs = as_obj(json!({"bold": true, "size": "14px", "color": "red"}));
        let out = diff_quill_attributes(Some(&old_attrs), Some(&new_attrs)).unwrap();
        assert_eq!(out, as_obj(json!({"size": "14px", "color": "red"})));
    }

    #[test]
    fn diff_returns_null_for_removed_keys() {
        let old_attrs = as_obj(json!({"bold": true, "italic": true}));
        let new_attrs = as_obj(json!({"bold": true}));
        let out = diff_quill_attributes(Some(&old_attrs), Some(&new_attrs)).unwrap();
        assert_eq!(out, as_obj(json!({"italic": null})));
    }

    #[test]
    fn diff_returns_none_when_no_change() {
        let old_attrs = as_obj(json!({"bold": true}));
        let new_attrs = as_obj(json!({"bold": true}));
        assert!(diff_quill_attributes(Some(&old_attrs), Some(&new_attrs)).is_none());
    }
}
