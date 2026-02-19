//! Metaschema — describes the json-type schema system using its own schema language.
//!
//! Upstream reference: json-type/src/metaschema/metaschema.ts

use serde_json::json;

use crate::schema::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn str_schema() -> Schema {
    Schema::Str(StrSchema::default())
}

fn any_schema() -> Schema {
    Schema::Any(AnySchema::default())
}

fn bool_schema() -> Schema {
    Schema::Bool(BoolSchema::default())
}

fn num_schema() -> Schema {
    Schema::Num(NumSchema::default())
}

fn con_schema(value: serde_json::Value) -> Schema {
    Schema::Con(ConSchema {
        base: SchemaBase::default(),
        value,
    })
}

fn ref_schema(name: &str) -> Schema {
    Schema::Ref(RefSchema {
        base: SchemaBase::default(),
        ref_: name.to_string(),
    })
}

fn arr_schema(item: Schema) -> Schema {
    Schema::Arr(ArrSchema {
        type_: Some(Box::new(item)),
        ..Default::default()
    })
}

fn map_schema(value: Schema) -> Schema {
    Schema::Map(MapSchema {
        base: SchemaBase::default(),
        key: None,
        value: Box::new(value),
    })
}

fn or_schema(types: Vec<Schema>) -> Schema {
    Schema::Or(OrSchema {
        base: SchemaBase::default(),
        types,
        discriminator: json!(["num", -1]),
    })
}

fn key(k: &str, v: Schema) -> KeySchema {
    KeySchema {
        base: SchemaBase::default(),
        key: k.to_string(),
        value: Box::new(v),
        optional: None,
    }
}

fn key_opt(k: &str, v: Schema) -> KeySchema {
    KeySchema {
        base: SchemaBase::default(),
        key: k.to_string(),
        value: Box::new(v),
        optional: Some(true),
    }
}

fn obj(keys: Vec<KeySchema>) -> Schema {
    Schema::Obj(ObjSchema {
        keys,
        ..Default::default()
    })
}

fn alias(id: &str, value: Schema) -> AliasSchema {
    AliasSchema {
        base: SchemaBase::default(),
        key: id.to_string(),
        value: Box::new(value),
        optional: None,
        pub_: Some(true),
    }
}

// ── Individual type definitions ───────────────────────────────────────────────

fn display() -> AliasSchema {
    alias(
        "Display",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
        ]),
    )
}

fn schema_example() -> AliasSchema {
    alias(
        "SchemaExample",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("value", any_schema()),
        ]),
    )
}

fn schema_base() -> AliasSchema {
    alias(
        "SchemaBase",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", str_schema()),
            key_opt("meta", map_schema(any_schema())),
            key_opt("default", any_schema()),
            key_opt("examples", arr_schema(ref_schema("SchemaExample"))),
            key_opt("deprecated", obj(vec![key_opt("info", str_schema())])),
            key_opt("metadata", map_schema(any_schema())),
        ]),
    )
}

fn any_schema_def() -> AliasSchema {
    alias(
        "AnySchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("any"))),
        ]),
    )
}

fn con_schema_def() -> AliasSchema {
    alias(
        "ConSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("con"))),
            key("value", any_schema()),
        ]),
    )
}

fn bool_schema_def() -> AliasSchema {
    alias(
        "BoolSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("bool"))),
        ]),
    )
}

fn num_schema_def() -> AliasSchema {
    alias(
        "NumSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("num"))),
            key_opt(
                "format",
                or_schema(vec![
                    con_schema(json!("i")),
                    con_schema(json!("u")),
                    con_schema(json!("f")),
                    con_schema(json!("i8")),
                    con_schema(json!("i16")),
                    con_schema(json!("i32")),
                    con_schema(json!("i64")),
                    con_schema(json!("u8")),
                    con_schema(json!("u16")),
                    con_schema(json!("u32")),
                    con_schema(json!("u64")),
                    con_schema(json!("f32")),
                    con_schema(json!("f64")),
                ]),
            ),
            key_opt("gt", num_schema()),
            key_opt("gte", num_schema()),
            key_opt("lt", num_schema()),
            key_opt("lte", num_schema()),
        ]),
    )
}

fn str_schema_def() -> AliasSchema {
    alias(
        "StrSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("str"))),
            key_opt(
                "format",
                or_schema(vec![con_schema(json!("ascii")), con_schema(json!("utf8"))]),
            ),
            key_opt("ascii", bool_schema()),
            key_opt("noJsonEscape", bool_schema()),
            key_opt("min", num_schema()),
            key_opt("max", num_schema()),
        ]),
    )
}

