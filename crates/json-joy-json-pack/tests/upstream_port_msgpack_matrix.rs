use json_joy_json_pack::msgpack::{
    MsgPackDecoder, MsgPackDecoderFast, MsgPackEncoder, MsgPackEncoderFast, MsgPackEncoderStable,
    MsgPackError, MsgPackPathSegment, MsgPackToJsonConverter,
};
use json_joy_json_pack::{JsonPackExtension, JsonPackValue, PackValue};

fn obj(fields: &[(&str, PackValue)]) -> PackValue {
    PackValue::Object(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn assert_pack_value_eq(actual: &PackValue, expected: &PackValue) {
    match (actual, expected) {
        (PackValue::Float(a), PackValue::Float(b)) if a.is_nan() && b.is_nan() => {}
        (PackValue::Float(a), PackValue::Integer(b)) if *a == *b as f64 => {}
        (PackValue::Integer(a), PackValue::Float(b)) if *a as f64 == *b => {}
        (PackValue::Array(a), PackValue::Array(b)) => {
            assert_eq!(a.len(), b.len(), "array length mismatch");
            for (left, right) in a.iter().zip(b.iter()) {
                assert_pack_value_eq(left, right);
            }
        }
        (PackValue::Object(a), PackValue::Object(b)) => {
            assert_eq!(a.len(), b.len(), "object field length mismatch");
            for ((ak, av), (bk, bv)) in a.iter().zip(b.iter()) {
                assert_eq!(ak, bk, "object key mismatch");
                assert_pack_value_eq(av, bv);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn msgpack_encoder_wire_matrix() {
    let mut encoder = MsgPackEncoderFast::new();

    assert_eq!(encoder.encode(&PackValue::Null), vec![0xc0]);
    assert_eq!(encoder.encode(&PackValue::Bool(false)), vec![0xc2]);
    assert_eq!(encoder.encode(&PackValue::Bool(true)), vec![0xc3]);
    assert_eq!(encoder.encode(&PackValue::Integer(0)), vec![0x00]);
    assert_eq!(encoder.encode(&PackValue::Integer(127)), vec![0x7f]);
    assert_eq!(encoder.encode(&PackValue::Integer(-1)), vec![0xff]);
    assert_eq!(encoder.encode(&PackValue::Integer(-32)), vec![0xe0]);

    assert_eq!(encoder.encode(&PackValue::Str("".into())), vec![0xa0]);
    assert_eq!(
        encoder.encode(&PackValue::Str("foo".into())),
        vec![0xa3, b'f', b'o', b'o']
    );

    let arr_15 = PackValue::Array((1..=15).map(PackValue::Integer).collect());
    let encoded_15 = encoder.encode(&arr_15);
    assert_eq!(encoded_15[0], 0x9f);
    assert_eq!(encoded_15.len(), 16);

    let arr_16 = PackValue::Array((1..=16).map(PackValue::Integer).collect());
    let encoded_16 = encoder.encode(&arr_16);
    assert_eq!(&encoded_16[..3], &[0xdc, 0x00, 0x10]);
    assert_eq!(encoded_16.len(), 19);

    let map_16 = PackValue::Object(
        (0..16)
            .map(|i| (i.to_string(), PackValue::Integer(i)))
            .collect(),
    );
    let encoded_map_16 = encoder.encode(&map_16);
    assert_eq!(&encoded_map_16[..3], &[0xde, 0x00, 0x10]);
}

#[test]
fn msgpack_decoder_matrix() {
    let mut encoder = MsgPackEncoderFast::new();
    let mut decoder = MsgPackDecoderFast::new();

    let values = vec![
        PackValue::Null,
        PackValue::Bool(true),
        PackValue::Bool(false),
        PackValue::Integer(123),
        PackValue::Integer(-32),
        PackValue::Integer(-4_807_526_976),
        PackValue::Float(3_456.123_456_789_022_4),
        PackValue::Str("".into()),
        PackValue::Str("abc".into()),
        PackValue::Str("a".repeat(256)),
        PackValue::Array(vec![
            PackValue::Integer(1),
            PackValue::Array(vec![PackValue::Integer(2)]),
            PackValue::Object(vec![("k".into(), PackValue::Bool(true))]),
        ]),
        obj(&[("foo", PackValue::Str("bar".into()))]),
    ];

    for value in values {
        let encoded = encoder.encode(&value);
        let decoded = decoder
            .decode(&encoded)
            .unwrap_or_else(|e| panic!("decode failed for {value:?}: {e}"));
        assert_pack_value_eq(&decoded, &value);
    }
}

#[test]
fn msgpack_decoder_one_level_matrix() {
    let mut encoder = MsgPackEncoder::new();
    let mut decoder = MsgPackDecoder::new();
    let mut nested_decoder = MsgPackDecoder::new();

    let input = obj(&[
        ("foo", PackValue::Str("bar".into())),
        ("baz", PackValue::Bool(true)),
        (
            "arr",
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]),
        ),
        ("obj", obj(&[("a", PackValue::Str("b".into()))])),
    ]);

    let encoded = encoder.encode(&input);
    let decoded = decoder.read_level(&encoded).expect("decode level");

    let PackValue::Object(fields) = decoded else {
        panic!("expected object");
    };
    assert_eq!(fields[0], ("foo".into(), PackValue::Str("bar".into())));
    assert_eq!(fields[1], ("baz".into(), PackValue::Bool(true)));

    let PackValue::Blob(arr_blob) = &fields[2].1 else {
        panic!("expected arr blob");
    };
    let arr_decoded = nested_decoder
        .decode(&arr_blob.val)
        .expect("decode arr blob");
    assert_eq!(
        arr_decoded,
        PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)])
    );

    let PackValue::Blob(obj_blob) = &fields[3].1 else {
        panic!("expected obj blob");
    };
    let obj_decoded = nested_decoder
        .decode(&obj_blob.val)
        .expect("decode obj blob");
    assert_eq!(obj_decoded, obj(&[("a", PackValue::Str("b".into()))]));

    let top_arr = PackValue::Array(vec![
        PackValue::Integer(1),
        PackValue::Object(vec![("k".into(), PackValue::Integer(2))]),
    ]);
    let top_arr_encoded = encoder.encode(&top_arr);
    let top_arr_level = decoder.read_level(&top_arr_encoded).expect("array level");
    let PackValue::Array(values) = top_arr_level else {
        panic!("expected top-level array");
    };
    assert_eq!(values[0], PackValue::Integer(1));
    assert!(matches!(values[1], PackValue::Blob(_)));
}

#[test]
fn msgpack_decoder_shallow_read_matrix() {
    let mut encoder = MsgPackEncoder::new();
    let mut decoder = MsgPackDecoder::new();

    let document = obj(&[
        (
            "a",
            obj(&[(
                "b",
                obj(&[(
                    "c",
                    PackValue::Array(vec![
                        PackValue::Integer(1),
                        PackValue::Integer(2),
                        PackValue::Integer(3),
                    ]),
                )]),
            )]),
        ),
        (
            "hmm",
            PackValue::Array(vec![obj(&[("foo", PackValue::Str("bar".into()))])]),
        ),
    ]);

    let encoded = encoder.encode(&document);

    decoder.reset(&encoded);
    let val = decoder
        .find_path(&[
            MsgPackPathSegment::Key("a"),
            MsgPackPathSegment::Key("b"),
            MsgPackPathSegment::Key("c"),
            MsgPackPathSegment::Index(1),
        ])
        .and_then(|d| d.read_any())
        .expect("find nested array value");
    assert_eq!(val, PackValue::Integer(2));

    decoder.reset(&encoded);
    let val = decoder
        .find_path(&[
            MsgPackPathSegment::Key("hmm"),
            MsgPackPathSegment::Index(0),
            MsgPackPathSegment::Key("foo"),
        ])
        .and_then(|d| d.read_any())
        .expect("find nested object key");
    assert_eq!(val, PackValue::Str("bar".into()));

    decoder.reset(&encoded);
    assert!(matches!(decoder.find_index(0), Err(MsgPackError::NotArr)));

    decoder.reset(&encoded);
    assert!(matches!(
        decoder.find_key("missing"),
        Err(MsgPackError::KeyNotFound)
    ));

    let arr = PackValue::Array(vec![PackValue::Integer(1)]);
    let arr_encoded = encoder.encode(&arr);
    decoder.reset(&arr_encoded);
    assert!(matches!(
        decoder.find_index(1),
        Err(MsgPackError::IndexOutOfBounds)
    ));
}

#[test]
fn msgpack_decoder_validate_matrix() {
    let mut encoder = MsgPackEncoder::new();
    let mut decoder = MsgPackDecoder::new();
    let encoded = encoder.encode(&PackValue::Float(1.1));

    assert!(decoder.validate(&encoded, 0, encoded.len()).is_ok());

    let mut longer = vec![0; encoded.len() + 1];
    longer[..encoded.len()].copy_from_slice(&encoded);
    assert!(matches!(
        decoder.validate(&longer, 0, longer.len()),
        Err(MsgPackError::InvalidSize)
    ));

    let shorter = encoded[..encoded.len() - 1].to_vec();
    assert!(decoder.validate(&shorter, 0, shorter.len()).is_err());

    let invalid = vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    assert!(decoder.validate(&invalid, 0, invalid.len()).is_err());
}

#[test]
fn msgpack_encoder_blob_extension_and_stable_matrix() {
    let mut encoder = MsgPackEncoder::new();
    let mut decoder = MsgPackDecoderFast::new();

    let precomputed =
        JsonPackValue::new(encoder.encode(&PackValue::Array(vec![PackValue::Str("gaga".into())])));
    let wrapped = obj(&[("foo", PackValue::Blob(precomputed))]);
    let expected = obj(&[("foo", PackValue::Array(vec![PackValue::Str("gaga".into())]))]);
    let encoded_wrapped = encoder.encode(&wrapped);
    let encoded_expected = encoder.encode(&expected);
    assert_eq!(encoded_wrapped, encoded_expected);

    let ext = PackValue::Extension(Box::new(JsonPackExtension::new(
        33,
        PackValue::Bytes(vec![1, 2, 3, 4, 5]),
    )));
    let ext_encoded = encoder.encode(&obj(&[("foo", ext.clone())]));
    let ext_decoded = decoder.decode(&ext_encoded).expect("decode extension");
    assert_eq!(ext_decoded, obj(&[("foo", ext)]));

    let mut stable = MsgPackEncoderStable::new();
    let out1 = stable.encode(&obj(&[
        ("a", PackValue::Integer(1)),
        ("b", PackValue::Integer(2)),
    ]));
    let out2 = stable.encode(&obj(&[
        ("b", PackValue::Integer(2)),
        ("a", PackValue::Integer(1)),
    ]));
    assert_eq!(out1, out2);
    assert_eq!(out1, vec![130, 161, 97, 1, 161, 98, 2]);
}

#[test]
fn msgpack_to_json_converter_matrix() {
    let mut encoder = MsgPackEncoder::new();
    let mut converter = MsgPackToJsonConverter::new();

    let docs = vec![
        PackValue::Null,
        PackValue::Bool(true),
        PackValue::Integer(123),
        PackValue::Str("hello".into()),
        PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]),
        obj(&[("foo", PackValue::Str("bar".into()))]),
    ];

    for doc in docs {
        let msgpack = encoder.encode(&doc);
        let json_text = converter.convert(&msgpack);
        let parsed: serde_json::Value =
            serde_json::from_str(&json_text).unwrap_or_else(|e| panic!("invalid json: {e}"));
        let expected: serde_json::Value = doc.clone().into();
        assert_eq!(parsed, expected);
    }
}
