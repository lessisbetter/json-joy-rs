//! Apache Avro type definitions.
//!
//! Upstream reference: `json-pack/src/avro/types.ts`
//! Reference: Apache Avro 1.12.0 specification

/// Avro schema.
#[derive(Debug, Clone)]
pub enum AvroSchema {
    Null,
    Boolean,
    Int,
    Long,
    Float,
    Double,
    Bytes,
    String,
    Record {
        name: String,
        namespace: Option<String>,
        fields: Vec<AvroField>,
        aliases: Vec<String>,
        doc: Option<String>,
    },
    Enum {
        name: String,
        namespace: Option<String>,
        symbols: Vec<String>,
        default: Option<String>,
        aliases: Vec<String>,
    },
    Array {
        items: Box<AvroSchema>,
    },
    Map {
        values: Box<AvroSchema>,
    },
    Fixed {
        name: String,
        namespace: Option<String>,
        size: usize,
        aliases: Vec<String>,
    },
    Union(Vec<AvroSchema>),
    /// Reference to a named type (resolved during encoding/decoding).
    Ref(String),
}

impl AvroSchema {
    /// Returns the full name (namespace.name if both present).
    pub fn full_name(&self) -> Option<String> {
        match self {
            AvroSchema::Record {
                name, namespace, ..
            } => Some(qualify(name, namespace.as_deref())),
            AvroSchema::Enum {
                name, namespace, ..
            } => Some(qualify(name, namespace.as_deref())),
            AvroSchema::Fixed {
                name, namespace, ..
            } => Some(qualify(name, namespace.as_deref())),
            _ => None,
        }
    }
}

fn qualify(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(ns) if !ns.is_empty() => format!("{}.{}", ns, name),
        _ => name.to_string(),
    }
}

/// A field in an Avro record schema.
#[derive(Debug, Clone)]
pub struct AvroField {
    pub name: String,
    pub type_: AvroSchema,
    pub default: Option<AvroValue>,
    pub doc: Option<String>,
    pub aliases: Vec<String>,
}

/// Avro runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum AvroValue {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    Str(String),
    Record(Vec<(String, AvroValue)>),
    Enum(String),
    Array(Vec<AvroValue>),
    Map(Vec<(String, AvroValue)>),
    Fixed(Vec<u8>),
    Union { index: usize, value: Box<AvroValue> },
}
