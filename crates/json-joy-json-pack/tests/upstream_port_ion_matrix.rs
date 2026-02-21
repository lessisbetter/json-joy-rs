use json_joy_json_pack::ion::{IonDecodeError, IonDecoder, IonEncoder};
use json_joy_json_pack::PackValue;

fn obj(fields: &[(&str, PackValue)]) -> PackValue {
    PackValue::Object(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn assert_ion_eq(actual: &PackValue, expected: &PackValue) {
    match (actual, expected) {
        (PackValue::UInteger(a), PackValue::Integer(b)) if *a == *b as u64 => {}
        (PackValue::Integer(a), PackValue::UInteger(b)) if *a as u64 == *b => {}
        (PackValue::Float(a), PackValue::Float(b)) if a.is_nan() && b.is_nan() => {}
        (PackValue::Array(a), PackValue::Array(b)) => {
            assert_eq!(a.len(), b.len(), "array length mismatch");
            for (left, right) in a.iter().zip(b.iter()) {
                assert_ion_eq(left, right);
            }
        }
        (PackValue::Object(a), PackValue::Object(b)) => {
            assert_eq!(a.len(), b.len(), "object field length mismatch");
            for ((ak, av), (bk, bv)) in a.iter().zip(b.iter()) {
                assert_eq!(ak, bk, "object key mismatch");
                assert_ion_eq(av, bv);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn ion_encoder_decoder_matrix() {
    let mut encoder = IonEncoder::new();
    let mut decoder = IonDecoder::new();

    let values = vec![
        PackValue::Null,
        PackValue::Bool(true),
        PackValue::Bool(false),
        PackValue::Integer(0),
        PackValue::Integer(1),
        PackValue::Integer(127),
        PackValue::Integer(128),
        PackValue::Integer(65_535),
        PackValue::Integer(-1),
        PackValue::Integer(-127),
        PackValue::Integer(-128),
        PackValue::Integer(-65_535),
        PackValue::Float(0.5),
        PackValue::Float(-123.456),
        PackValue::Str("".into()),
        PackValue::Str("hello".into()),
        PackValue::Str("unicode: 👍🎉💯".into()),
        PackValue::Bytes(vec![]),
        PackValue::Bytes(vec![1, 2, 3, 4, 5]),
        PackValue::Array(vec![
            PackValue::Integer(1),
            PackValue::Str("x".into()),
            PackValue::Bool(true),
        ]),
        obj(&[
            ("a", PackValue::Integer(1)),
            ("b", PackValue::Str("c".into())),
        ]),
        obj(&[(
            "user",
            obj(&[
                ("name", PackValue::Str("John".into())),
                ("active", PackValue::Bool(true)),
            ]),
        )]),
    ];

    for value in values {
        let encoded = encoder.encode(&value);
        let decoded = decoder
            .decode(&encoded)
            .unwrap_or_else(|e| panic!("decode failed for {value:?}: {e}"));
        assert_ion_eq(&decoded, &value);
    }
}

#[test]
fn ion_wire_and_symbol_table_matrix() {
    let mut encoder = IonEncoder::new();
    let mut decoder = IonDecoder::new();

    assert_eq!(
        encoder.encode(&PackValue::Null),
        vec![0xe0, 0x01, 0x00, 0xea, 0x0f]
    );
    assert_eq!(
        encoder.encode(&PackValue::Bool(true)),
        vec![0xe0, 0x01, 0x00, 0xea, 0x11]
    );
    assert_eq!(
        encoder.encode(&PackValue::Bool(false)),
        vec![0xe0, 0x01, 0x00, 0xea, 0x10]
    );

    let value = obj(&[
        ("foo", PackValue::Integer(1)),
        ("bar", PackValue::Integer(2)),
    ]);
    let encoded = encoder.encode(&value);
    // Has IVM and at least one annotation marker for local symbol table.
    assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
    assert!(encoded.iter().any(|b| (b >> 4) == 0x0e));

    let decoded = decoder.decode(&encoded).unwrap();
    assert_ion_eq(&decoded, &value);
}

#[test]
fn ion_decoder_error_matrix() {
    let mut decoder = IonDecoder::new();

    assert!(matches!(
        decoder.decode(&[]),
        Err(IonDecodeError::EndOfInput)
    ));
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xeb]),
        Err(IonDecodeError::InvalidBvm)
    ));

    // Negative zero (NINT with length 0) is illegal.
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0x30]),
        Err(IonDecodeError::NegativeZero)
    ));

    // Struct field SID that is not in system/local table.
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0xd2, 0x8a, 0x20]),
        Err(IonDecodeError::UnknownSymbol(10))
    ));
}

#[test]
fn ion_decoder_base_edge_matrix() {
    let mut decoder = IonDecoder::new();

    // NULL NOP padding should consume/discard the next value and still decode as null.
    assert_eq!(
        decoder
            .decode(&[0xe0, 0x01, 0x00, 0xea, 0x00, 0x11])
            .unwrap(),
        PackValue::Null
    );

    // Invalid bool length is rejected (only 0, 1, and 15 are valid).
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0x12]),
        Err(IonDecodeError::InvalidBoolLen(2))
    ));

    // Unknown type nibble should surface explicit unknown-type error.
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0x50]),
        Err(IonDecodeError::UnknownType(5))
    ));

    // Annotation wrappers shorter than 3 bytes are invalid.
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0xe2, 0x80, 0x0f]),
        Err(IonDecodeError::AnnotationTooShort(2))
    ));

    // Container length mismatch should be detected for both lists and structs.
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0xb1, 0x21, 0x01]),
        Err(IonDecodeError::ListLengthMismatch)
    ));
    assert!(matches!(
        decoder.decode(&[0xe0, 0x01, 0x00, 0xea, 0xd1, 0x81, 0x21, 0x01]),
        Err(IonDecodeError::StructLengthMismatch)
    ));
}

#[test]
fn ion_decoder_read_matrix() {
    let mut decoder = IonDecoder::new();
    assert_eq!(
        decoder
            .decode(&[0xe0, 0x01, 0x00, 0xea, 0x11, 0x10])
            .unwrap(),
        PackValue::Bool(true)
    );
    assert_eq!(decoder.read().unwrap(), PackValue::Bool(false));
}
