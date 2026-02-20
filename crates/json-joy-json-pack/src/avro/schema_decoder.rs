//! Apache Avro schema-aware decoder.
//!
//! Upstream reference: `json-pack/src/avro/AvroSchemaDecoder.ts`

use std::collections::HashMap;

use super::decoder::{AvroDecodeError, AvroDecoder};
use super::schema_validator::AvroSchemaValidator;
use super::types::{AvroSchema, AvroValue};

/// Apache Avro schema-aware decoder.
pub struct AvroSchemaDecoder {
    decoder: AvroDecoder,
    validator: AvroSchemaValidator,
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
            validator: AvroSchemaValidator::new(),
            named: HashMap::new(),
        }
    }

    pub fn decode(
        &mut self,
        data: &[u8],
        schema: &AvroSchema,
    ) -> Result<AvroValue, AvroDecodeError> {
        self.named.clear();
        if !self.validator.validate_schema(schema) {
            return Err(AvroDecodeError::InvalidSchema);
        }
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
                    .ok_or(AvroDecodeError::InvalidEnumIndex(idx))?;
                Ok(AvroValue::Enum(sym))
            }
            AvroSchema::Array { items } => self.read_array_value(items),
            AvroSchema::Map { values } => self.read_map_value(values),
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

    fn read_array_value(&mut self, item_schema: &AvroSchema) -> Result<AvroValue, AvroDecodeError> {
        let item_schema = item_schema.clone();
        let mut items = Vec::new();
        loop {
            let count = self.decoder.read_varint_u32()? as usize;
            if count == 0 {
                break;
            }
            for _ in 0..count {
                items.push(self.read_value(&item_schema)?);
            }
        }
        Ok(AvroValue::Array(items))
    }

    fn read_map_value(&mut self, value_schema: &AvroSchema) -> Result<AvroValue, AvroDecodeError> {
        let value_schema = value_schema.clone();
        let mut entries = Vec::new();
        loop {
            let count = self.decoder.read_varint_u32()? as usize;
            if count == 0 {
                break;
            }
            for _ in 0..count {
                let key = self.decoder.read_str()?;
                if key == "__proto__" {
                    return Err(AvroDecodeError::InvalidKey);
                }
                entries.push((key, self.read_value(&value_schema)?));
            }
        }
        Ok(AvroValue::Map(entries))
    }
}
