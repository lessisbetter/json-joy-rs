//! XDR schema-aware decoder.
//!
//! Upstream reference: `json-pack/src/xdr/XdrSchemaDecoder.ts`

use super::decoder::{XdrDecodeError, XdrDecoder};
use super::types::{XdrDiscriminant, XdrSchema, XdrUnionValue, XdrValue};

/// XDR schema-aware decoder.
///
/// Decodes XDR wire bytes into [`XdrValue`] using a provided schema.
pub struct XdrSchemaDecoder {
    decoder: XdrDecoder,
}

impl Default for XdrSchemaDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl XdrSchemaDecoder {
    pub fn new() -> Self {
        Self {
            decoder: XdrDecoder::new(),
        }
    }

    pub fn decode(&mut self, data: &[u8], schema: &XdrSchema) -> Result<XdrValue, XdrDecodeError> {
        self.decoder.reset(data);
        self.read_value(schema)
    }

    fn read_value(&mut self, schema: &XdrSchema) -> Result<XdrValue, XdrDecodeError> {
        match schema {
            XdrSchema::Void => Ok(XdrValue::Void),
            XdrSchema::Int => Ok(XdrValue::Int(self.decoder.read_int()?)),
            XdrSchema::UnsignedInt => Ok(XdrValue::UnsignedInt(self.decoder.read_unsigned_int()?)),
            XdrSchema::Hyper => Ok(XdrValue::Hyper(self.decoder.read_hyper()?)),
            XdrSchema::UnsignedHyper => {
                Ok(XdrValue::UnsignedHyper(self.decoder.read_unsigned_hyper()?))
            }
            XdrSchema::Float => Ok(XdrValue::Float(self.decoder.read_float()?)),
            XdrSchema::Double => Ok(XdrValue::Double(self.decoder.read_double()?)),
            XdrSchema::Quadruple => Err(XdrDecodeError::UnsupportedType("quadruple")),
            XdrSchema::Boolean => Ok(XdrValue::Bool(self.decoder.read_boolean()?)),
            XdrSchema::Enum(values) => {
                let n = self.decoder.read_int()?;
                let name = values
                    .iter()
                    .find(|(_, v)| *v == n)
                    .map(|(k, _)| k.clone())
                    .unwrap_or_else(|| n.to_string());
                Ok(XdrValue::Enum(name))
            }
            XdrSchema::Opaque(size) => {
                Ok(XdrValue::Bytes(self.decoder.read_opaque(*size as usize)?))
            }
            XdrSchema::VarOpaque(max_size) => {
                let data = self.decoder.read_varlen_opaque()?;
                if let Some(max) = max_size {
                    if data.len() > *max as usize {
                        return Err(XdrDecodeError::MaxSizeExceeded);
                    }
                }
                Ok(XdrValue::Bytes(data))
            }
            XdrSchema::Str(max_size) => {
                let s = self.decoder.read_string()?;
                if let Some(max) = max_size {
                    if s.len() > *max as usize {
                        return Err(XdrDecodeError::MaxSizeExceeded);
                    }
                }
                Ok(XdrValue::Str(s))
            }
            XdrSchema::Array { element, size } => {
                let mut arr = Vec::with_capacity(*size as usize);
                for _ in 0..*size {
                    arr.push(self.read_value(element)?);
                }
                Ok(XdrValue::Array(arr))
            }
            XdrSchema::VarArray { element, max_size } => {
                let len = self.decoder.read_unsigned_int()? as usize;
                if let Some(max) = max_size {
                    if len > *max as usize {
                        return Err(XdrDecodeError::MaxSizeExceeded);
                    }
                }
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len {
                    arr.push(self.read_value(element)?);
                }
                Ok(XdrValue::Array(arr))
            }
            XdrSchema::Struct(fields) => {
                let mut out = Vec::with_capacity(fields.len());
                for (field_schema, field_name) in fields {
                    let val = self.read_value(field_schema)?;
                    out.push((field_name.clone(), val));
                }
                Ok(XdrValue::Struct(out))
            }
            XdrSchema::Union { arms, default } => {
                let disc_raw = self.decoder.read_int()?;
                let arm = arms.iter().find(|(d, _)| match d {
                    XdrDiscriminant::Int(v) => *v == disc_raw,
                    XdrDiscriminant::Bool(b) => (*b as i32) == disc_raw,
                    XdrDiscriminant::Str(_) => false,
                });
                let arm_schema = if let Some((_, s)) = arm {
                    s.as_ref()
                } else if let Some(def) = default {
                    def.as_ref()
                } else {
                    return Err(XdrDecodeError::UnknownDiscriminant);
                };
                let value = self.read_value(arm_schema)?;
                Ok(XdrValue::Union(Box::new(XdrUnionValue {
                    discriminant: XdrDiscriminant::Int(disc_raw),
                    value,
                })))
            }
            XdrSchema::Optional(inner) => {
                let present = self.decoder.read_boolean()?;
                if present {
                    Ok(XdrValue::Optional(Some(Box::new(self.read_value(inner)?))))
                } else {
                    Ok(XdrValue::Optional(None))
                }
            }
            XdrSchema::Const(v) => Ok(XdrValue::Int(*v)),
        }
    }
}
