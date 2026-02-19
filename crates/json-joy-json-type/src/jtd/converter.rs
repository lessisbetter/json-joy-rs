//! Converts TypeNode to JTD (JSON Type Definition) form.
//!
//! Upstream reference: json-type/src/jtd/converter.ts

use serde_json::Value;
use std::collections::HashMap;

use super::types::{JtdForm, JtdType};
use crate::schema::NumFormat;
use crate::type_def::TypeNode;

fn num_format_to_jtd(format: NumFormat) -> Option<JtdType> {
    match format {
        NumFormat::U8 => Some(JtdType::Uint8),
        NumFormat::U16 => Some(JtdType::Uint16),
        NumFormat::U32 => Some(JtdType::Uint32),
        NumFormat::I8 => Some(JtdType::Int8),
        NumFormat::I16 => Some(JtdType::Int16),
        NumFormat::I32 => Some(JtdType::Int32),
        NumFormat::F32 => Some(JtdType::Float32),
        _ => None,
    }
}

fn const_num_to_jtd(n: f64) -> JtdForm {
    if n != n.round() {
        return JtdForm::Type {
            type_: JtdType::Float64,
        };
    }
    if n >= 0.0 {
        if n <= 255.0 {
            return JtdForm::Type {
                type_: JtdType::Uint8,
            };
        }
        if n <= 65535.0 {
            return JtdForm::Type {
                type_: JtdType::Uint16,
            };
        }
        if n <= 4294967295.0 {
            return JtdForm::Type {
                type_: JtdType::Uint32,
            };
        }
        return JtdForm::Type {
            type_: JtdType::Float64,
        };
    }
    if n >= -128.0 {
        return JtdForm::Type {
            type_: JtdType::Int8,
        };
    }
    if n >= -32768.0 {
        return JtdForm::Type {
            type_: JtdType::Int16,
        };
    }
    if n >= -2147483648.0 {
        return JtdForm::Type {
            type_: JtdType::Int32,
        };
    }
    JtdForm::Type {
        type_: JtdType::Float64,
    }
}

/// Convert a `TypeNode` to a JTD form.
///
/// Ports `toJtdForm` from `json-type/src/jtd/converter.ts`.
pub fn to_jtd_form(type_: &TypeNode) -> JtdForm {
    match type_ {
        TypeNode::Any(_) => JtdForm::Empty { nullable: true },

        TypeNode::Bool(_) => JtdForm::Type {
            type_: JtdType::Boolean,
        },

        TypeNode::Con(t) => match &t.value {
            Value::Bool(_) => JtdForm::Type {
                type_: JtdType::Boolean,
            },
            Value::String(_) => JtdForm::Type {
                type_: JtdType::String,
            },
            Value::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                const_num_to_jtd(f)
            }
            _ => JtdForm::Empty { nullable: false },
        },

        TypeNode::Num(t) => {
            let jtd_type = t
                .schema
                .format
                .and_then(num_format_to_jtd)
                .unwrap_or(JtdType::Float64);
            JtdForm::Type { type_: jtd_type }
        }

        TypeNode::Str(_) => JtdForm::Type {
            type_: JtdType::String,
        },

        TypeNode::Arr(t) => {
            if let Some(item_type) = &t.type_ {
                JtdForm::Elements {
                    elements: Box::new(to_jtd_form(item_type)),
                }
            } else {
                JtdForm::Empty { nullable: true }
            }
        }

        TypeNode::Obj(t) => {
            let mut properties: HashMap<String, JtdForm> = HashMap::new();
            let mut optional_properties: HashMap<String, JtdForm> = HashMap::new();
            for key in &t.keys {
                let field_jtd = to_jtd_form(&key.val);
                if key.optional {
                    optional_properties.insert(key.key.clone(), field_jtd);
                } else {
                    properties.insert(key.key.clone(), field_jtd);
                }
            }
            let additional = t.schema.decode_unknown_keys != Some(false);
            JtdForm::Properties {
                properties,
                optional_properties,
                additional_properties: additional,
            }
        }

        TypeNode::Map(t) => JtdForm::Values {
            values: Box::new(to_jtd_form(&t.value)),
        },

        TypeNode::Ref(t) => JtdForm::Ref {
            ref_: t.ref_.clone(),
        },

        TypeNode::Alias(alias) => to_jtd_form(alias.get_type()),

        // Remaining types (or, bin, fn, fn$, key) fall back to empty
        _ => JtdForm::Empty { nullable: false },
    }
}
