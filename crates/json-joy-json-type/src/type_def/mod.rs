//! Type class hierarchy — Rust port of json-type/src/type/
//!
//! The TypeScript class hierarchy (AbsType<S> → concrete classes) is ported as:
//! - `TypeNode` enum: the sum type of all possible type classes
//! - Individual structs: `AnyType`, `NumType`, etc.
//! - `TypeBuilder`: factory for constructing TypeNode values

pub mod abs_type;
pub mod builder;
pub mod classes;
pub mod discriminator;
pub mod module_type;

pub use abs_type::BaseInfo;
pub use builder::TypeBuilder;
pub use classes::*;
pub use module_type::ModuleType;

use crate::schema::Schema;

/// The unified enum covering all type class instances.
///
/// Equivalent to the TypeScript union type `Type`.
#[derive(Debug, Clone)]
pub enum TypeNode {
    Any(AnyType),
    Bool(BoolType),
    Num(NumType),
    Str(StrType),
    Bin(BinType),
    Con(ConType),
    Arr(ArrType),
    Obj(ObjType),
    Map(MapType),
    Ref(RefType),
    Or(OrType),
    Fn(FnType),
    FnRx(FnRxType),
    Key(KeyType),
    Alias(AliasType),
}

impl TypeNode {
    /// Returns the kind string, matching the TypeScript `kind()` method.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Any(t) => t.kind(),
            Self::Bool(t) => t.kind(),
            Self::Num(t) => t.kind(),
            Self::Str(t) => t.kind(),
            Self::Bin(t) => t.kind(),
            Self::Con(t) => t.kind(),
            Self::Arr(t) => t.kind(),
            Self::Obj(t) => t.kind(),
            Self::Map(t) => t.kind(),
            Self::Ref(t) => t.kind(),
            Self::Or(t) => t.kind(),
            Self::Fn(t) => t.kind(),
            Self::FnRx(t) => t.kind(),
            Self::Key(t) => t.kind(),
            Self::Alias(t) => t.kind(),
        }
    }

    /// Returns the schema representation of this type node.
    pub fn get_schema(&self) -> Schema {
        match self {
            Self::Any(t) => t.get_schema(),
            Self::Bool(t) => t.get_schema(),
            Self::Num(t) => t.get_schema(),
            Self::Str(t) => t.get_schema(),
            Self::Bin(t) => t.get_schema(),
            Self::Con(t) => t.get_schema(),
            Self::Arr(t) => t.get_schema(),
            Self::Obj(t) => t.get_schema(),
            Self::Map(t) => t.get_schema(),
            Self::Ref(t) => t.get_schema(),
            Self::Or(t) => t.get_schema(),
            Self::Fn(t) => t.get_schema(),
            Self::FnRx(t) => t.get_schema(),
            Self::Key(t) => t.get_schema(),
            Self::Alias(t) => t.get_schema(),
        }
    }

    /// Returns a reference to the shared base info.
    pub fn base(&self) -> &BaseInfo {
        match self {
            Self::Any(t) => &t.base,
            Self::Bool(t) => &t.base,
            Self::Num(t) => &t.base,
            Self::Str(t) => &t.base,
            Self::Bin(t) => &t.base,
            Self::Con(t) => &t.base,
            Self::Arr(t) => &t.base,
            Self::Obj(t) => &t.base,
            Self::Map(t) => &t.base,
            Self::Ref(t) => &t.base,
            Self::Or(t) => &t.base,
            Self::Fn(t) => &t.base,
            Self::FnRx(t) => &t.base,
            Self::Key(t) => &t.base,
            Self::Alias(t) => &t.base,
        }
    }
}

impl std::fmt::Display for TypeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind())
    }
}
