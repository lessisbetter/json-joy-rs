use serde_json::Value;

/// Number format specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumFormat {
    I,
    U,
    F,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl NumFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::I => "i",
            Self::U => "u",
            Self::F => "f",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::I
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
        )
    }

    pub fn is_unsigned(self) -> bool {
        matches!(self, Self::U | Self::U8 | Self::U16 | Self::U32 | Self::U64)
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::F | Self::F32 | Self::F64)
    }
}

/// String format specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrFormat {
    Ascii,
    Utf8,
}

impl StrFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Utf8 => "utf8",
        }
    }
}

/// Binary encoding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinFormat {
    Json,
    Cbor,
    Msgpack,
    Resp3,
    Ion,
    Bson,
    Ubjson,
    Bencode,
}

impl BinFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Cbor => "cbor",
            Self::Msgpack => "msgpack",
            Self::Resp3 => "resp3",
            Self::Ion => "ion",
            Self::Bson => "bson",
            Self::Ubjson => "ubjson",
            Self::Bencode => "bencode",
        }
    }
}

/// Example of how a value of a given type could look.
#[derive(Debug, Clone)]
pub struct SchemaExample {
    pub value: Value,
    pub title: Option<String>,
    pub intro: Option<String>,
    pub description: Option<String>,
}

/// Deprecation information.
#[derive(Debug, Clone)]
pub struct Deprecated {
    pub info: Option<String>,
}

/// Fields common to all schema nodes.
#[derive(Debug, Clone, Default)]
pub struct SchemaBase {
    pub title: Option<String>,
    pub intro: Option<String>,
    pub description: Option<String>,
    pub meta: Option<Value>,
    pub default: Option<Value>,
    pub examples: Vec<SchemaExample>,
    pub deprecated: Option<Deprecated>,
}

/// Represents any value (unknown type).
#[derive(Debug, Clone, Default)]
pub struct AnySchema {
    pub base: SchemaBase,
}

/// Represents a constant value.
#[derive(Debug, Clone)]
pub struct ConSchema {
    pub base: SchemaBase,
    pub value: Value,
}

/// Represents a JSON boolean.
#[derive(Debug, Clone, Default)]
pub struct BoolSchema {
    pub base: SchemaBase,
}

/// Represents a JSON number with optional format and range constraints.
#[derive(Debug, Clone, Default)]
pub struct NumSchema {
    pub base: SchemaBase,
    pub format: Option<NumFormat>,
    pub gt: Option<f64>,
    pub gte: Option<f64>,
    pub lt: Option<f64>,
    pub lte: Option<f64>,
}

/// Represents a JSON string.
#[derive(Debug, Clone, Default)]
pub struct StrSchema {
    pub base: SchemaBase,
    pub format: Option<StrFormat>,
    pub ascii: Option<bool>,
    pub no_json_escape: Option<bool>,
    pub min: Option<u64>,
    pub max: Option<u64>,
}

/// Represents binary data (encoded value).
#[derive(Debug, Clone)]
pub struct BinSchema {
    pub base: SchemaBase,
    /// Type of value encoded in the binary data.
    pub type_: Box<Schema>,
    pub format: Option<BinFormat>,
    pub min: Option<u64>,
    pub max: Option<u64>,
}

/// Represents a JSON array.
#[derive(Debug, Clone, Default)]
pub struct ArrSchema {
    pub base: SchemaBase,
    /// Element type for homogeneous arrays.
    pub type_: Option<Box<Schema>>,
    /// Head tuple types (fixed prefix elements).
    pub head: Option<Vec<Schema>>,
    /// Tail tuple types (fixed suffix elements).
    pub tail: Option<Vec<Schema>>,
    pub min: Option<u64>,
    pub max: Option<u64>,
}

/// Represents a single field of an object.
#[derive(Debug, Clone)]
pub struct KeySchema {
    pub base: SchemaBase,
    pub key: String,
    pub value: Box<Schema>,
    pub optional: Option<bool>,
}

