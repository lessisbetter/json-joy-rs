use json_joy_json_pack::xdr::{
    XdrDecodeError, XdrDecoder, XdrDiscriminant, XdrEncodeError, XdrEncoder, XdrSchema,
    XdrSchemaDecoder, XdrSchemaEncoder, XdrUnionValue, XdrValue,
};

fn schema_roundtrip(value: &XdrValue, schema: &XdrSchema) -> XdrValue {
    let mut encoder = XdrSchemaEncoder::new();
    let mut decoder = XdrSchemaDecoder::new();
    let encoded = encoder
        .encode(value, schema)
        .unwrap_or_else(|e| panic!("schema encode failed: {e}"));
    decoder
        .decode(&encoded, schema)
        .unwrap_or_else(|e| panic!("schema decode failed: {e}"))
}

#[test]
fn xdr_primitive_roundtrip_matrix() {
    let mut encoder = XdrEncoder::new();
    let mut decoder = XdrDecoder::new();

    encoder.write_void();
    let bytes = encoder.writer.flush();
    assert!(bytes.is_empty());
    decoder.reset(&bytes);
    decoder.read_void();

    encoder.write_boolean(true);
    encoder.write_boolean(false);
    let bytes = encoder.writer.flush();
    assert_eq!(bytes, vec![0, 0, 0, 1, 0, 0, 0, 0]);
    decoder.reset(&bytes);
    assert!(decoder.read_boolean().unwrap());
    assert!(!decoder.read_boolean().unwrap());

    encoder.write_int(-1);
    encoder.write_int(0x1234_5678);
    encoder.write_unsigned_int(u32::MAX);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_int().unwrap(), -1);
    assert_eq!(decoder.read_int().unwrap(), 0x1234_5678);
    assert_eq!(decoder.read_unsigned_int().unwrap(), u32::MAX);

    encoder.write_hyper(i64::from_be_bytes([
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
    ]));
    encoder.write_unsigned_hyper(u64::from_be_bytes([
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
    ]));
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(
        decoder.read_hyper().unwrap().to_be_bytes(),
        [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]
    );
    assert_eq!(
        decoder.read_unsigned_hyper().unwrap().to_be_bytes(),
        [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]
    );

    encoder.write_float(std::f32::consts::PI);
    encoder.write_double(std::f64::consts::PI);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert!((decoder.read_float().unwrap() - std::f32::consts::PI).abs() < 0.00001);
    assert!((decoder.read_double().unwrap() - std::f64::consts::PI).abs() < 1e-15);

    encoder.write_opaque(&[1, 2, 3]);
    encoder.write_opaque(&[1, 2, 3, 4]);
    encoder.write_varlen_opaque(&[1, 2, 3]);
    encoder.write_varlen_opaque(&[]);
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_opaque(3).unwrap(), vec![1, 2, 3]);
    assert_eq!(decoder.read_opaque(4).unwrap(), vec![1, 2, 3, 4]);
    assert_eq!(decoder.read_varlen_opaque().unwrap(), vec![1, 2, 3]);
    assert_eq!(decoder.read_varlen_opaque().unwrap(), Vec::<u8>::new());

    encoder.write_str("hello");
    encoder.write_str("");
    encoder.write_str("cafe");
    let bytes = encoder.writer.flush();
    decoder.reset(&bytes);
    assert_eq!(decoder.read_string().unwrap(), "hello");
    assert_eq!(decoder.read_string().unwrap(), "");
    assert_eq!(decoder.read_string().unwrap(), "cafe");
}

#[test]
fn xdr_decoder_array_helpers_matrix() {
    let mut encoder = XdrEncoder::new();
    let mut decoder = XdrDecoder::new();

    for n in [1i32, 2, 3] {
        encoder.write_int(n);
    }
    encoder.write_unsigned_int(4);
    for n in [4i32, 5, 6, 7] {
        encoder.write_int(n);
    }
    let bytes = encoder.writer.flush();

    decoder.reset(&bytes);
    let fixed = decoder
        .read_array(3, |d| d.read_int())
        .expect("fixed array decode");
    assert_eq!(fixed, vec![1, 2, 3]);
    let var = decoder
        .read_varlen_array(|d| d.read_int())
        .expect("var array decode");
    assert_eq!(var, vec![4, 5, 6, 7]);
}

