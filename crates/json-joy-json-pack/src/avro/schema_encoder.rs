//! Apache Avro schema-aware encoder.
//!
//! Upstream reference: `json-pack/src/avro/AvroSchemaEncoder.ts`

use std::collections::HashMap;

use super::encoder::AvroEncoder;
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
    #[error("fixed size mismatch")]
    FixedSizeMismatch,
    #[error("unresolved schema reference: {0}")]
    UnresolvedRef(String),
}

/// Apache Avro schema-aware encoder.
pub struct AvroSchemaEncoder {
    encoder: AvroEncoder,
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
            named: HashMap::new(),
        }
    }

    pub fn encode(
        &mut self,
        value: &AvroValue,
        schema: &AvroSchema,
    ) -> Result<Vec<u8>, AvroEncodeError> {
        self.named.clear();
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
            (AvroSchema::Long, AvroValue::Long(n)) => {
                self.encoder.write_long(*n);
                Ok(())
            }
            (AvroSchema::Float, AvroValue::Float(f)) => {
                self.encoder.write_float(*f);
                Ok(())
            }
            (AvroSchema::Double, AvroValue::Double(f)) => {
                self.encoder.write_double(*f);
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
                        .or_else(|| field.default.as_ref())
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
                self.encoder.write_long(arr.len() as i64);
                let items = items.as_ref().clone();
                for item in arr {
                    self.write_value(item, &items)?;
                }
                self.encoder.write_long(0);
                Ok(())
            }
            (AvroSchema::Map { values }, AvroValue::Map(map)) => {
                self.encoder.write_long(map.len() as i64);
                let values = values.as_ref().clone();
                for (key, val) in map {
                    self.encoder.write_str(key);
                    self.write_value(val, &values)?;
                }
                self.encoder.write_long(0);
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
            _ => Err(AvroEncodeError::TypeMismatch("schema/value mismatch")),
        }
    }
}
