//! XDR schema/value validator.
//!
//! Upstream reference: `json-pack/src/xdr/XdrSchemaValidator.ts`

use super::types::{XdrDiscriminant, XdrSchema, XdrValue};

/// Validates XDR schemas and runtime values against schemas.
pub struct XdrSchemaValidator;

impl Default for XdrSchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl XdrSchemaValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_schema(&self, schema: &XdrSchema) -> bool {
        self.validate_schema_internal(schema)
    }

    pub fn validate_value(&self, value: &XdrValue, schema: &XdrSchema) -> bool {
        self.validate_value_internal(value, schema)
    }

    fn validate_schema_internal(&self, schema: &XdrSchema) -> bool {
        match schema {
            XdrSchema::Void
            | XdrSchema::Int
            | XdrSchema::UnsignedInt
            | XdrSchema::Boolean
            | XdrSchema::Hyper
            | XdrSchema::UnsignedHyper
            | XdrSchema::Float
            | XdrSchema::Double
            | XdrSchema::Quadruple
            | XdrSchema::Opaque(_)
            | XdrSchema::VarOpaque(_)
            | XdrSchema::Str(_)
            | XdrSchema::Const(_) => true,
            XdrSchema::Enum(values) => self.validate_enum_schema(values),
            XdrSchema::Array { element, .. } => self.validate_schema_internal(element),
            XdrSchema::VarArray { element, .. } => self.validate_schema_internal(element),
            XdrSchema::Struct(fields) => self.validate_struct_schema(fields),
            XdrSchema::Union { arms, default } => self.validate_union_schema(arms, default),
            XdrSchema::Optional(inner) => self.validate_schema_internal(inner),
        }
    }

    fn validate_enum_schema(&self, values: &[(String, i32)]) -> bool {
        let mut seen_values = Vec::with_capacity(values.len());
        for (_, value) in values {
            if seen_values.contains(value) {
                return false;
            }
            seen_values.push(*value);
        }
        true
    }

    fn validate_struct_schema(&self, fields: &[(Box<XdrSchema>, String)]) -> bool {
        let mut names: Vec<&str> = Vec::with_capacity(fields.len());
        for (schema, name) in fields {
            if name.is_empty() {
                return false;
            }
            if names.contains(&name.as_str()) {
                return false;
            }
            if !self.validate_schema_internal(schema) {
                return false;
            }
            names.push(name);
        }
        true
    }

    fn validate_union_schema(
        &self,
        arms: &[(XdrDiscriminant, Box<XdrSchema>)],
        default: &Option<Box<XdrSchema>>,
    ) -> bool {
        if arms.is_empty() {
            return false;
        }

        let mut seen_discriminants: Vec<&XdrDiscriminant> = Vec::with_capacity(arms.len());
        for (discriminant, schema) in arms {
            if seen_discriminants.contains(&discriminant) {
                return false;
            }
            if !self.validate_schema_internal(schema) {
                return false;
            }
            seen_discriminants.push(discriminant);
        }

        if let Some(default_schema) = default {
            self.validate_schema_internal(default_schema)
        } else {
            true
        }
    }

    fn validate_value_internal(&self, value: &XdrValue, schema: &XdrSchema) -> bool {
        match schema {
            XdrSchema::Void => matches!(value, XdrValue::Void),
            XdrSchema::Int => matches!(value, XdrValue::Int(_)),
            XdrSchema::UnsignedInt => matches!(value, XdrValue::UnsignedInt(_)),
            XdrSchema::Boolean => matches!(value, XdrValue::Bool(_)),
            XdrSchema::Hyper => matches!(value, XdrValue::Hyper(_)),
            XdrSchema::UnsignedHyper => matches!(value, XdrValue::UnsignedHyper(_)),
            XdrSchema::Float => matches!(value, XdrValue::Float(_)),
            XdrSchema::Double | XdrSchema::Quadruple => matches!(value, XdrValue::Double(_)),
            XdrSchema::Enum(values) => match value {
                XdrValue::Enum(name) => values.iter().any(|(enum_name, _)| enum_name == name),
                _ => false,
            },
            XdrSchema::Opaque(size) => match value {
                XdrValue::Bytes(bytes) => bytes.len() == *size as usize,
                _ => false,
            },
            XdrSchema::VarOpaque(max_size) => match value {
                XdrValue::Bytes(bytes) => {
                    if let Some(max) = max_size {
                        bytes.len() <= *max as usize
                    } else {
                        true
                    }
                }
                _ => false,
            },
            XdrSchema::Str(max_size) => match value {
                XdrValue::Str(s) => {
                    if let Some(max) = max_size {
                        s.len() <= *max as usize
                    } else {
                        true
                    }
                }
                _ => false,
            },
            XdrSchema::Array { element, size } => match value {
                XdrValue::Array(arr) => {
                    arr.len() == *size as usize
                        && arr
                            .iter()
                            .all(|item| self.validate_value_internal(item, element))
                }
                _ => false,
            },
            XdrSchema::VarArray { element, max_size } => match value {
                XdrValue::Array(arr) => {
                    let within_size = if let Some(max) = max_size {
                        arr.len() <= *max as usize
                    } else {
                        true
                    };
                    within_size
                        && arr
                            .iter()
                            .all(|item| self.validate_value_internal(item, element))
                }
                _ => false,
            },
            XdrSchema::Struct(fields) => match value {
                XdrValue::Struct(values) => fields.iter().all(|(field_schema, field_name)| {
                    values
                        .iter()
                        .find(|(name, _)| name == field_name)
                        .is_some_and(|(_, field_value)| {
                            self.validate_value_internal(field_value, field_schema)
                        })
                }),
                _ => false,
            },
            XdrSchema::Union { arms, default } => match value {
                XdrValue::Union(union_value) => {
                    if let Some((_, arm_schema)) = arms
                        .iter()
                        .find(|(discriminant, _)| discriminant == &union_value.discriminant)
                    {
                        self.validate_value_internal(&union_value.value, arm_schema)
                    } else if let Some(default_schema) = default {
                        self.validate_value_internal(&union_value.value, default_schema)
                    } else {
                        false
                    }
                }
                _ => false,
            },
            XdrSchema::Optional(inner) => match value {
                XdrValue::Optional(None) => true,
                XdrValue::Optional(Some(inner_value)) => {
                    self.validate_value_internal(inner_value, inner)
                }
                _ => false,
            },
            XdrSchema::Const(_) => true,
        }
    }
}
