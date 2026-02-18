//! BSON-specific value types.
//!
//! Upstream reference: `json-pack/src/bson/values.ts`

/// BSON ObjectId (12 bytes: 4-byte timestamp + 5-byte process ID + 3-byte counter).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonObjectId {
    pub timestamp: u32,
    pub process: u64,
    pub counter: u32,
}

/// BSON DBPointer (deprecated BSON type).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonDbPointer {
    pub name: String,
    pub id: BsonObjectId,
}

/// BSON JavaScript code (without scope).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonJavascriptCode {
    pub code: String,
}

/// BSON Symbol (deprecated BSON type).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonSymbol {
    pub symbol: String,
}

/// BSON JavaScript code with scope (deprecated).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonJavascriptCodeWithScope {
    pub code: String,
    pub scope: Vec<(String, BsonValue)>,
}

/// EJSON wrapper: BSON int32 typed number.
#[derive(Debug, Clone, PartialEq)]
pub struct BsonInt32 {
    pub value: i32,
}

/// EJSON wrapper: BSON int64 typed number.
#[derive(Debug, Clone, PartialEq)]
pub struct BsonInt64 {
    pub value: i64,
}

/// EJSON wrapper: BSON double typed number.
#[derive(Debug, Clone, PartialEq)]
pub struct BsonFloat {
    pub value: f64,
}

/// BSON Timestamp (MongoDB internal replication timestamp).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonTimestamp {
    pub increment: i32,
    pub timestamp: i32,
}

/// BSON Decimal128 (16-byte IEEE 754 decimal floating-point).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonDecimal128 {
    pub data: Vec<u8>,
}

/// BSON MinKey sentinel.
#[derive(Debug, Clone, PartialEq)]
pub struct BsonMinKey;

/// BSON MaxKey sentinel.
#[derive(Debug, Clone, PartialEq)]
pub struct BsonMaxKey;

/// BSON Binary data (subtype + raw bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct BsonBinary {
    pub subtype: u8,
    pub data: Vec<u8>,
}

/// A BSON value that can appear as a document field value.
#[derive(Debug, Clone, PartialEq)]
pub enum BsonValue {
    /// BSON double (0x01)
    Float(f64),
    /// BSON UTF-8 string (0x02)
    Str(String),
    /// Embedded BSON document (0x03)
    Document(Vec<(String, BsonValue)>),
    /// BSON array (0x04)
    Array(Vec<BsonValue>),
    /// BSON binary data (0x05)
    Binary(BsonBinary),
    /// BSON undefined (deprecated) (0x06)
    Undefined,
    /// BSON ObjectId (0x07)
    ObjectId(BsonObjectId),
    /// BSON boolean (0x08)
    Boolean(bool),
    /// BSON UTC datetime (milliseconds since epoch) (0x09)
    DateTime(i64),
    /// BSON null (0x0a)
    Null,
    /// BSON regular expression (0x0b)
    Regex(String, String),
    /// BSON DBPointer (deprecated) (0x0c)
    DbPointer(BsonDbPointer),
    /// BSON JavaScript code (0x0d)
    JavaScriptCode(BsonJavascriptCode),
    /// BSON Symbol (deprecated) (0x0e)
    Symbol(BsonSymbol),
    /// BSON JavaScript code with scope (deprecated) (0x0f)
    JavaScriptCodeWithScope(BsonJavascriptCodeWithScope),
    /// BSON int32 (0x10)
    Int32(i32),
    /// BSON Timestamp (0x11)
    Timestamp(BsonTimestamp),
    /// BSON int64 (0x12)
    Int64(i64),
    /// BSON Decimal128 (0x13)
    Decimal128(BsonDecimal128),
    /// BSON MinKey (0xFF)
    MinKey,
    /// BSON MaxKey (0x7F)
    MaxKey,
}