#[test]
fn xdr_schema_scalar_and_string_matrix() {
    let cases: &[(XdrValue, XdrSchema)] = &[
        (XdrValue::Int(42), XdrSchema::Int),
        (XdrValue::UnsignedInt(u32::MAX), XdrSchema::UnsignedInt),
        (XdrValue::Bool(true), XdrSchema::Boolean),
        (
            XdrValue::Hyper(i64::from_be_bytes([
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
            ])),
            XdrSchema::Hyper,
        ),
        (
            XdrValue::UnsignedHyper(u64::from_be_bytes([
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
            ])),
            XdrSchema::UnsignedHyper,
        ),
        (XdrValue::Float(3.5), XdrSchema::Float),
        (XdrValue::Double(7.25), XdrSchema::Double),
        (
            XdrValue::Enum("GREEN".into()),
            XdrSchema::Enum(vec![
                ("RED".into(), 0),
                ("GREEN".into(), 1),
                ("BLUE".into(), 2),
            ]),
        ),
        (XdrValue::Bytes(vec![1, 2, 3]), XdrSchema::Opaque(3)),
        (
            XdrValue::Bytes(vec![1, 2, 3]),
            XdrSchema::VarOpaque(Some(10)),
        ),
        (XdrValue::Str("hello".into()), XdrSchema::Str(Some(10))),
    ];

    for (value, schema) in cases {
        let decoded = schema_roundtrip(value, schema);
        assert_eq!(decoded, *value);
    }
}

#[test]
fn xdr_schema_array_struct_optional_union_matrix() {
    let array_schema = XdrSchema::Array {
        element: Box::new(XdrSchema::Int),
        size: 3,
    };
    let array_value = XdrValue::Array(vec![XdrValue::Int(1), XdrValue::Int(2), XdrValue::Int(3)]);
    assert_eq!(schema_roundtrip(&array_value, &array_schema), array_value);

    let varray_schema = XdrSchema::VarArray {
        element: Box::new(XdrSchema::Int),
        max_size: Some(5),
    };
    let varray_value = XdrValue::Array(vec![XdrValue::Int(7), XdrValue::Int(8)]);
    assert_eq!(
        schema_roundtrip(&varray_value, &varray_schema),
        varray_value
    );

    let struct_schema = XdrSchema::Struct(vec![
        (Box::new(XdrSchema::Int), "id".into()),
        (Box::new(XdrSchema::Str(None)), "name".into()),
    ]);
    let struct_value = XdrValue::Struct(vec![
        ("id".into(), XdrValue::Int(42)),
        ("name".into(), XdrValue::Str("test".into())),
    ]);
    assert_eq!(
        schema_roundtrip(&struct_value, &struct_schema),
        struct_value
    );

    let optional_schema = XdrSchema::Optional(Box::new(XdrSchema::Int));
    let optional_some = XdrValue::Optional(Some(Box::new(XdrValue::Int(11))));
    let optional_none = XdrValue::Optional(None);
    assert_eq!(
        schema_roundtrip(&optional_some, &optional_schema),
        optional_some
    );
    assert_eq!(
        schema_roundtrip(&optional_none, &optional_schema),
        optional_none
    );

    let union_schema = XdrSchema::Union {
        arms: vec![
            (XdrDiscriminant::Int(0), Box::new(XdrSchema::Int)),
            (XdrDiscriminant::Int(1), Box::new(XdrSchema::Str(None))),
        ],
        default: None,
    };
    let union_int = XdrValue::Union(Box::new(XdrUnionValue {
        discriminant: XdrDiscriminant::Int(0),
        value: XdrValue::Int(42),
    }));
    let union_str = XdrValue::Union(Box::new(XdrUnionValue {
        discriminant: XdrDiscriminant::Int(1),
        value: XdrValue::Str("hello".into()),
    }));
    assert_eq!(schema_roundtrip(&union_int, &union_schema), union_int);
    assert_eq!(schema_roundtrip(&union_str, &union_schema), union_str);

    let union_default_schema = XdrSchema::Union {
        arms: vec![(XdrDiscriminant::Int(0), Box::new(XdrSchema::Int))],
        default: Some(Box::new(XdrSchema::Boolean)),
    };
    let union_default_value = XdrValue::Union(Box::new(XdrUnionValue {
        discriminant: XdrDiscriminant::Int(99),
        value: XdrValue::Bool(true),
    }));
    assert_eq!(
        schema_roundtrip(&union_default_value, &union_default_schema),
        union_default_value
    );
}

