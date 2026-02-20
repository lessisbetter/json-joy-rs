//! Apache Avro schema/value validator.
//!
//! Upstream reference: `json-pack/src/avro/AvroSchemaValidator.ts`
//! Reference: Apache Avro 1.12.0 specification

use std::collections::{HashMap, HashSet};

use super::types::{AvroField, AvroSchema, AvroValue};

/// Validates Avro schemas and Avro runtime values against schemas.
pub struct AvroSchemaValidator {
    named_schemas: HashMap<String, AvroSchema>,
}

impl Default for AvroSchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AvroSchemaValidator {
    pub fn new() -> Self {
        Self {
            named_schemas: HashMap::new(),
        }
    }

    /// Validates an Avro schema and resolves named schema references.
    pub fn validate_schema(&mut self, schema: &AvroSchema) -> bool {
        self.named_schemas.clear();
        self.validate_schema_internal(schema)
    }

    /// Validates that a value conforms to the given Avro schema.
    pub fn validate_value(&mut self, value: &AvroValue, schema: &AvroSchema) -> bool {
        self.named_schemas.clear();
        let _ = self.validate_schema_internal(schema);
        self.validate_value_against_schema(value, schema)
    }

    fn validate_schema_internal(&mut self, schema: &AvroSchema) -> bool {
        match schema {
            AvroSchema::Null
            | AvroSchema::Boolean
            | AvroSchema::Int
            | AvroSchema::Long
            | AvroSchema::Float
            | AvroSchema::Double
            | AvroSchema::Bytes
            | AvroSchema::String => true,
            AvroSchema::Ref(name) => self.validate_ref_schema(name),
            AvroSchema::Union(schemas) => self.validate_union_schema(schemas),
            AvroSchema::Record {
                name,
                namespace,
                fields,
                ..
            } => self.validate_record_schema(schema, name, namespace.as_deref(), fields),
            AvroSchema::Enum {
                name,
                namespace,
                symbols,
                default,
                ..
            } => self.validate_enum_schema(schema, name, namespace.as_deref(), symbols, default),
            AvroSchema::Array { items } => self.validate_schema_internal(items),
            AvroSchema::Map { values } => self.validate_schema_internal(values),
            AvroSchema::Fixed {
                name,
                namespace,
                size,
                ..
            } => self.validate_fixed_schema(schema, name, namespace.as_deref(), *size),
        }
    }

    fn validate_ref_schema(&self, schema: &str) -> bool {
        matches!(
            schema,
            "null" | "boolean" | "int" | "long" | "float" | "double" | "bytes" | "string"
        ) || self.named_schemas.contains_key(schema)
    }

    fn validate_union_schema(&mut self, schema: &[AvroSchema]) -> bool {
        if schema.is_empty() {
            return false;
        }

        let mut type_set = HashSet::with_capacity(schema.len());
        for sub_schema in schema {
            if !self.validate_schema_internal(sub_schema) {
                return false;
            }
            let type_name = self.schema_type_name(sub_schema);
            if !type_set.insert(type_name) {
                return false;
            }
        }
        true
    }

    fn validate_record_schema(
        &mut self,
        schema: &AvroSchema,
        name: &str,
        namespace: Option<&str>,
        fields: &[AvroField],
    ) -> bool {
        if name.is_empty() {
            return false;
        }

        let full_name = full_name(name, namespace);
        if self.named_schemas.contains_key(&full_name) {
            return false;
        }
        self.named_schemas.insert(full_name, schema.clone());

        let mut field_names = HashSet::with_capacity(fields.len());
        for field in fields {
            if !self.validate_record_field(field) {
                return false;
            }
            if !field_names.insert(field.name.clone()) {
                return false;
            }
        }

        true
    }

    fn validate_record_field(&mut self, field: &AvroField) -> bool {
        !field.name.is_empty() && self.validate_schema_internal(&field.type_)
    }

    fn validate_enum_schema(
        &mut self,
        schema: &AvroSchema,
        name: &str,
        namespace: Option<&str>,
        symbols: &[String],
        default: &Option<String>,
    ) -> bool {
        if name.is_empty() {
            return false;
        }

        let full_name = full_name(name, namespace);
        if self.named_schemas.contains_key(&full_name) {
            return false;
        }
        self.named_schemas.insert(full_name, schema.clone());

        if symbols.is_empty() {
            return false;
        }

        let mut symbol_set = HashSet::with_capacity(symbols.len());
        for symbol in symbols {
            if !symbol_set.insert(symbol.clone()) {
                return false;
            }
        }

        if let Some(default_symbol) = default {
            symbols.iter().any(|symbol| symbol == default_symbol)
        } else {
            true
        }
    }

    fn validate_fixed_schema(
        &mut self,
        schema: &AvroSchema,
        name: &str,
        namespace: Option<&str>,
        _size: usize,
    ) -> bool {
        if name.is_empty() {
            return false;
        }

        let full_name = full_name(name, namespace);
        if self.named_schemas.contains_key(&full_name) {
            return false;
        }
        self.named_schemas.insert(full_name, schema.clone());
        true
    }

