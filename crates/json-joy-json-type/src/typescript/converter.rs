//! Converts TypeNode to TypeScript AST.
//!
//! Upstream reference: json-type/src/typescript/converter.ts

use serde_json::Value;

use super::types::{TsDeclaration, TsMember, TsParam, TsType};
use crate::type_def::TypeNode;

/// Convert a `TypeNode` to a TypeScript AST type node.
///
/// Ports `toTypeScriptAst` from `json-type/src/typescript/converter.ts`.
pub fn to_typescript_ast(type_: &TypeNode) -> TsType {
    match type_ {
        TypeNode::Any(_) => TsType::Any,

        TypeNode::Bool(_) => TsType::Boolean,

        TypeNode::Num(_) => TsType::Number,

        TypeNode::Str(_) => TsType::String,

        TypeNode::Bin(_) => TsType::TypeReference {
            name: "Uint8Array".into(),
            type_args: vec![],
        },

        TypeNode::Con(t) => match &t.value {
            Value::Bool(true) => TsType::True,
            Value::Bool(false) => TsType::False,
            Value::String(s) => TsType::StringLiteral(s.clone()),
            Value::Number(n) => TsType::NumericLiteral(n.to_string()),
            Value::Null => TsType::Null,
            _ => TsType::Object,
        },

        TypeNode::Arr(t) => {
            // Tuple when head or tail items are present
            if !t.head.is_empty() || !t.tail.is_empty() {
                let mut elements: Vec<TsType> = Vec::new();
                for h in &t.head {
                    elements.push(to_typescript_ast(h));
                }
                if let Some(body) = &t.type_ {
                    elements.push(TsType::Rest(Box::new(to_typescript_ast(body))));
                }
                for tail_item in &t.tail {
                    elements.push(to_typescript_ast(tail_item));
                }
                TsType::Tuple(elements)
            } else if let Some(item_type) = &t.type_ {
                TsType::Array(Box::new(to_typescript_ast(item_type)))
            } else {
                TsType::Array(Box::new(TsType::Unknown))
            }
        }

        TypeNode::Obj(t) => {
            let members: Vec<TsMember> = t
                .keys
                .iter()
                .map(|key| {
                    let comment =
                        build_comment(key.base.title.as_deref(), key.base.description.as_deref());
                    TsMember::Property {
                        name: key.key.clone(),
                        type_: to_typescript_ast(&key.val),
                        optional: key.optional,
                        comment,
                    }
                })
                .collect();
            let mut all_members = members;
            if t.schema.decode_unknown_keys.unwrap_or(false)
                || t.schema.encode_unknown_keys.unwrap_or(false)
            {
                all_members.push(TsMember::Index {
                    type_: TsType::Unknown,
                });
            }
            let comment = {
                let base = &t.schema.base;
                build_comment(base.title.as_deref(), base.description.as_deref())
            };
            TsType::TypeLiteral {
                members: all_members,
                comment,
            }
        }

        TypeNode::Map(t) => TsType::TypeReference {
            name: "Record".into(),
            type_args: vec![TsType::String, to_typescript_ast(&t.value)],
        },

        TypeNode::Or(t) => TsType::Union(t.types.iter().map(to_typescript_ast).collect()),

        TypeNode::Ref(t) => TsType::TypeReference {
            name: t.ref_.clone(),
            type_args: vec![],
        },

        TypeNode::Fn(t) => TsType::FnType {
            params: vec![TsParam {
                name: "request".into(),
                type_: to_typescript_ast(&t.req),
            }],
            return_type: Box::new(TsType::TypeReference {
                name: "Promise".into(),
                type_args: vec![to_typescript_ast(&t.res)],
            }),
        },

        TypeNode::FnRx(t) => TsType::FnType {
            params: vec![TsParam {
                name: "request$".into(),
                type_: TsType::TypeReference {
                    name: "Observable".into(),
                    type_args: vec![to_typescript_ast(&t.req)],
                },
            }],
            return_type: Box::new(TsType::TypeReference {
                name: "Observable".into(),
                type_args: vec![to_typescript_ast(&t.res)],
            }),
        },

        TypeNode::Key(t) => to_typescript_ast(&t.val),

        TypeNode::Alias(alias) => to_typescript_ast(alias.get_type()),
    }
}

/// Build a JSDoc-style comment string from title and description.
fn build_comment(title: Option<&str>, description: Option<&str>) -> Option<String> {
    match (title, description) {
        (None, None) => None,
        (Some(t), None) => Some(t.to_string()),
        (None, Some(d)) => Some(d.to_string()),
        (Some(t), Some(d)) => Some(format!("{}\n\n{}", t, d)),
    }
}

/// Convert an `AliasType` to a top-level TypeScript declaration.
///
/// Ports `aliasToTs` from `json-type/src/typescript/converter.ts`.
pub fn alias_to_ts(type_: &TypeNode, name: &str) -> TsDeclaration {
    match type_ {
        TypeNode::Obj(obj) => {
            let members: Vec<TsMember> = obj
                .keys
                .iter()
                .map(|key| {
                    let comment =
                        build_comment(key.base.title.as_deref(), key.base.description.as_deref());
                    TsMember::Property {
                        name: key.key.clone(),
                        type_: to_typescript_ast(&key.val),
                        optional: key.optional,
                        comment,
                    }
                })
                .collect();
            TsDeclaration::Interface {
                name: name.to_string(),
                members,
                comment: None,
            }
        }
        _ => TsDeclaration::TypeAlias {
            name: name.to_string(),
            type_: to_typescript_ast(type_),
            comment: None,
        },
    }
}
