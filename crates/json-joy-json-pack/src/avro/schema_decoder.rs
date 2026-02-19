//! Apache Avro schema-aware decoder.
//!
//! Upstream reference: `json-pack/src/avro/AvroSchemaDecoder.ts`

use std::collections::HashMap;

use super::decoder::{AvroDecodeError, AvroDecoder};
use super::types::{AvroSchema, AvroValue};

/// Apache Avro schema-aware decoder.
pub struct AvroSchemaDecoder {
    decoder: AvroDecoder,
    named: HashMap<String, AvroSchema>,
}

impl Default for AvroSchemaDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AvroSchemaDecoder {
    pub fn new() -> Self {
        Self {
            decoder: AvroDecoder::new(),
            named: HashMap::new(),
        }
    }

    pub fn decode(
        &mut self,
        data: &[u8],
        schema: &AvroSchema,
    ) -> Result<AvroValue, AvroDecodeError> {
        self.named.clear();
        self.collect_named(schema);
        self.decoder.reset(data);
        self.read_value(schema)
    }

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

    fn resolve<'a>(&'a self, schema: &'a AvroSchema) -> &'a AvroSchema {
        if let AvroSchema::Ref(name) = schema {
            self.named.get(name).unwrap_or(schema)
        } else {
            schema
        }
    }

    fn read_value(&mut self, schema: &AvroSchema) -> Result<AvroValue, AvroDecodeError> {
        let schema = self.resolve(schema).clone();
        match &schema {
            AvroSchema::Null => Ok(AvroValue::Null),
            AvroSchema::Boolean => Ok(AvroValue::Bool(self.decoder.read_boolean()?)),
            AvroSchema::Int => Ok(AvroValue::Int(self.decoder.read_int()?)),
            AvroSchema::Long => Ok(AvroValue::Long(self.decoder.read_long()?)),
            AvroSchema::Float => Ok(AvroValue::Float(self.decoder.read_float()?)),
            AvroSchema::Double => Ok(AvroValue::Double(self.decoder.read_double()?)),
            AvroSchema::Bytes => Ok(AvroValue::Bytes(self.decoder.read_bytes()?)),
            AvroSchema::String => Ok(AvroValue::Str(self.decoder.read_str()?)),
            AvroSchema::Record { fields, .. } => {
                let mut pairs = Vec::with_capacity(fields.len());
                for field in fields {
                    let val = self.read_value(&field.type_)?;
                    pairs.push((field.name.clone(), val));
                }
                Ok(AvroValue::Record(pairs))
            }
            AvroSchema::Enum { symbols, .. } => {
                let idx = self.decoder.read_int()?;
                let sym = symbols
                    .get(idx as usize)
                    .cloned()
                    .unwrap_or_else(|| idx.to_string());
                Ok(AvroValue::Enum(sym))
            }
            AvroSchema::Array { items } => {
                let items = items.as_ref().clone();
                let arr = self
                    .decoder
                    .read_array(|dec| Self::read_value_with_named(dec, &items, &HashMap::new()))?;
                Ok(AvroValue::Array(arr))
            }
            AvroSchema::Map { values } => {
                let values = values.as_ref().clone();
                let map = self
                    .decoder
                    .read_map(|dec| Self::read_value_with_named(dec, &values, &HashMap::new()))?;
                Ok(AvroValue::Map(map))
            }
            AvroSchema::Fixed { size, .. } => Ok(AvroValue::Fixed(self.decoder.read_fixed(*size)?)),
            AvroSchema::Union(schemas) => {
                let idx = self.decoder.read_union_index()?;
                if idx >= schemas.len() {
                    return Err(AvroDecodeError::UnionIndexOutOfRange);
                }
                let s = schemas[idx].clone();
                let val = self.read_value(&s)?;
                Ok(AvroValue::Union {
                    index: idx,
                    value: Box::new(val),
                })
            }
            AvroSchema::Ref(_) => Err(AvroDecodeError::EndOfInput), // unresolved ref
        }
    }

    /// Helper for closures that need stateless decoding (for Array/Map blocks).
    fn read_value_with_named(
        dec: &mut AvroDecoder,
        schema: &AvroSchema,
        _named: &HashMap<String, AvroSchema>,
    ) -> Result<AvroValue, AvroDecodeError> {
        // For simple primitives in arrays/maps, delegate directly.
        match schema {
            AvroSchema::Null => Ok(AvroValue::Null),
            AvroSchema::Boolean => Ok(AvroValue::Bool(dec.read_boolean()?)),
            AvroSchema::Int => Ok(AvroValue::Int(dec.read_int()?)),
            AvroSchema::Long => Ok(AvroValue::Long(dec.read_long()?)),
            AvroSchema::Float => Ok(AvroValue::Float(dec.read_float()?)),
            AvroSchema::Double => Ok(AvroValue::Double(dec.read_double()?)),
            AvroSchema::Bytes => Ok(AvroValue::Bytes(dec.read_bytes()?)),
            AvroSchema::String => Ok(AvroValue::Str(dec.read_str()?)),
            AvroSchema::Fixed { size, .. } => Ok(AvroValue::Fixed(dec.read_fixed(*size)?)),
            _ => Err(AvroDecodeError::EndOfInput),
        }
    }
}
