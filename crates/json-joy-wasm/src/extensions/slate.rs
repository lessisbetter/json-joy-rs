use json_joy::json_crdt_extensions::peritext::rga::Anchor;
use json_joy::json_crdt_extensions::peritext::slice::constants::{
    SliceStacking, HEADER_STACKING_SHIFT,
};
use serde_json::{json, Map, Value};

fn header(stacking: SliceStacking, x1: Anchor, x2: Anchor) -> u64 {
    ((stacking as u64) << HEADER_STACKING_SHIFT) | ((x1 as u64) << 0) | ((x2 as u64) << 1)
}

fn is_js_falsy(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(false) => true,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i == 0
            } else if let Some(u) = n.as_u64() {
                u == 0
            } else if let Some(f) = n.as_f64() {
                f == 0.0 || f.is_nan()
            } else {
                false
            }
        }
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn inline_header(stacking: SliceStacking) -> u64 {
    header(stacking, Anchor::Before, Anchor::After)
}

fn marker_header() -> u64 {
    header(SliceStacking::Marker, Anchor::Before, Anchor::Before)
}

struct SlateConverter {
    text: String,
    slices: Vec<Value>,
}

impl SlateConverter {
    fn new() -> Self {
        Self {
            text: String::new(),
            slices: Vec::new(),
        }
    }

    fn conv(&mut self, node: &Value, path: &[Value], node_discriminator: usize) {
        let obj = match node.as_object() {
            Some(o) => o,
            None => return,
        };

        let start = self.text.chars().count();

        if let Some(text) = obj.get("text").and_then(Value::as_str) {
            self.text.push_str(text);
            let end = start + text.chars().count();

            for (tag, data) in obj {
                if tag == "text" {
                    continue;
                }
                let data_empty = is_js_falsy(data) || matches!(data, Value::Bool(true));
                let stacking = if data_empty {
                    SliceStacking::One
                } else {
                    SliceStacking::Many
                };
                let mut slice = vec![
                    Value::from(inline_header(stacking)),
                    Value::from(start as u64),
                    Value::from(end as u64),
                    Value::from(tag.clone()),
                ];
                if !data_empty {
                    slice.push(data.clone());
                }
                self.slices.push(Value::Array(slice));
            }
            return;
        }

        let type_name = match obj.get("type").and_then(Value::as_str) {
            Some(t) => t,
            None => return,
        };

        let children: Vec<Value> = obj
            .get("children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        // Mirrors TS behavior where `{...data}` is always truthy for element nodes.
        let data_obj: Map<String, Value> = obj
            .iter()
            .filter(|(k, _)| *k != "type" && *k != "children")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let step = Value::Array(vec![
            Value::from(type_name.to_string()),
            Value::from(node_discriminator as u64),
            Value::Object(data_obj),
        ]);

        let has_no_children = children.is_empty();
        let first_is_inline = children
            .first()
            .and_then(Value::as_object)
            .and_then(|o| o.get("text"))
            .and_then(Value::as_str)
            .is_some();

        if has_no_children || first_is_inline {
            self.text.push('\n');
            let mut type_path = path.to_vec();
            type_path.push(step.clone());
            self.slices.push(Value::Array(vec![
                Value::from(marker_header()),
                Value::from(start as u64),
                Value::from(start as u64),
                Value::Array(type_path),
            ]));
        }

        if !children.is_empty() {
            let mut next_path = path.to_vec();
            next_path.push(step);
            self.cont(&next_path, &children);
        }
    }

    fn cont(&mut self, path: &[Value], content: &[Value]) {
        let mut prev_tag = String::new();
        let mut discriminator = 0usize;

        for child in content {
            let tag = child
                .as_object()
                .and_then(|o| o.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            discriminator = if tag == prev_tag {
                discriminator + 1
            } else {
                0
            };
            self.conv(child, path, discriminator);
            prev_tag = tag;
        }
    }

    fn convert(mut self, doc: &Value) -> Value {
        if let Some(nodes) = doc.as_array() {
            if !nodes.is_empty() {
                self.cont(&[], nodes);
            }
        }
        json!([self.text, 0, self.slices])
    }
}

pub fn from_slate_to_view_range(doc: &Value) -> Value {
    SlateConverter::new().convert(doc)
}

#[cfg(test)]
mod tests {
    use super::from_slate_to_view_range;
    use serde_json::json;

    #[test]
    fn converts_simple_slate_document() {
        let doc = json!([
            {
                "type": "paragraph",
                "children": [
                    {"text": "hi", "bold": true},
                    {"text": "!", "italic": {"level": 1}}
                ]
            }
        ]);

        let view = from_slate_to_view_range(&doc);
        let text = view[0].as_str().unwrap();
        let slices = view[2].as_array().unwrap();

        assert_eq!(text, "\nhi!");
        // block marker + bold + italic
        assert_eq!(slices.len(), 3);
    }

    #[test]
    fn emits_marker_for_empty_block() {
        let doc = json!([
            {
                "type": "paragraph",
                "children": []
            }
        ]);

        let view = from_slate_to_view_range(&doc);
        assert_eq!(view[0].as_str().unwrap(), "\n");
        assert_eq!(view[2].as_array().unwrap().len(), 1);
    }
}
