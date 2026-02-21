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

fn expected_uint_wire(n: u64) -> Vec<u8> {
    let mut out = vec![0xe0, 0x01, 0x00, 0xea];
    if n == 0 {
        out.push(0x20);
        return out;
    }
    let bytes = n.to_be_bytes();
    let first = bytes
        .iter()
        .position(|b| *b != 0)
        .expect("non-zero integer must have significant byte");
    let payload = &bytes[first..];
    out.push(0x20 | payload.len() as u8);
    out.extend_from_slice(payload);
    out
}

fn expected_nint_wire(magnitude: u64) -> Vec<u8> {
    let bytes = magnitude.to_be_bytes();
    let first = bytes
        .iter()
        .position(|b| *b != 0)
        .expect("negative integer magnitude must be > 0");
    let payload = &bytes[first..];
    let mut out = vec![0xe0, 0x01, 0x00, 0xea, 0x30 | payload.len() as u8];
    out.extend_from_slice(payload);
    out
}

fn read_vuint(bytes: &[u8], mut pos: usize) -> (u32, usize) {
    let mut value: u32 = 0;
    loop {
        let byte = bytes[pos];
        pos += 1;
        value = (value << 7) | (byte & 0x7f) as u32;
        if byte & 0x80 != 0 {
            return (value, pos);
        }
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

#[test]
fn ion_integer_boundary_wire_matrix() {
    let mut encoder = IonEncoder::new();
    let mut decoder = IonDecoder::new();

    let u56 = 0x01_00_00_00_00_00_00_00_u64;
    let u_max = u64::MAX;
    let i_min = i64::MIN;

    let encoded_u56 = encoder.encode(&PackValue::UInteger(u56));
    assert_eq!(encoded_u56, expected_uint_wire(u56));
    assert_eq!(
        decoder.decode(&encoded_u56).unwrap(),
        PackValue::UInteger(u56)
    );

    let encoded_umax = encoder.encode(&PackValue::UInteger(u_max));
    assert_eq!(encoded_umax, expected_uint_wire(u_max));
    assert_eq!(
        decoder.decode(&encoded_umax).unwrap(),
        PackValue::UInteger(u_max)
    );

    let encoded_imin = encoder.encode(&PackValue::Integer(i_min));
    assert_eq!(encoded_imin, expected_nint_wire(i_min.unsigned_abs()));
    assert_eq!(
        decoder.decode(&encoded_imin).unwrap(),
        PackValue::Integer(i_min)
    );
}

#[test]
fn ion_upstream_encoder_boundary_inventory_matrix() {
    let mut encoder = IonEncoder::new();
    let mut decoder = IonDecoder::new();

    let ints: &[i64] = &[
        0,
        1,
        2,
        3,
        128,
        254,
        255,
        256,
        257,
        65_535,
        (1 << 16) - 2,
        (1 << 16) - 1,
        1 << 16,
        (1 << 16) + 1,
        (1 << 16) + 2,
        (1 << 24) - 2,
        (1 << 24) - 1,
        1 << 24,
        (1 << 24) + 1,
        (1 << 24) + 2,
        (1_i64 << 32) - 2,
        (1_i64 << 32) - 1,
        1_i64 << 32,
        (1_i64 << 32) + 1,
        (1_i64 << 32) + 2,
        (1_i64 << 40) - 2,
        1_i64 << 40,
        (1_i64 << 40) + 1,
        (1_i64 << 40) + 2,
        (1_i64 << 48) - 2,
        (1_i64 << 48) - 1,
        1_i64 << 48,
        (1_i64 << 48) + 1,
        (1_i64 << 48) + 2,
        (1_i64 << 53) - 1,
    ];

    for &value in ints {
        let positive = PackValue::Integer(value);
        let encoded = encoder.encode(&positive);
        let decoded = decoder.decode(&encoded).unwrap();
        assert_ion_eq(&decoded, &positive);

        if value > 0 {
            let negative = PackValue::Integer(-value);
            let encoded_neg = encoder.encode(&negative);
            let decoded_neg = decoder.decode(&encoded_neg).unwrap();
            assert_ion_eq(&decoded_neg, &negative);
        }
    }

    let floats: &[f64] = &[
        0.1,
        0.2,
        0.3,
        0.4,
        0.5,
        0.6,
        0.7,
        0.8,
        0.9,
        0.123,
        0.1234,
        0.12345,
        1.1,
        123.123,
        std::f64::consts::PI,
        4.23,
        7.22,
    ];
    for &value in floats {
        for signed in [value, -value] {
            let encoded = encoder.encode(&PackValue::Float(signed));
            assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
            assert_eq!(encoded[4], 0x48);
            let expected_tail = signed.to_le_bytes();
            assert_eq!(&encoded[5..13], &expected_tail);
            assert_ion_eq(
                &decoder.decode(&encoded).unwrap(),
                &PackValue::Float(signed),
            );
        }
    }

    let strings = vec![
        String::new(),
        "a".to_owned(),
        "ab".to_owned(),
        "abc".to_owned(),
        "abcd".to_owned(),
        "abcde".to_owned(),
        "abcdef".to_owned(),
        "abcdefg".to_owned(),
        "abcdefgh".to_owned(),
        "abcdefghi".to_owned(),
        "abcdefghij".to_owned(),
        "abcdefghijk".to_owned(),
        "abcdefghijkl".to_owned(),
        "abcdefghijklm".to_owned(),
        "abcdefghijklmn".to_owned(),
        "abcdefghijklmnopqrs".to_owned(),
        "abcdefghijklmnopqrstuvwxyz".to_owned(),
        "01234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567".to_owned(),
        "a".repeat(20_000),
    ];

    for value in strings {
        let encoded = encoder.encode(&PackValue::Str(value.clone()));
        assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
        let utf8 = value.as_bytes();
        if utf8.len() < 14 {
            assert_eq!(encoded[4], 0x80 | utf8.len() as u8);
            assert_eq!(&encoded[5..], utf8);
        } else {
            assert_eq!(encoded[4], 0x8e);
            let (declared, content_pos) = read_vuint(&encoded, 5);
            assert_eq!(declared as usize, utf8.len());
            assert_eq!(&encoded[content_pos..], utf8);
        }
        assert_eq!(decoder.decode(&encoded).unwrap(), PackValue::Str(value));
    }

    let binaries = vec![
        Vec::<u8>::new(),
        vec![0],
        vec![1, 2, 3],
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        (0u8..=255).collect(),
    ];

    for value in binaries {
        let encoded = encoder.encode(&PackValue::Bytes(value.clone()));
        assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
        if value.len() < 14 {
            assert_eq!(encoded[4], 0xa0 | value.len() as u8);
            assert_eq!(&encoded[5..], value.as_slice());
        } else {
            assert_eq!(encoded[4], 0xae);
            let (declared, content_pos) = read_vuint(&encoded, 5);
            assert_eq!(declared as usize, value.len());
            assert_eq!(&encoded[content_pos..], value.as_slice());
        }
        assert_eq!(decoder.decode(&encoded).unwrap(), PackValue::Bytes(value));
    }

    let arrays = vec![
        PackValue::Array(vec![]),
        PackValue::Array(vec![PackValue::Str(String::new())]),
        PackValue::Array(vec![PackValue::Str("asdf".to_owned())]),
        PackValue::Array(vec![PackValue::Integer(0)]),
        PackValue::Array(vec![
            PackValue::Integer(0),
            PackValue::Integer(0),
            PackValue::Integer(0),
        ]),
        PackValue::Array(vec![PackValue::Integer(0), PackValue::Integer(1)]),
        PackValue::Array((1..=6).map(PackValue::Integer).collect()),
        PackValue::Array((1..=16).map(PackValue::Integer).collect()),
        PackValue::Array(vec![PackValue::Array(vec![])]),
        PackValue::Array(vec![PackValue::Array(vec![
            PackValue::Integer(1),
            PackValue::Integer(2),
            PackValue::Integer(3),
            PackValue::Str("x".to_owned()),
        ])]),
    ];

    for value in arrays {
        let encoded = encoder.encode(&value);
        assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
        assert_ion_eq(&decoder.decode(&encoded).unwrap(), &value);
    }

    let objects = vec![
        PackValue::Object(vec![]),
        PackValue::Object(vec![("a".to_owned(), PackValue::Integer(1))]),
        PackValue::Object(vec![
            ("a".to_owned(), PackValue::Str("b".to_owned())),
            ("foo".to_owned(), PackValue::Str("bar".to_owned())),
        ]),
        PackValue::Object(vec![(
            "foo".to_owned(),
            PackValue::Array(vec![
                PackValue::Str("bar".to_owned()),
                PackValue::Integer(1),
                PackValue::Null,
                PackValue::Object(vec![
                    ("a".to_owned(), PackValue::Str("gg".to_owned())),
                    ("d".to_owned(), PackValue::Integer(123)),
                ]),
            ]),
        )]),
    ];

    for value in objects {
        let encoded = encoder.encode(&value);
        assert_eq!(&encoded[0..4], &[0xe0, 0x01, 0x00, 0xea]);
        if let PackValue::Object(fields) = &value {
            if !fields.is_empty() {
                assert!(
                    encoded.iter().skip(4).any(|b| (b >> 4) == 0x0e),
                    "object payload with fields should include symbol-table annotation"
                );
            }
        }
        assert_ion_eq(&decoder.decode(&encoded).unwrap(), &value);
    }
}