fn bin_schema_def() -> AliasSchema {
    alias(
        "BinSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("bin"))),
            key("type", ref_schema("Schema")),
            key_opt(
                "format",
                or_schema(vec![
                    con_schema(json!("json")),
                    con_schema(json!("cbor")),
                    con_schema(json!("msgpack")),
                    con_schema(json!("resp3")),
                    con_schema(json!("ion")),
                    con_schema(json!("bson")),
                    con_schema(json!("ubjson")),
                    con_schema(json!("bencode")),
                ]),
            ),
            key_opt("min", num_schema()),
            key_opt("max", num_schema()),
        ]),
    )
}

fn arr_schema_def() -> AliasSchema {
    alias(
        "ArrSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("arr"))),
            key_opt("type", ref_schema("Schema")),
            key_opt("head", arr_schema(ref_schema("Schema"))),
            key_opt("tail", arr_schema(ref_schema("Schema"))),
            key_opt("min", num_schema()),
            key_opt("max", num_schema()),
        ]),
    )
}

fn key_schema_def() -> AliasSchema {
    alias(
        "KeySchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("key"))),
            key("key", str_schema()),
            key("value", ref_schema("Schema")),
            key_opt("optional", bool_schema()),
        ]),
    )
}

fn obj_schema_def() -> AliasSchema {
    alias(
        "ObjSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("obj"))),
            key("keys", arr_schema(ref_schema("KeySchema"))),
            key_opt("extends", arr_schema(str_schema())),
            key_opt("decodeUnknownKeys", bool_schema()),
            key_opt("encodeUnknownKeys", bool_schema()),
        ]),
    )
}

fn map_schema_def() -> AliasSchema {
    alias(
        "MapSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("map"))),
            key_opt("key", ref_schema("Schema")),
            key("value", ref_schema("Schema")),
        ]),
    )
}

fn ref_schema_def() -> AliasSchema {
    alias(
        "RefSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("ref"))),
            key("ref", str_schema()),
        ]),
    )
}

fn or_schema_def() -> AliasSchema {
    alias(
        "OrSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("or"))),
            key("types", arr_schema(ref_schema("Schema"))),
            key("discriminator", any_schema()),
        ]),
    )
}

fn fn_schema_def() -> AliasSchema {
    alias(
        "FnSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("fn"))),
            key("req", ref_schema("Schema")),
            key("res", ref_schema("Schema")),
        ]),
    )
}

fn fn_rx_schema_def() -> AliasSchema {
    alias(
        "FnRxSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("fn$"))),
            key("req", ref_schema("Schema")),
            key("res", ref_schema("Schema")),
        ]),
    )
}

fn alias_schema_def() -> AliasSchema {
    alias(
        "AliasSchema",
        obj(vec![
            key_opt("title", str_schema()),
            key_opt("intro", str_schema()),
            key_opt("description", str_schema()),
            key("kind", con_schema(json!("key"))),
            key("key", str_schema()),
            key("value", ref_schema("Schema")),
            key_opt("optional", bool_schema()),
            key_opt("pub", bool_schema()),
        ]),
    )
}

fn module_schema_def() -> AliasSchema {
    alias(
        "ModuleSchema",
        obj(vec![
            key("kind", con_schema(json!("module"))),
            key("keys", arr_schema(ref_schema("AliasSchema"))),
        ]),
    )
}

fn json_schema_def() -> AliasSchema {
    alias(
        "JsonSchema",
        or_schema(vec![
            ref_schema("BoolSchema"),
            ref_schema("NumSchema"),
            ref_schema("StrSchema"),
            ref_schema("BinSchema"),
            ref_schema("ArrSchema"),
            ref_schema("ConSchema"),
            ref_schema("ObjSchema"),
            ref_schema("KeySchema"),
            ref_schema("MapSchema"),
        ]),
    )
}

fn schema_def() -> AliasSchema {
    alias(
        "Schema",
        or_schema(vec![
            ref_schema("JsonSchema"),
            ref_schema("RefSchema"),
            ref_schema("OrSchema"),
            ref_schema("AnySchema"),
            ref_schema("FnSchema"),
            ref_schema("FnRxSchema"),
        ]),
    )
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Returns the metaschema: a `ModuleSchema` that describes the entire
/// json-type schema system using its own schema language.
///
/// Ports the `module` export from `json-type/src/metaschema/metaschema.ts`.
pub fn module() -> ModuleSchema {
    ModuleSchema {
        base: SchemaBase::default(),
        keys: vec![
            display(),
            schema_example(),
            schema_base(),
            any_schema_def(),
            con_schema_def(),
            bool_schema_def(),
            num_schema_def(),
            str_schema_def(),
            bin_schema_def(),
            arr_schema_def(),
            key_schema_def(),
            obj_schema_def(),
            map_schema_def(),
            ref_schema_def(),
            or_schema_def(),
            fn_schema_def(),
            fn_rx_schema_def(),
            alias_schema_def(),
            module_schema_def(),
            json_schema_def(),
            schema_def(),
        ],
    }
}
