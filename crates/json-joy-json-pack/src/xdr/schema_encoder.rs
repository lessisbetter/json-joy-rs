//! XDR schema-aware encoder.
//!
//! Upstream reference: `json-pack/src/xdr/XdrSchemaEncoder.ts`

use super::encoder::XdrEncoder;
use super::types::{XdrDiscriminant, XdrSchema, XdrValue};

/// XDR encoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XdrEncodeError {
    #[error("schema/value type mismatch: expected {0}")]
    TypeMismatch(&'static str),
    #[error("value out of range for schema type")]
    OutOfRange,
    #[error("enum value not found in schema")]
    EnumValueNotFound,
    #[error("required struct field missing: {0}")]
    MissingField(String),
    #[error("no matching union arm for discriminant")]
    NoUnionArm,
    #[error("array size mismatch")]
    ArraySizeMismatch,
    #[error("unsupported XDR type: {0}")]
    UnsupportedType(&'static str),
}

/// XDR schema-aware encoder.
pub struct XdrSchemaEncoder {
    encoder: XdrEncoder,
}

impl Default for XdrSchemaEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl XdrSchemaEncoder {
    pub fn new() -> Self {
        Self {
            encoder: XdrEncoder::new(),
        }
    }

    pub fn encode(
        &mut self,
        value: &XdrValue,
        schema: &XdrSchema,
    ) -> Result<Vec<u8>, XdrEncodeError> {
        self.write_value(value, schema)?;
        Ok(self.encoder.writer.flush())
    }

    fn write_value(&mut self, value: &XdrValue, schema: &XdrSchema) -> Result<(), XdrEncodeError> {
        match schema {
            XdrSchema::Void => Ok(()),
            XdrSchema::Int => {
                if let XdrValue::Int(n) = value {
                    self.encoder.write_int(*n);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("int"))
                }
            }
            XdrSchema::UnsignedInt => {
                if let XdrValue::UnsignedInt(n) = value {
                    self.encoder.write_unsigned_int(*n);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("unsigned_int"))
                }
            }
            XdrSchema::Hyper => {
                if let XdrValue::Hyper(n) = value {
                    self.encoder.write_hyper(*n);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("hyper"))
                }
            }
            XdrSchema::UnsignedHyper => {
                if let XdrValue::UnsignedHyper(n) = value {
                    self.encoder.write_unsigned_hyper(*n);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("unsigned_hyper"))
                }
            }
            XdrSchema::Float => {
                if let XdrValue::Float(f) = value {
                    self.encoder.write_float(*f);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("float"))
                }
            }
            XdrSchema::Double => {
                if let XdrValue::Double(f) = value {
                    self.encoder.write_double(*f);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("double"))
                }
            }
            XdrSchema::Quadruple => Err(XdrEncodeError::UnsupportedType("quadruple")),
            XdrSchema::Boolean => {
                if let XdrValue::Bool(b) = value {
                    self.encoder.write_boolean(*b);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("boolean"))
                }
            }
            XdrSchema::Enum(entries) => {
                if let XdrValue::Enum(name) = value {
                    let n = entries
                        .iter()
                        .find(|(k, _)| k == name)
                        .map(|(_, v)| *v)
                        .ok_or(XdrEncodeError::EnumValueNotFound)?;
                    self.encoder.write_int(n);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("enum"))
                }
            }
            XdrSchema::Opaque(size) => {
                if let XdrValue::Bytes(b) = value {
                    if b.len() != *size as usize {
                        return Err(XdrEncodeError::ArraySizeMismatch);
                    }
                    self.encoder.write_opaque(b);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("opaque"))
                }
            }
            XdrSchema::VarOpaque(max_size) => {
                if let XdrValue::Bytes(b) = value {
                    if let Some(max) = max_size {
                        if b.len() > *max as usize {
                            return Err(XdrEncodeError::OutOfRange);
                        }
                    }
                    self.encoder.write_varlen_opaque(b);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("vopaque"))
                }
            }
            XdrSchema::Str(max_size) => {
                if let XdrValue::Str(s) = value {
                    if let Some(max) = max_size {
                        if s.len() > *max as usize {
                            return Err(XdrEncodeError::OutOfRange);
                        }
                    }
                    self.encoder.write_str(s);
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("string"))
                }
            }
            XdrSchema::Array { element, size } => {
                if let XdrValue::Array(arr) = value {
                    if arr.len() != *size as usize {
                        return Err(XdrEncodeError::ArraySizeMismatch);
                    }
                    for item in arr {
                        self.write_value(item, element)?;
                    }
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("array"))
                }
            }
            XdrSchema::VarArray { element, max_size } => {
                if let XdrValue::Array(arr) = value {
                    if let Some(max) = max_size {
                        if arr.len() > *max as usize {
                            return Err(XdrEncodeError::OutOfRange);
                        }
                    }
                    self.encoder.write_unsigned_int(arr.len() as u32);
                    for item in arr {
                        self.write_value(item, element)?;
                    }
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("varray"))
                }
            }
            XdrSchema::Struct(fields) => {
                if let XdrValue::Struct(pairs) = value {
                    for (field_schema, field_name) in fields {
                        let field_val = pairs
                            .iter()
                            .find(|(k, _)| k == field_name)
                            .map(|(_, v)| v)
                            .ok_or_else(|| XdrEncodeError::MissingField(field_name.clone()))?;
                        self.write_value(field_val, field_schema)?;
                    }
                    Ok(())
                } else {
                    Err(XdrEncodeError::TypeMismatch("struct"))
                }
            }
            XdrSchema::Union { arms, default } => {
                if let XdrValue::Union(u) = value {
                    let disc_int = match &u.discriminant {
                        XdrDiscriminant::Int(n) => *n,
                        XdrDiscriminant::Bool(b) => *b as i32,
                        XdrDiscriminant::Str(_) => {
                            return Err(XdrEncodeError::UnsupportedType(
                                "string union discriminant",
                            ))
                        }
                    };
                    let arm = arms.iter().find(|(d, _)| d == &u.discriminant);
                    let arm_schema = if let Some((_, s)) = arm {
                        s.as_ref()
                    } else if let Some(def) = default {
                        def.as_ref()
                    } else {
                        return Err(XdrEncodeError::NoUnionArm);
                    };
                    self.encoder.write_int(disc_int);
                    self.write_value(&u.value, arm_schema)
                } else {
                    Err(XdrEncodeError::TypeMismatch("union"))
                }
            }
            XdrSchema::Optional(inner) => {
                if let XdrValue::Optional(opt) = value {
                    if let Some(inner_val) = opt {
                        self.encoder.write_boolean(true);
                        self.write_value(inner_val, inner)
                    } else {
                        self.encoder.write_boolean(false);
                        Ok(())
                    }
                } else {
                    Err(XdrEncodeError::TypeMismatch("optional"))
                }
            }
            XdrSchema::Const(_) => Ok(()),
        }
    }
}
