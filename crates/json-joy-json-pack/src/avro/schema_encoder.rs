//! Apache Avro schema-aware encoder.
//!
//! Upstream reference: `json-pack/src/avro/AvroSchemaEncoder.ts`

use std::collections::HashMap;

use super::encoder::AvroEncoder;
use super::schema_validator::AvroSchemaValidator;
use super::types::{AvroSchema, AvroValue};

/// Avro encoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AvroEncodeError {
    #[error("type mismatch: expected {0}")]
    TypeMismatch(&'static str),
    #[error("enum symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("required record field missing: {0}")]
    MissingField(String),
    #[error("union value index out of range")]
    UnionIndexOutOfRange,
    #[error("value does not match any union type")]
    UnionNoMatchingType,
    #[error("fixed size mismatch")]
    FixedSizeMismatch,
    #[error("invalid schema")]
    InvalidSchema,
    #[error("value does not conform to schema")]
    ValueDoesNotConform,
    #[error("unresolved schema reference: {0}")]
    UnresolvedRef(String),
}

/// Apache Avro schema-aware encoder.
pub struct AvroSchemaEncoder {
    encoder: AvroEncoder,
    validator: AvroSchemaValidator,
    named: HashMap<String, AvroSchema>,
}

impl Default for AvroSchemaEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AvroSchemaEncoder {
    pub fn new() -> Self {
        Self {
            encoder: AvroEncoder::new(),
            validator: AvroSchemaValidator::new(),
            named: HashMap::new(),
        }
    }

    pub fn encode(
        &mut self,
        value: &AvroValue,
        schema: &AvroSchema,
    ) -> Result<Vec<u8>, AvroEncodeError> {
        self.named.clear();
        if !self.validator.validate_schema(schema) {
            return Err(AvroEncodeError::InvalidSchema);
        }
        if !self.validator.validate_value(value, schema) {
            return Err(AvroEncodeError::ValueDoesNotConform);
        }
        self.collect_named(schema);
        self.write_value(value, schema)?;
        Ok(self.encoder.writer.flush())
    }

    /// Recursively register named types from the schema.
    fn collect_named(&mut self, schema: &AvroSchema) {
        match schema {
            AvroSchema::Record { fields, .. } => {
                if let Some(name) = schema.full_name() {
                    self.named.insert(name, schema.clone());
                }
                for f in fields {
                    self.collect_named(&f.type_);
                }
            }
            AvroSchema::Enum { .. } | AvroSchema::Fixed { .. } => {
                if let Some(name) = schema.full_name() {
                    self.named.insert(name, schema.clone());
                }
            }
            AvroSchema::Array { items } => self.collect_named(items),
            AvroSchema::Map { values } => self.collect_named(values),
            AvroSchema::Union(schemas) => {
                for s in schemas {
                    self.collect_named(s);
                }
            }
            _ => {}
        }
    }

    fn resolve<'a>(&'a self, schema: &'a AvroSchema) -> Result<&'a AvroSchema, AvroEncodeError> {
        if let AvroSchema::Ref(name) = schema {
            self.named
                .get(name)
                .ok_or_else(|| AvroEncodeError::UnresolvedRef(name.clone()))
        } else {
            Ok(schema)
        }
    }

