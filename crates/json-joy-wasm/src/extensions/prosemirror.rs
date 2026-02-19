use json_joy::json_crdt_extensions::peritext::rga::Anchor;
use json_joy::json_crdt_extensions::peritext::slice::constants::{
    SliceStacking, HEADER_STACKING_SHIFT,
};
use serde_json::{json, Value};

use crate::extensions::types::PmNode;

fn header(stacking: SliceStacking, x1: Anchor, x2: Anchor) -> u64 {
    ((stacking as u64) << HEADER_STACKING_SHIFT) | ((x1 as u64) << 0) | ((x2 as u64) << 1)
}

fn inline_header(stacking: SliceStacking) -> u64 {
    header(stacking, Anchor::Before, Anchor::After)
}

fn marker_header() -> u64 {
    header(SliceStacking::Marker, Anchor::Before, Anchor::Before)
}

struct ProseMirrorConverter {
    text: String,
    slices: Vec<Value>,
}

impl ProseMirrorConverter {
    fn new() -> Self {
        Self {
            text: String::new(),
            slices: Vec::new(),
        }
    }

    fn node_type_name(node: &PmNode) -> String {
        node.node_type.name.clone()
    }

    fn content(node: &PmNode) -> Vec<PmNode> {
        node.content
            .as_ref()
            .map(|f| f.content.clone())
            .unwrap_or_default()
    }

    fn conv(&mut self, node: &PmNode, path: &[Value], node_discriminator: usize) {
        let type_name = Self::node_type_name(node);

        let start = self.text.chars().count();
        let mut inline_text = String::new();

        if type_name == "text" {
            if let Some(text) = &node.text {
                inline_text.push_str(text);
                if !inline_text.is_empty() {
                    self.text.push_str(&inline_text);
                }
            }
        }

        if inline_text.is_empty() {
            let content = Self::content(node);
            let data = node
                .attrs
                .as_ref()
                .map(|m| serde_json::to_value(m).unwrap_or(Value::Null));

            let step = if node_discriminator != 0 || data.is_some() {
                Value::Array(vec![
                    Value::from(type_name.clone()),
                    Value::from(node_discriminator as u64),
                    data.unwrap_or(Value::Null),
                ])
            } else {
                Value::from(type_name.clone())
            };

            let has_no_children = content.is_empty();
            let first_is_inline = content
                .first()
                .map(Self::node_type_name)
                .map(|t| t == "text")
                .unwrap_or(false);

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

            if !content.is_empty() {
                let mut next_path = path.to_vec();
                next_path.push(step);
                self.cont(&next_path, &content);
            }
        }

        if let Some(marks) = &node.marks {
            let end = start + inline_text.chars().count();
            for mark in marks {
                let mark_type = &mark.mark_type.name;

                let attrs = mark
                    .attrs
                    .as_ref()
                    .map(|m| serde_json::to_value(m).unwrap_or_else(|_| json!({})))
                    .unwrap_or_else(|| json!({}));
                let data_empty = mark.attrs.as_ref().map(|m| m.is_empty()).unwrap_or(true);
                let stacking = if data_empty {
                    SliceStacking::One
                } else {
                    SliceStacking::Many
                };

                let mut slice = vec![
                    Value::from(inline_header(stacking)),
                    Value::from(start as u64),
                    Value::from(end as u64),
                    Value::from(mark_type.to_string()),
                ];
                if !data_empty {
                    slice.push(attrs);
                }
                self.slices.push(Value::Array(slice));
            }
        }
    }

    fn cont(&mut self, path: &[Value], content: &[PmNode]) {
        let mut prev_tag = String::new();
        let mut discriminator = 0usize;

        for child in content {
            let tag = Self::node_type_name(child);
            discriminator = if tag == prev_tag {
                discriminator + 1
            } else {
                0
            };
            self.conv(child, path, discriminator);
            prev_tag = tag;
        }
    }

    fn convert(mut self, node: &PmNode) -> Value {
        let content = Self::content(node);
        if !content.is_empty() {
            self.cont(&[], &content);
        }
        json!([self.text, 0, self.slices])
    }
}

pub fn from_prosemirror_to_view_range(node: &Value) -> Value {
    match serde_json::from_value::<PmNode>(node.clone()) {
        Ok(parsed) => ProseMirrorConverter::new().convert(&parsed),
        Err(_) => json!(["", 0, []]),
    }
}

#[cfg(test)]
mod tests {
    use super::from_prosemirror_to_view_range;
    use serde_json::json;

    #[test]
    fn converts_simple_prosemirror_document() {
        let node = json!({
            "type": {"name": "doc"},
            "content": {
                "content": [
                    {
                        "type": {"name": "paragraph"},
                        "attrs": {},
                        "content": {
                            "content": [
                                {
                                    "type": {"name": "text"},
                                    "text": "abc",
                                    "marks": [
                                        {"type": {"name": "strong"}, "attrs": {}}
                                    ]
                                }
                            ]
                        }
                    }
                ]
            }
        });

        let view = from_prosemirror_to_view_range(&node);
        assert_eq!(view[0].as_str().unwrap(), "\nabc");
        assert_eq!(view[2].as_array().unwrap().len(), 2);
    }

    #[test]
    fn handles_empty_content() {
        let node = json!({
            "type": {"name": "doc"},
            "content": {"content": []}
        });
        let view = from_prosemirror_to_view_range(&node);
        assert_eq!(view[0].as_str().unwrap(), "");
        assert!(view[2].as_array().unwrap().is_empty());
    }
}
