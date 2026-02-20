use json_joy_json_pack::avro::{
    AvroDecodeError, AvroDecoder, AvroEncodeError, AvroEncoder, AvroField, AvroSchema,
    AvroSchemaDecoder, AvroSchemaEncoder, AvroValue,
};

fn field(name: &str, type_: AvroSchema) -> AvroField {
    AvroField {
        name: name.to_string(),
        type_,
        default: None,
        doc: None,
        aliases: Vec::new(),
    }
}

fn field_with_default(name: &str, type_: AvroSchema, default: AvroValue) -> AvroField {
    AvroField {
        name: name.to_string(),
        type_,
        default: Some(default),
        doc: None,
        aliases: Vec::new(),
    }
}

#[test]
fn avro_encoder_wire_matrix() {
    let mut encoder = AvroEncoder::new();

    encoder.write_null();
    assert!(encoder.writer.flush().is_empty());

    encoder.write_boolean(true);
    assert_eq!(encoder.writer.flush(), vec![1]);
    encoder.write_boolean(false);
    assert_eq!(encoder.writer.flush(), vec![0]);

    encoder.write_int(42);
    assert_eq!(encoder.writer.flush(), vec![84]);
    encoder.write_int(-1);
    assert_eq!(encoder.writer.flush(), vec![1]);

    encoder.write_bytes(&[1, 2, 3, 4]);
    assert_eq!(encoder.writer.flush(), vec![4, 1, 2, 3, 4]);

    encoder.write_str("hello");
    assert_eq!(encoder.writer.flush(), b"\x05hello".to_vec());

    encoder.write_varint_u32(3);
    encoder.write_int(1);
    encoder.write_int(2);
    encoder.write_int(3);
    encoder.write_varint_u32(0);
    assert_eq!(encoder.writer.flush(), vec![3, 2, 4, 6, 0]);

    encoder.write_varint_u32(1);
    encoder.write_str("key");
    encoder.write_str("value");
    encoder.write_varint_u32(0);
    assert_eq!(encoder.writer.flush(), b"\x01\x03key\x05value\x00".to_vec());
}

#[test]
fn avro_decoder_matrix() {
    let mut encoder = AvroEncoder::new();
    let mut decoder = AvroDecoder::new();

    encoder.write_int(300);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_int().unwrap(), 300);

    encoder.write_long(1_000_000);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_long().unwrap(), 1_000_000);

    encoder.write_float(std::f32::consts::PI);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert!((decoder.read_float().unwrap() - std::f32::consts::PI).abs() < 1e-6);

    encoder.write_double(std::f64::consts::PI);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_double().unwrap(), std::f64::consts::PI);

    encoder.write_bytes(&[1, 2, 3]);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_bytes().unwrap(), vec![1, 2, 3]);

    encoder.write_str("Hello 🌍");
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_str().unwrap(), "Hello 🌍");

    // Array: [1, 2, 3]
    encoder.write_varint_u32(3);
    encoder.write_int(1);
    encoder.write_int(2);
    encoder.write_int(3);
    encoder.write_varint_u32(0);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(
        decoder.read_array(|dec| dec.read_int()).unwrap(),
        vec![1, 2, 3]
    );

    // Map: {"a": "x", "b": "y"}
    encoder.write_varint_u32(2);
    encoder.write_str("a");
    encoder.write_str("x");
    encoder.write_str("b");
    encoder.write_str("y");
    encoder.write_varint_u32(0);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(
        decoder.read_map(|dec| dec.read_str()).unwrap(),
        vec![
            ("a".to_string(), "x".to_string()),
            ("b".to_string(), "y".to_string())
        ]
    );

    // INVALID_KEY map key.
    let mut invalid_key_bytes = vec![1, 9];
    invalid_key_bytes.extend_from_slice(b"__proto__");
    decoder.reset(&invalid_key_bytes);
    assert_eq!(
        decoder.read_map(|dec| dec.read_str()).unwrap_err(),
        AvroDecodeError::InvalidKey
    );

    decoder.reset(&[0x80, 0x80, 0x80, 0x80, 0x80]);
    assert_eq!(
        decoder.read_int().unwrap_err(),
        AvroDecodeError::VarIntTooLong
    );

    decoder.reset(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80]);
    assert_eq!(
        decoder.read_long().unwrap_err(),
        AvroDecodeError::VarLongTooLong
    );

    // Union index is zigzag int; -1 is invalid for union index.
    encoder.write_int(-1);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(
        decoder.read_union_index().unwrap_err(),
        AvroDecodeError::UnionIndexOutOfRange
    );
}

