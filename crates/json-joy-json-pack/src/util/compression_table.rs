//! Compression table builder and value compressor.
//!
//! Upstream reference: `json-pack/src/util/CompressionTable.ts`

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::{JsonPackExtension, PackValue};

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CompressionError {
    #[error("value not found in compression table")]
    ValueNotFound,
    #[error("unsupported value type for compression")]
    UnsupportedValueType,
    #[error("table index overflow")]
    IndexOverflow,
    #[error("extension tag out of supported range")]
    TagOutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum LiteralKey {
    Null,
    Undefined,
    Bool(bool),
    Str(String),
    FloatBits(u64),
}

struct NonIntegerEntry {
    sort_key: String,
    insertion_order: usize,
    key: LiteralKey,
    value: PackValue,
}

impl LiteralKey {
    fn from_pack(value: &PackValue) -> Result<Self, CompressionError> {
        match value {
            PackValue::Null => Ok(Self::Null),
            PackValue::Undefined => Ok(Self::Undefined),
            PackValue::Bool(b) => Ok(Self::Bool(*b)),
            PackValue::Str(s) => Ok(Self::Str(s.clone())),
            PackValue::Float(f) => {
                // JS Set/Map key semantics for numbers use SameValueZero:
                // NaN equals NaN, and -0 equals +0.
                if f.is_nan() {
                    Ok(Self::FloatBits(u64::MAX))
                } else if *f == 0.0 {
                    Ok(Self::FloatBits(0.0f64.to_bits()))
                } else {
                    Ok(Self::FloatBits(f.to_bits()))
                }
            }
            _ => Err(CompressionError::UnsupportedValueType),
        }
    }
}

fn literal_sort_key(value: &PackValue) -> Result<String, CompressionError> {
    match value {
        PackValue::Null => Ok("null".to_owned()),
        PackValue::Undefined => Ok("undefined".to_owned()),
        PackValue::Bool(false) => Ok("false".to_owned()),
        PackValue::Bool(true) => Ok("true".to_owned()),
        PackValue::Str(s) => Ok(s.clone()),
        PackValue::Float(f) => Ok(if f.is_nan() {
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
        }),
        _ => Err(CompressionError::UnsupportedValueType),
    }
}

fn index_to_pack(index: usize) -> Result<PackValue, CompressionError> {
    let idx = i64::try_from(index).map_err(|_| CompressionError::IndexOverflow)?;
    Ok(PackValue::Integer(idx))
}

#[derive(Debug, Default)]
pub struct CompressionTable {
    integers: BTreeSet<i64>,
    non_integers: Vec<PackValue>,
    non_integer_keys: HashSet<LiteralKey>,
    table: Vec<PackValue>,
    map: HashMap<LiteralKey, usize>,
    integer_map: HashMap<i64, usize>,
}

impl CompressionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(value: &PackValue) -> Result<Self, CompressionError> {
        let mut table = Self::new();
        table.walk(value)?;
        table.finalize()?;
        Ok(table)
    }

    pub fn add_integer(&mut self, int: i64) {
        self.integers.insert(int);
    }

    pub fn add_literal(&mut self, value: &PackValue) -> Result<(), CompressionError> {
        match value {
            PackValue::Integer(i) => {
                self.add_integer(*i);
                Ok(())
            }
            PackValue::UInteger(u) => {
                let i = i64::try_from(*u).map_err(|_| CompressionError::UnsupportedValueType)?;
                self.add_integer(i);
                Ok(())
            }
            PackValue::BigInt(i) => {
                let v = i64::try_from(*i).map_err(|_| CompressionError::UnsupportedValueType)?;
                self.add_integer(v);
                Ok(())
            }
            PackValue::Null
            | PackValue::Undefined
            | PackValue::Bool(_)
            | PackValue::Str(_)
            | PackValue::Float(_) => {
                let key = LiteralKey::from_pack(value)?;
                if self.non_integer_keys.insert(key) {
                    self.non_integers.push(value.clone());
                }
                Ok(())
            }
            _ => Err(CompressionError::UnsupportedValueType),
        }
    }

    pub fn walk(&mut self, value: &PackValue) -> Result<(), CompressionError> {
        match value {
            PackValue::Object(obj) => {
                for (key, val) in obj {
                    self.add_literal(&PackValue::Str(key.clone()))?;
                    self.walk(val)?;
                }
                Ok(())
            }
            PackValue::Array(arr) => {
                for item in arr {
                    self.walk(item)?;
                }
                Ok(())
            }
            PackValue::Extension(ext) => {
                let tag = i64::try_from(ext.tag).map_err(|_| CompressionError::TagOutOfRange)?;
                self.add_integer(tag);
                self.walk(&ext.val)
            }
            // Mirrors upstream behavior: unknown object-like values are ignored during table walk.
            PackValue::Bytes(_) | PackValue::Blob(_) => Ok(()),
            _ => self.add_literal(value),
        }
    }

    pub fn finalize(&mut self) -> Result<(), CompressionError> {
        let integers: Vec<i64> = self.integers.iter().copied().collect();
        let len = integers.len();
        if let Some(first) = integers.first().copied() {
            self.table.push(PackValue::Integer(first));
            self.integer_map.insert(first, 0);
            let mut last = first;
            for (i, int) in integers.iter().copied().enumerate().skip(1) {
                self.table.push(PackValue::Integer(int - last));
                self.integer_map.insert(int, i);
                last = int;
            }
        }

        let mut non_integers: Vec<NonIntegerEntry> = self
            .non_integers
            .iter()
            .cloned()
            .enumerate()
            .map(|(insertion_order, value)| {
                Ok(NonIntegerEntry {
                    sort_key: literal_sort_key(&value)?,
                    insertion_order,
                    key: LiteralKey::from_pack(&value)?,
                    value,
                })
            })
            .collect::<Result<Vec<_>, CompressionError>>()?;
        non_integers.sort_by(|a, b| {
            a.sort_key
                .cmp(&b.sort_key)
                .then(a.insertion_order.cmp(&b.insertion_order))
        });

        for (offset, entry) in non_integers.into_iter().enumerate() {
            let index = len + offset;
            self.table.push(entry.value);
            self.map.insert(entry.key, index);
        }

        self.integers.clear();
        self.non_integers.clear();
        self.non_integer_keys.clear();
        Ok(())
    }

    pub fn get_index(&self, value: &PackValue) -> Result<usize, CompressionError> {
        match value {
            PackValue::Integer(i) => self
                .integer_map
                .get(i)
                .copied()
                .ok_or(CompressionError::ValueNotFound),
            PackValue::UInteger(u) => {
                let i = i64::try_from(*u).map_err(|_| CompressionError::ValueNotFound)?;
                self.integer_map
                    .get(&i)
                    .copied()
                    .ok_or(CompressionError::ValueNotFound)
            }
            PackValue::BigInt(i) => {
                let v = i64::try_from(*i).map_err(|_| CompressionError::ValueNotFound)?;
                self.integer_map
                    .get(&v)
                    .copied()
                    .ok_or(CompressionError::ValueNotFound)
            }
            PackValue::Null
            | PackValue::Undefined
            | PackValue::Bool(_)
            | PackValue::Str(_)
            | PackValue::Float(_) => {
                let key = LiteralKey::from_pack(value)?;
                self.map
                    .get(&key)
                    .copied()
                    .ok_or(CompressionError::ValueNotFound)
            }
            _ => Err(CompressionError::UnsupportedValueType),
        }
    }

    pub fn get_table(&self) -> &[PackValue] {
        &self.table
    }

    pub fn compress(&self, value: &PackValue) -> Result<PackValue, CompressionError> {
        match value {
            PackValue::Object(obj) => {
                let mut out = Vec::with_capacity(obj.len());
                for (key, val) in obj {
                    let key_index = self.get_index(&PackValue::Str(key.clone()))?;
                    out.push((key_index.to_string(), self.compress(val)?));
                }
                out.sort_by_key(|(key, _)| key.parse::<usize>().unwrap_or(usize::MAX));
                Ok(PackValue::Object(out))
            }
            PackValue::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(self.compress(item)?);
                }
                Ok(PackValue::Array(out))
            }
            PackValue::Extension(ext) => {
                let tag = i64::try_from(ext.tag).map_err(|_| CompressionError::TagOutOfRange)?;
                let tag_index = self.get_index(&PackValue::Integer(tag))?;
                let tag_index_u64 =
                    u64::try_from(tag_index).map_err(|_| CompressionError::IndexOverflow)?;
                Ok(PackValue::Extension(Box::new(JsonPackExtension::new(
                    tag_index_u64,
                    self.compress(&ext.val)?,
                ))))
            }
            PackValue::Bytes(_) | PackValue::Blob(_) => Err(CompressionError::UnsupportedValueType),
            _ => index_to_pack(self.get_index(value)?),
        }
    }
}