#[test]
fn xdr_schema_error_matrix() {
    let mut encoder = XdrSchemaEncoder::new();
    let mut decoder = XdrSchemaDecoder::new();

    let err = encoder
        .encode(&XdrValue::Str("hi".into()), &XdrSchema::Int)
        .expect_err("type mismatch should error");
    assert_eq!(err, XdrEncodeError::TypeMismatch("int"));

    let err = encoder
        .encode(
            &XdrValue::Bytes(vec![1, 2, 3]),
            &XdrSchema::VarOpaque(Some(2)),
        )
        .expect_err("oversized vopaque should error");
    assert_eq!(err, XdrEncodeError::OutOfRange);

    let err = encoder
        .encode(
            &XdrValue::Array(vec![XdrValue::Int(1), XdrValue::Int(2), XdrValue::Int(3)]),
            &XdrSchema::VarArray {
                element: Box::new(XdrSchema::Int),
                max_size: Some(2),
            },
        )
        .expect_err("oversized varray should error");
    assert_eq!(err, XdrEncodeError::OutOfRange);

    let err = encoder
        .encode(
            &XdrValue::Struct(vec![("id".into(), XdrValue::Int(42))]),
            &XdrSchema::Struct(vec![
                (Box::new(XdrSchema::Int), "id".into()),
                (Box::new(XdrSchema::Str(None)), "name".into()),
            ]),
        )
        .expect_err("missing field should error");
    assert_eq!(err, XdrEncodeError::MissingField("name".into()));

    let err = encoder
        .encode(
            &XdrValue::Union(Box::new(XdrUnionValue {
                discriminant: XdrDiscriminant::Int(2),
                value: XdrValue::Int(42),
            })),
            &XdrSchema::Union {
                arms: vec![(XdrDiscriminant::Int(0), Box::new(XdrSchema::Int))],
                default: None,
            },
        )
        .expect_err("missing union arm should error");
    assert_eq!(err, XdrEncodeError::NoUnionArm);

    let mut prim_encoder = XdrEncoder::new();
    prim_encoder.write_varlen_opaque(&[1, 2, 3]);
    let bytes = prim_encoder.writer.flush();
    let err = decoder
        .decode(&bytes, &XdrSchema::VarOpaque(Some(2)))
        .expect_err("decode vopaque max should error");
    assert_eq!(err, XdrDecodeError::MaxSizeExceeded);

    let mut prim_encoder = XdrEncoder::new();
    prim_encoder.write_unsigned_int(3);
    prim_encoder.write_int(1);
    prim_encoder.write_int(2);
    prim_encoder.write_int(3);
    let bytes = prim_encoder.writer.flush();
    let err = decoder
        .decode(
            &bytes,
            &XdrSchema::VarArray {
                element: Box::new(XdrSchema::Int),
                max_size: Some(2),
            },
        )
        .expect_err("decode varray max should error");
    assert_eq!(err, XdrDecodeError::MaxSizeExceeded);
}

#[test]
fn xdr_schema_quadruple_is_not_implemented_matrix() {
    let mut schema_encoder = XdrSchemaEncoder::new();
    let mut primitive_encoder = XdrEncoder::new();
    let mut schema_decoder = XdrSchemaDecoder::new();

    let err = schema_encoder
        .encode(&XdrValue::Double(std::f64::consts::PI), &XdrSchema::Quadruple)
        .expect_err("quadruple encode should be unsupported");
    assert_eq!(err, XdrEncodeError::UnsupportedType("quadruple"));

    primitive_encoder.write_double(std::f64::consts::PI);
    let bytes = primitive_encoder.writer.flush();
    let err = schema_decoder
        .decode(&bytes, &XdrSchema::Quadruple)
        .expect_err("quadruple decode should be unsupported");
    assert_eq!(err, XdrDecodeError::UnsupportedType("quadruple"));
}

#[test]
fn xdr_schema_string_union_discriminant_not_supported_matrix() {
    let mut encoder = XdrSchemaEncoder::new();
    let err = encoder
        .encode(
            &XdrValue::Union(Box::new(XdrUnionValue {
                discriminant: XdrDiscriminant::Str("red".into()),
                value: XdrValue::Int(7),
            })),
            &XdrSchema::Union {
                arms: vec![(XdrDiscriminant::Str("red".into()), Box::new(XdrSchema::Int))],
                default: None,
            },
        )
        .expect_err("string discriminants should be unsupported");
    assert_eq!(
        err,
        XdrEncodeError::UnsupportedType("string union discriminant")
    );
}

#[test]
fn xdr_decoder_invalid_utf8_matrix() {
    let mut decoder = XdrDecoder::new();
    let invalid = [0x00, 0x00, 0x00, 0x02, 0xff, 0xff, 0x00, 0x00];
    decoder.reset(&invalid);
    let err = decoder
        .read_string()
        .expect_err("invalid UTF-8 should fail");
    assert_eq!(err, XdrDecodeError::InvalidUtf8);
}