#[test]
fn avro_schema_codec_roundtrip_matrix() {
    let mut schema_encoder = AvroSchemaEncoder::new();
    let mut schema_decoder = AvroSchemaDecoder::new();

    let user_schema = AvroSchema::Record {
        name: "User".to_string(),
        namespace: None,
        fields: vec![
            field("id", AvroSchema::Int),
            field_with_default(
                "name",
                AvroSchema::String,
                AvroValue::Str("Unknown".to_string()),
            ),
        ],
        aliases: Vec::new(),
        doc: None,
    };
    let user_value = AvroValue::Record(vec![
        ("id".to_string(), AvroValue::Int(42)),
        ("name".to_string(), AvroValue::Str("John".to_string())),
    ]);
    let bytes = schema_encoder.encode(&user_value, &user_schema).unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &user_schema).unwrap(),
        user_value
    );

    let nested_array_schema = AvroSchema::Array {
        items: Box::new(AvroSchema::Array {
            items: Box::new(AvroSchema::Int),
        }),
    };
    let nested_array_value = AvroValue::Array(vec![
        AvroValue::Array(vec![AvroValue::Int(1), AvroValue::Int(2)]),
        AvroValue::Array(vec![
            AvroValue::Int(3),
            AvroValue::Int(4),
            AvroValue::Int(5),
        ]),
    ]);
    let bytes = schema_encoder
        .encode(&nested_array_value, &nested_array_schema)
        .unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &nested_array_schema).unwrap(),
        nested_array_value
    );

    let complex_map_schema = AvroSchema::Map {
        values: Box::new(AvroSchema::Record {
            name: "Value".to_string(),
            namespace: None,
            fields: vec![field("count", AvroSchema::Int)],
            aliases: Vec::new(),
            doc: None,
        }),
    };
    let complex_map_value = AvroValue::Map(vec![
        (
            "item1".to_string(),
            AvroValue::Record(vec![("count".to_string(), AvroValue::Int(10))]),
        ),
        (
            "item2".to_string(),
            AvroValue::Record(vec![("count".to_string(), AvroValue::Int(20))]),
        ),
    ]);
    let bytes = schema_encoder
        .encode(&complex_map_value, &complex_map_schema)
        .unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &complex_map_schema).unwrap(),
        complex_map_value
    );

    let union_schema =
        AvroSchema::Union(vec![AvroSchema::Null, AvroSchema::String, AvroSchema::Int]);
    let union_string = AvroValue::Str("hello".to_string());
    let bytes = schema_encoder.encode(&union_string, &union_schema).unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &union_schema).unwrap(),
        AvroValue::Union {
            index: 1,
            value: Box::new(union_string.clone())
        }
    );

    let union_null = AvroValue::Null;
    let bytes = schema_encoder.encode(&union_null, &union_schema).unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &union_schema).unwrap(),
        AvroValue::Union {
            index: 0,
            value: Box::new(AvroValue::Null)
        }
    );

    let explicit_union_int = AvroValue::Union {
        index: 2,
        value: Box::new(AvroValue::Int(42)),
    };
    let bytes = schema_encoder
        .encode(&explicit_union_int, &union_schema)
        .unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &union_schema).unwrap(),
        explicit_union_int
    );

    let recursive_schema = AvroSchema::Record {
        name: "Node".to_string(),
        namespace: None,
        fields: vec![
            field("value", AvroSchema::Int),
            field(
                "next",
                AvroSchema::Union(vec![AvroSchema::Null, AvroSchema::Ref("Node".to_string())]),
            ),
        ],
        aliases: Vec::new(),
        doc: None,
    };
    let recursive_value = AvroValue::Record(vec![
        ("value".to_string(), AvroValue::Int(1)),
        (
            "next".to_string(),
            AvroValue::Union {
                index: 1,
                value: Box::new(AvroValue::Record(vec![
                    ("value".to_string(), AvroValue::Int(2)),
                    (
                        "next".to_string(),
                        AvroValue::Union {
                            index: 0,
                            value: Box::new(AvroValue::Null),
                        },
                    ),
                ])),
            },
        ),
    ]);
    let bytes = schema_encoder
        .encode(&recursive_value, &recursive_schema)
        .unwrap();
    assert_eq!(
        schema_decoder.decode(&bytes, &recursive_schema).unwrap(),
        recursive_value
    );
}

#[test]
fn avro_schema_error_matrix() {
    let mut schema_encoder = AvroSchemaEncoder::new();
    let mut schema_decoder = AvroSchemaDecoder::new();

    // Invalid schema: duplicate union type.
    let invalid_union_schema = AvroSchema::Union(vec![AvroSchema::String, AvroSchema::String]);
    assert_eq!(
        schema_encoder
            .encode(&AvroValue::Str("x".to_string()), &invalid_union_schema)
            .unwrap_err(),
        AvroEncodeError::InvalidSchema
    );
    assert_eq!(
        schema_decoder
            .decode(&[], &invalid_union_schema)
            .unwrap_err(),
        AvroDecodeError::InvalidSchema
    );

    let fixed_schema = AvroSchema::Fixed {
        name: "Hash".to_string(),
        namespace: None,
        size: 4,
        aliases: Vec::new(),
    };
    assert_eq!(
        schema_encoder
            .encode(&AvroValue::Fixed(vec![1, 2, 3]), &fixed_schema)
            .unwrap_err(),
        AvroEncodeError::ValueDoesNotConform
    );

    let enum_schema = AvroSchema::Enum {
        name: "Color".to_string(),
        namespace: None,
        symbols: vec!["RED".to_string(), "GREEN".to_string(), "BLUE".to_string()],
        default: None,
        aliases: Vec::new(),
    };
    let mut primitive_encoder = AvroEncoder::new();
    primitive_encoder.write_int(5);
    let invalid_enum_bytes = primitive_encoder.writer.flush();
    assert_eq!(
        schema_decoder
            .decode(&invalid_enum_bytes, &enum_schema)
            .unwrap_err(),
        AvroDecodeError::InvalidEnumIndex(5)
    );
}
