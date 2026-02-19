//! Schema builder — port of SchemaBuilder.ts.
//!
//! Provides a fluent API for constructing schema values.

use serde_json::Value;

use super::schema::*;

/// Builder for constructing schema values.
///
/// Upstream reference: json-type/src/schema/SchemaBuilder.ts
#[derive(Debug, Clone, Default)]
pub struct SchemaBuilder;

#[allow(non_snake_case)]
impl SchemaBuilder {
    pub fn new() -> Self {
        Self
    }

    // ------------------------------------------------------------------
    // Shorthand property accessors (no options)

    pub fn str(&self) -> Schema {
        self.String(None)
    }

    pub fn num(&self) -> Schema {
        self.Number(None)
    }

    pub fn bool(&self) -> Schema {
        self.Boolean(None)
    }

    pub fn any(&self) -> Schema {
        self.Any(None)
    }

    pub fn arr(&self) -> Schema {
        self.Array(self.any(), None)
    }

    pub fn obj(&self) -> Schema {
        self.Object(vec![], None)
    }

    pub fn map(&self) -> Schema {
        self.Map(self.any(), None, None)
    }

    pub fn bin(&self) -> Schema {
        self.Binary(self.any(), None, None)
    }

    pub fn fn_(&self) -> Schema {
        self.Function(self.any(), self.any(), None)
    }

    pub fn fn_rx(&self) -> Schema {
        self.function_streaming(self.any(), self.any(), None)
    }

    pub fn undef(&self) -> Schema {
        self.Const(Value::Null, None)
    }

    pub fn nil(&self) -> Schema {
        self.Const(Value::Null, None)
    }

    // ------------------------------------------------------------------
    // Named constructors

    pub fn Boolean(&self, base: Option<SchemaBase>) -> Schema {
        Schema::Bool(BoolSchema {
            base: base.unwrap_or_default(),
        })
    }

    pub fn Number(&self, base: Option<SchemaBase>) -> Schema {
        Schema::Num(NumSchema {
            base: base.unwrap_or_default(),
            ..Default::default()
        })
    }

    pub fn String(&self, base: Option<SchemaBase>) -> Schema {
        Schema::Str(StrSchema {
            base: base.unwrap_or_default(),
            ..Default::default()
        })
    }

    pub fn Any(&self, base: Option<SchemaBase>) -> Schema {
        Schema::Any(AnySchema {
            base: base.unwrap_or_default(),
        })
    }

    pub fn Const(&self, value: Value, base: Option<SchemaBase>) -> Schema {
        Schema::Con(ConSchema {
            base: base.unwrap_or_default(),
            value,
        })
    }

    pub fn Binary(
        &self,
        type_: Schema,
        format: Option<BinFormat>,
        base: Option<SchemaBase>,
    ) -> Schema {
        Schema::Bin(BinSchema {
            base: base.unwrap_or_default(),
            type_: Box::new(type_),
            format,
            min: None,
            max: None,
        })
    }

    pub fn Array(&self, type_: Schema, base: Option<SchemaBase>) -> Schema {
        Schema::Arr(ArrSchema {
            base: base.unwrap_or_default(),
            type_: Some(Box::new(type_)),
            ..Default::default()
        })
    }

    pub fn Tuple(
        &self,
        head: Vec<Schema>,
        type_: Option<Schema>,
        tail: Option<Vec<Schema>>,
    ) -> Schema {
        Schema::Arr(ArrSchema {
            base: SchemaBase::default(),
            type_: type_.map(Box::new),
            head: Some(head),
            tail,
            ..Default::default()
        })
    }

    pub fn Object(&self, keys: Vec<KeySchema>, base: Option<SchemaBase>) -> Schema {
        Schema::Obj(ObjSchema {
            base: base.unwrap_or_default(),
            keys,
            ..Default::default()
        })
    }

    pub fn Key(&self, key: impl Into<String>, value: Schema) -> KeySchema {
        KeySchema {
            base: SchemaBase::default(),
            key: key.into(),
            value: Box::new(value),
            optional: None,
        }
    }

    pub fn KeyOpt(&self, key: impl Into<String>, value: Schema) -> KeySchema {
        KeySchema {
            base: SchemaBase::default(),
            key: key.into(),
            value: Box::new(value),
            optional: Some(true),
        }
    }

    pub fn Map(&self, value: Schema, key: Option<Schema>, base: Option<SchemaBase>) -> Schema {
        Schema::Map(MapSchema {
            base: base.unwrap_or_default(),
            key: key.map(Box::new),
            value: Box::new(value),
        })
    }

    pub fn Ref(&self, ref_: impl Into<String>) -> Schema {
        Schema::Ref(RefSchema {
            base: SchemaBase::default(),
            ref_: ref_.into(),
        })
    }

    pub fn Or(&self, types: Vec<Schema>) -> Schema {
        Schema::Or(OrSchema {
            base: SchemaBase::default(),
            types,
            discriminator: serde_json::json!(["num", -1]),
        })
    }

    pub fn Function(&self, req: Schema, res: Schema, base: Option<SchemaBase>) -> Schema {
        Schema::Fn(FnSchema {
            base: base.unwrap_or_default(),
            req: Box::new(req),
            res: Box::new(res),
        })
    }

    /// Streaming function (`fn$` in upstream TypeScript).
    pub fn function_streaming(&self, req: Schema, res: Schema, base: Option<SchemaBase>) -> Schema {
        Schema::FnRx(FnRxSchema {
            base: base.unwrap_or_default(),
            req: Box::new(req),
            res: Box::new(res),
        })
    }
}

/// Global default schema builder.
pub static S: SchemaBuilder = SchemaBuilder;
