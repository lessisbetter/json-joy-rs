//! Compression table importer and value decompressor.
//!
//! Upstream reference: `json-pack/src/util/DecompressionTable.ts`

use crate::{JsonPackExtension, PackValue};

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DecompressionError {
    #[error("table index out of bounds")]
    OutOfBounds,
    #[error("invalid table literal type")]
    InvalidLiteralType,
    #[error("invalid compressed value")]
    InvalidCompressedValue,
}

#[derive(Debug, Default)]
pub struct DecompressionTable {
    table: Vec<PackValue>,
}

impl DecompressionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn import_table(&mut self, rle_table: &[PackValue]) {
        if rle_table.is_empty() {
            return;
        }

        self.table.push(rle_table[0].clone());

        let mut i = 1usize;
        let len = rle_table.len();

        if let PackValue::Integer(first) = rle_table[0] {
            let mut prev = first;
            while i < len {
                if let PackValue::Integer(delta) = rle_table[i] {
                    prev += delta;
                    self.table.push(PackValue::Integer(prev));
                    i += 1;
                } else {
                    break;
                }
            }
        }

        while i < len {
            self.table.push(rle_table[i].clone());
            i += 1;
        }
    }

    pub fn get_literal(&self, index: usize) -> Option<&PackValue> {
        self.table.get(index)
    }

    fn literal_to_key(literal: &PackValue) -> String {
        match literal {
            PackValue::Null => "null".to_owned(),
            PackValue::Undefined => "undefined".to_owned(),
            PackValue::Bool(false) => "false".to_owned(),
            PackValue::Bool(true) => "true".to_owned(),
            PackValue::Integer(i) => i.to_string(),
            PackValue::UInteger(u) => u.to_string(),
            PackValue::Float(f) => {
                if f.is_nan() {
                    "NaN".to_owned()
                } else if f.is_infinite() {
                    if *f > 0.0 {
                        "Infinity".to_owned()
                    } else {
                        "-Infinity".to_owned()
                    }
                } else if *f == 0.0 {
                    "0".to_owned()
                } else {
                    format!("{f}")
                }
            }
            PackValue::BigInt(i) => i.to_string(),
            PackValue::Str(s) => s.clone(),
            PackValue::Bytes(b) => format!("{:?}", b),
            PackValue::Array(_) => "[array]".to_owned(),
            PackValue::Object(_) => "[object]".to_owned(),
            PackValue::Extension(_) => "[extension]".to_owned(),
            PackValue::Blob(_) => "[blob]".to_owned(),
        }
    }

    fn read_index(index_value: &PackValue) -> Result<usize, DecompressionError> {
        match index_value {
            PackValue::Integer(i) if *i >= 0 => {
                usize::try_from(*i).map_err(|_| DecompressionError::InvalidCompressedValue)
            }
            PackValue::UInteger(u) => {
                usize::try_from(*u).map_err(|_| DecompressionError::InvalidCompressedValue)
            }
            _ => Err(DecompressionError::InvalidCompressedValue),
        }
    }

    pub fn decompress(&self, value: &PackValue) -> Result<PackValue, DecompressionError> {
        match value {
            PackValue::Integer(_) | PackValue::UInteger(_) => {
                let index = Self::read_index(value)?;
                self.get_literal(index)
                    .cloned()
                    .ok_or(DecompressionError::OutOfBounds)
            }
            PackValue::Object(obj) => {
                let mut out = Vec::with_capacity(obj.len());
                for (key, val) in obj {
                    let index = key
                        .parse::<usize>()
                        .map_err(|_| DecompressionError::InvalidCompressedValue)?;
                    let key_literal = self
                        .get_literal(index)
                        .ok_or(DecompressionError::OutOfBounds)?;
                    let decompressed_key = Self::literal_to_key(key_literal);
                    out.push((decompressed_key, self.decompress(val)?));
                }
                Ok(PackValue::Object(out))
            }
            PackValue::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(self.decompress(item)?);
                }
                Ok(PackValue::Array(out))
            }
            PackValue::Extension(ext) => {
                let tag_literal = self
                    .get_literal(ext.tag as usize)
                    .ok_or(DecompressionError::OutOfBounds)?;
                let tag = match tag_literal {
                    PackValue::Integer(i) if *i >= 0 => *i as u64,
                    PackValue::UInteger(u) => *u,
                    _ => return Err(DecompressionError::InvalidLiteralType),
                };
                Ok(PackValue::Extension(Box::new(JsonPackExtension::new(
                    tag,
                    self.decompress(&ext.val)?,
                ))))
            }
            other => Ok(other.clone()),
        }
    }
}