    fn validate_value_against_schema(&self, value: &AvroValue, schema: &AvroSchema) -> bool {
        match schema {
            AvroSchema::Ref(name) => self.validate_value_against_ref_schema(value, name),
            AvroSchema::Union(schemas) => {
                if let AvroValue::Union { index, value } = value {
                    schemas
                        .get(*index)
                        .is_some_and(|schema| self.validate_value_against_schema(value, schema))
                } else {
                    schemas
                        .iter()
                        .any(|sub_schema| self.validate_value_against_schema(value, sub_schema))
                }
            }
            AvroSchema::Null => matches!(value, AvroValue::Null),
            AvroSchema::Boolean => matches!(value, AvroValue::Bool(_)),
            AvroSchema::Int => self.validate_int_value(value),
            AvroSchema::Long => self.validate_long_value(value),
            AvroSchema::Float | AvroSchema::Double => self.validate_float_or_double_value(value),
            AvroSchema::Bytes => matches!(value, AvroValue::Bytes(_)),
            AvroSchema::String => matches!(value, AvroValue::Str(_)),
            AvroSchema::Record { fields, .. } => self.validate_value_against_record(value, fields),
            AvroSchema::Enum { symbols, .. } => self.validate_value_against_enum(value, symbols),
            AvroSchema::Array { items } => self.validate_value_against_array(value, items),
            AvroSchema::Map { values } => self.validate_value_against_map(value, values),
            AvroSchema::Fixed { size, .. } => self.validate_value_against_fixed(value, *size),
        }
    }

    fn validate_value_against_ref_schema(&self, value: &AvroValue, schema: &str) -> bool {
        match schema {
            "null" => matches!(value, AvroValue::Null),
            "boolean" => matches!(value, AvroValue::Bool(_)),
            "int" => self.validate_int_value(value),
            "long" => self.validate_long_value(value),
            "float" | "double" => self.validate_float_or_double_value(value),
            "bytes" => matches!(value, AvroValue::Bytes(_)),
            "string" => matches!(value, AvroValue::Str(_)),
            _ => self
                .named_schemas
                .get(schema)
                .is_some_and(|named| self.validate_value_against_schema(value, named)),
        }
    }

    fn validate_value_against_record(
        &self,
        value: &AvroValue,
        schema_fields: &[AvroField],
    ) -> bool {
        let fields = match value {
            AvroValue::Record(fields) => fields,
            _ => return false,
        };

        for field in schema_fields {
            match fields.iter().find(|(name, _)| name == &field.name) {
                Some((_, field_value)) => {
                    if !self.validate_value_against_schema(field_value, &field.type_) {
                        return false;
                    }
                }
                None if field.default.is_none() => return false,
                None => {}
            }
        }
        true
    }

    fn validate_value_against_enum(&self, value: &AvroValue, symbols: &[String]) -> bool {
        match value {
            AvroValue::Enum(symbol) => symbols.iter().any(|candidate| candidate == symbol),
            _ => false,
        }
    }

    fn validate_value_against_array(&self, value: &AvroValue, item_schema: &AvroSchema) -> bool {
        match value {
            AvroValue::Array(items) => items
                .iter()
                .all(|item| self.validate_value_against_schema(item, item_schema)),
            _ => false,
        }
    }

    fn validate_value_against_map(&self, value: &AvroValue, value_schema: &AvroSchema) -> bool {
        match value {
            AvroValue::Map(entries) => entries.iter().all(|(_, entry_value)| {
                self.validate_value_against_schema(entry_value, value_schema)
            }),
            _ => false,
        }
    }

    fn validate_value_against_fixed(&self, value: &AvroValue, size: usize) -> bool {
        match value {
            AvroValue::Fixed(bytes) => bytes.len() == size,
            _ => false,
        }
    }

    fn validate_int_value(&self, value: &AvroValue) -> bool {
        self.number_from_value(value).is_some_and(|number| {
            number.is_finite()
                && number.fract() == 0.0
                && number >= i32::MIN as f64
                && number <= i32::MAX as f64
        })
    }

    fn validate_long_value(&self, value: &AvroValue) -> bool {
        self.number_from_value(value)
            .is_some_and(|number| number.is_finite() && number.fract() == 0.0)
    }

    fn validate_float_or_double_value(&self, value: &AvroValue) -> bool {
        self.number_from_value(value).is_some()
    }

    fn number_from_value(&self, value: &AvroValue) -> Option<f64> {
        match value {
            AvroValue::Int(n) => Some(*n as f64),
            AvroValue::Long(n) => Some(*n as f64),
            AvroValue::Float(n) => Some(*n as f64),
            AvroValue::Double(n) => Some(*n),
            _ => None,
        }
    }

    fn schema_type_name(&self, schema: &AvroSchema) -> String {
        match schema {
            AvroSchema::Null => "null".to_string(),
            AvroSchema::Boolean => "boolean".to_string(),
            AvroSchema::Int => "int".to_string(),
            AvroSchema::Long => "long".to_string(),
            AvroSchema::Float => "float".to_string(),
            AvroSchema::Double => "double".to_string(),
            AvroSchema::Bytes => "bytes".to_string(),
            AvroSchema::String => "string".to_string(),
            AvroSchema::Record { .. } => "record".to_string(),
            AvroSchema::Enum { .. } => "enum".to_string(),
            AvroSchema::Array { .. } => "array".to_string(),
            AvroSchema::Map { .. } => "map".to_string(),
            AvroSchema::Fixed { .. } => "fixed".to_string(),
            AvroSchema::Union(_) => "union".to_string(),
            AvroSchema::Ref(name) => name.clone(),
        }
    }
}

fn full_name(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) if !namespace.is_empty() => format!("{namespace}.{name}"),
        _ => name.to_string(),
    }
}