    fn write_value(
        &mut self,
        value: &AvroValue,
        schema: &AvroSchema,
    ) -> Result<(), AvroEncodeError> {
        let schema = self.resolve(schema)?.clone();
        match (&schema, value) {
            (AvroSchema::Null, AvroValue::Null) => Ok(()),
            (AvroSchema::Boolean, AvroValue::Bool(b)) => {
                self.encoder.write_boolean(*b);
                Ok(())
            }
            (AvroSchema::Int, AvroValue::Int(n)) => {
                self.encoder.write_int(*n);
                Ok(())
            }
            (AvroSchema::Int, AvroValue::Long(n)) if i32::try_from(*n).is_ok() => {
                self.encoder.write_int(*n as i32);
                Ok(())
            }
            (AvroSchema::Long, AvroValue::Long(n)) => {
                self.encoder.write_long(*n);
                Ok(())
            }
            (AvroSchema::Long, AvroValue::Int(n)) => {
                self.encoder.write_long(*n as i64);
                Ok(())
            }
            (AvroSchema::Float, AvroValue::Float(f)) => {
                self.encoder.write_float(*f);
                Ok(())
            }
            (AvroSchema::Float, AvroValue::Int(n)) => {
                self.encoder.write_float(*n as f32);
                Ok(())
            }
            (AvroSchema::Float, AvroValue::Long(n)) => {
                self.encoder.write_float(*n as f32);
                Ok(())
            }
            (AvroSchema::Float, AvroValue::Double(f)) => {
                self.encoder.write_float(*f as f32);
                Ok(())
            }
            (AvroSchema::Double, AvroValue::Double(f)) => {
                self.encoder.write_double(*f);
                Ok(())
            }
            (AvroSchema::Double, AvroValue::Float(f)) => {
                self.encoder.write_double(*f as f64);
                Ok(())
            }
            (AvroSchema::Double, AvroValue::Int(n)) => {
                self.encoder.write_double(*n as f64);
                Ok(())
            }
            (AvroSchema::Double, AvroValue::Long(n)) => {
                self.encoder.write_double(*n as f64);
                Ok(())
            }
            (AvroSchema::Bytes, AvroValue::Bytes(b)) => {
                self.encoder.write_bytes(b);
                Ok(())
            }
            (AvroSchema::String, AvroValue::Str(s)) => {
                self.encoder.write_str(s);
                Ok(())
            }
            (AvroSchema::Record { fields, .. }, AvroValue::Record(pairs)) => {
                for field in fields {
                    let val = pairs
                        .iter()
                        .find(|(k, _)| k == &field.name)
                        .map(|(_, v)| v)
                        .or(field.default.as_ref())
                        .ok_or_else(|| AvroEncodeError::MissingField(field.name.clone()))?;
                    self.write_value(val, &field.type_)?;
                }
                Ok(())
            }
            (AvroSchema::Enum { symbols, .. }, AvroValue::Enum(s)) => {
                let idx = symbols
                    .iter()
                    .position(|sym| sym == s)
                    .ok_or_else(|| AvroEncodeError::SymbolNotFound(s.clone()))?;
                self.encoder.write_int(idx as i32);
                Ok(())
            }
            (AvroSchema::Array { items }, AvroValue::Array(arr)) => {
                self.encoder.write_varint_u32(arr.len() as u32);
                let items = items.as_ref().clone();
                for item in arr {
                    self.write_value(item, &items)?;
                }
                self.encoder.write_varint_u32(0);
                Ok(())
            }
            (AvroSchema::Map { values }, AvroValue::Map(map)) => {
                self.encoder.write_varint_u32(map.len() as u32);
                let values = values.as_ref().clone();
                for (key, val) in map {
                    self.encoder.write_str(key);
                    self.write_value(val, &values)?;
                }
                self.encoder.write_varint_u32(0);
                Ok(())
            }
            (AvroSchema::Fixed { size, .. }, AvroValue::Fixed(b)) => {
                if b.len() != *size {
                    return Err(AvroEncodeError::FixedSizeMismatch);
                }
                self.encoder.writer.buf(b);
                Ok(())
            }
            (AvroSchema::Union(schemas), AvroValue::Union { index, value }) => {
                if *index >= schemas.len() {
                    return Err(AvroEncodeError::UnionIndexOutOfRange);
                }
                self.encoder.write_int(*index as i32);
                let s = schemas[*index].clone();
                self.write_value(value, &s)
            }
            (AvroSchema::Union(schemas), other) => {
                for (index, union_schema) in schemas.iter().enumerate() {
                    if self.value_matches_schema(other, union_schema) {
                        self.encoder.write_int(index as i32);
                        return self.write_value(other, union_schema);
                    }
                }
                Err(AvroEncodeError::UnionNoMatchingType)
            }
            _ => Err(AvroEncodeError::TypeMismatch("schema/value mismatch")),
        }
    }

    fn value_matches_schema(&self, value: &AvroValue, schema: &AvroSchema) -> bool {
        let resolved = match self.resolve(schema) {
            Ok(resolved) => resolved,
            Err(_) => return false,
        };

        match (resolved, value) {
            (AvroSchema::Null, AvroValue::Null) => true,
            (AvroSchema::Boolean, AvroValue::Bool(_)) => true,
            (AvroSchema::Int, AvroValue::Int(_)) => true,
            (AvroSchema::Int, AvroValue::Long(n)) => i32::try_from(*n).is_ok(),
            (AvroSchema::Long, AvroValue::Long(_)) => true,
            (AvroSchema::Long, AvroValue::Int(_)) => true,
            (AvroSchema::Float, AvroValue::Float(_))
            | (AvroSchema::Float, AvroValue::Double(_))
            | (AvroSchema::Float, AvroValue::Int(_))
            | (AvroSchema::Float, AvroValue::Long(_)) => true,
            (AvroSchema::Double, AvroValue::Double(_))
            | (AvroSchema::Double, AvroValue::Float(_))
            | (AvroSchema::Double, AvroValue::Int(_))
            | (AvroSchema::Double, AvroValue::Long(_)) => true,
            (AvroSchema::Bytes, AvroValue::Bytes(_)) => true,
            (AvroSchema::String, AvroValue::Str(_)) => true,
            (AvroSchema::Record { fields, .. }, AvroValue::Record(pairs)) => {
                fields.iter().all(|field| {
                    match pairs.iter().find(|(name, _)| name == &field.name) {
                        Some((_, field_value)) => {
                            self.value_matches_schema(field_value, &field.type_)
                        }
                        None => field.default.is_some(),
                    }
                })
            }
            (AvroSchema::Enum { symbols, .. }, AvroValue::Enum(symbol)) => symbols.contains(symbol),
            (AvroSchema::Array { items }, AvroValue::Array(values)) => values
                .iter()
                .all(|value| self.value_matches_schema(value, items)),
            (AvroSchema::Map { values }, AvroValue::Map(entries)) => entries
                .iter()
                .all(|(_, value)| self.value_matches_schema(value, values)),
            (AvroSchema::Fixed { size, .. }, AvroValue::Fixed(bytes)) => bytes.len() == *size,
            (AvroSchema::Union(schemas), AvroValue::Union { index, value }) => schemas
                .get(*index)
                .is_some_and(|schema| self.value_matches_schema(value, schema)),
            (AvroSchema::Union(schemas), value) => schemas
                .iter()
                .any(|schema| self.value_matches_schema(value, schema)),
            _ => false,
        }
    }
}