/// Represents a JSON object with defined keys.
#[derive(Debug, Clone, Default)]
pub struct ObjSchema {
    pub base: SchemaBase,
    pub keys: Vec<KeySchema>,
    pub extends: Option<Vec<String>>,
    pub decode_unknown_keys: Option<bool>,
    pub encode_unknown_keys: Option<bool>,
}

/// Represents an object treated as a map (all values same type).
#[derive(Debug, Clone)]
pub struct MapSchema {
    pub base: SchemaBase,
    pub key: Option<Box<Schema>>,
    pub value: Box<Schema>,
}

/// Reference to another named type.
#[derive(Debug, Clone)]
pub struct RefSchema {
    pub base: SchemaBase,
    pub ref_: String,
}

/// Union of multiple types.
#[derive(Debug, Clone)]
pub struct OrSchema {
    pub base: SchemaBase,
    pub types: Vec<Schema>,
    pub discriminator: Value,
}

/// RPC function type (request/response).
#[derive(Debug, Clone)]
pub struct FnSchema {
    pub base: SchemaBase,
    pub req: Box<Schema>,
    pub res: Box<Schema>,
}

/// Streaming RPC function type (Observable request/response).
#[derive(Debug, Clone)]
pub struct FnRxSchema {
    pub base: SchemaBase,
    pub req: Box<Schema>,
    pub res: Box<Schema>,
}

/// Named alias in a module.
#[derive(Debug, Clone)]
pub struct AliasSchema {
    pub base: SchemaBase,
    pub key: String,
    pub value: Box<Schema>,
    pub optional: Option<bool>,
    pub pub_: Option<bool>,
}

/// Module containing named type aliases.
#[derive(Debug, Clone, Default)]
pub struct ModuleSchema {
    pub base: SchemaBase,
    pub keys: Vec<AliasSchema>,
}

/// The unified Schema enum covering all schema kinds.
#[derive(Debug, Clone)]
pub enum Schema {
    Any(AnySchema),
    Bool(BoolSchema),
    Num(NumSchema),
    Str(StrSchema),
    Bin(BinSchema),
    Con(ConSchema),
    Arr(ArrSchema),
    Obj(ObjSchema),
    Key(KeySchema),
    Map(MapSchema),
    Ref(RefSchema),
    Or(OrSchema),
    Fn(FnSchema),
    FnRx(FnRxSchema),
    Alias(AliasSchema),
    Module(ModuleSchema),
}

impl Schema {
    /// Returns the "kind" string identifier for this schema node.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Any(_) => "any",
            Self::Bool(_) => "bool",
            Self::Num(_) => "num",
            Self::Str(_) => "str",
            Self::Bin(_) => "bin",
            Self::Con(_) => "con",
            Self::Arr(_) => "arr",
            Self::Obj(_) => "obj",
            Self::Key(_) => "key",
            Self::Map(_) => "map",
            Self::Ref(_) => "ref",
            Self::Or(_) => "or",
            Self::Fn(_) => "fn",
            Self::FnRx(_) => "fn$",
            Self::Alias(_) => "key",
            Self::Module(_) => "module",
        }
    }

    /// Returns the base schema fields.
    pub fn base(&self) -> &SchemaBase {
        match self {
            Self::Any(s) => &s.base,
            Self::Bool(s) => &s.base,
            Self::Num(s) => &s.base,
            Self::Str(s) => &s.base,
            Self::Bin(s) => &s.base,
            Self::Con(s) => &s.base,
            Self::Arr(s) => &s.base,
            Self::Obj(s) => &s.base,
            Self::Key(s) => &s.base,
            Self::Map(s) => &s.base,
            Self::Ref(s) => &s.base,
            Self::Or(s) => &s.base,
            Self::Fn(s) => &s.base,
            Self::FnRx(s) => &s.base,
            Self::Alias(s) => &s.base,
            Self::Module(s) => &s.base,
        }
    }
}
