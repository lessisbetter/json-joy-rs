use json_joy_json_pack::cbor::{
    CborDecoder, CborDecoderDag, CborEncoder, CborEncoderDag, CborEncoderFast, CborEncoderStable,
    CborError,
};
use json_joy_json_pack::{JsonPackExtension, PackValue};

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
fn cbor_encoder_decoder_matrix() {
    let mut encoder = CborEncoder::new();
    let decoder = CborDecoder::new();

    let values = vec![
        PackValue::Null,
        PackValue::Bool(true),
        PackValue::Bool(false),
        PackValue::Integer(0),
        PackValue::Integer(23),
        PackValue::Integer(24),
        PackValue::Integer(-1),
        PackValue::Integer(-24),
        PackValue::Integer(-25),
        PackValue::UInteger(u64::MAX),
        PackValue::Float(0.1),
        PackValue::Float(-123.123),
        PackValue::Bytes(vec![]),
        PackValue::Bytes(vec![1, 2, 3, 4, 5]),
        PackValue::Str("".into()),
        PackValue::Str("asdf asfd 😱 asdf asdf 👀 as".into()),
        PackValue::Array(vec![
            PackValue::Integer(1),
            PackValue::Str("a".into()),
            PackValue::Integer(-2),
        ]),
        obj(&[
            ("foo", PackValue::Str("bar".into())),
            ("baz", PackValue::Integer(123)),
        ]),
        PackValue::Extension(Box::new(JsonPackExtension::new(
            42,
            PackValue::Str("cid".into()),
        ))),
    ];

    for value in values {
        let encoded = encoder.encode(&value);
        let decoded = decoder
            .decode(&encoded)
            .unwrap_or_else(|e| panic!("decode failed for {value:?}: {e}"));
        assert_pack_value_eq(&decoded, &value);

        let (decoded_with_consumed, consumed) = decoder
            .decode_with_consumed(&encoded)
            .expect("decode_with_consumed");
        assert_eq!(consumed, encoded.len());
        assert_pack_value_eq(&decoded_with_consumed, &value);
    }
}

#[test]
fn cbor_streaming_indefinite_matrix() {
    let mut encoder = CborEncoderFast::new();
    let decoder = CborDecoder::new();

    encoder.writer.reset();
    encoder.write_start_bin();
    encoder.write_bin(&[1, 2, 3]);
    encoder.write_bin(&[4, 5, 6]);
    encoder.write_bin(&[7, 8, 9]);
    encoder.write_end();
    let encoded_bin = encoder.writer.flush();
    let decoded_bin = decoder.decode(&encoded_bin).expect("decode indef bin");
    assert_eq!(
        decoded_bin,
        PackValue::Bytes(vec![1, 2, 3, 4, 5, 6, 7, 8, 9])
    );

    encoder.writer.reset();
    encoder.write_start_str();
    encoder.write_str("abc");
    encoder.write_str("def");
    encoder.write_str("ghi");
    encoder.write_end();
    let encoded_str = encoder.writer.flush();
    let decoded_str = decoder.decode(&encoded_str).expect("decode indef str");
    assert_eq!(decoded_str, PackValue::Str("abcdefghi".into()));

    encoder.writer.reset();
    encoder.write_start_arr();
    encoder.write_arr_values(&[PackValue::Integer(1), PackValue::Integer(2)]);
    encoder.write_arr_values(&[PackValue::Integer(3), PackValue::Integer(4)]);
    encoder.write_end_arr();
    let encoded_arr = encoder.writer.flush();
    let decoded_arr = decoder.decode(&encoded_arr).expect("decode indef arr");
    assert_eq!(
        decoded_arr,
        PackValue::Array(vec![
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]),
            PackValue::Array(vec![PackValue::Integer(3), PackValue::Integer(4)]),
        ])
    );

    encoder.writer.reset();
    encoder.write_start_obj();
    encoder.write_str("foo");
    encoder.write_str("bar");
    encoder.write_str("n");
    encoder.write_integer(1);
    encoder.write_end_obj();
    let encoded_obj = encoder.writer.flush();
    let decoded_obj = decoder.decode(&encoded_obj).expect("decode indef obj");
    assert_eq!(
        decoded_obj,
        obj(&[
            ("foo", PackValue::Str("bar".into())),
            ("n", PackValue::Integer(1)),
        ])
    );
}

#[test]
fn cbor_validate_matrix() {
    let mut encoder = CborEncoder::new();
    let decoder = CborDecoder::new();

    let encoded = encoder.encode(&PackValue::Float(1.1));
    assert!(decoder.validate(&encoded, 0, encoded.len()).is_ok());

    let mut longer = vec![0; encoded.len() + 1];
    longer[..encoded.len()].copy_from_slice(&encoded);
    assert!(matches!(
        decoder.validate(&longer, 0, longer.len()),
        Err(CborError::InvalidSize)
    ));

    let shorter = encoded[..encoded.len() - 1].to_vec();
    assert!(decoder.validate(&shorter, 0, shorter.len()).is_err());

    let invalid = vec![0xff];
    assert!(decoder.validate(&invalid, 0, invalid.len()).is_err());

    let mut fast = CborEncoderFast::new();
    fast.writer.reset();
    fast.write_start_obj();
    fast.write_str("foo");
    fast.write_end();
    let invalid_map = fast.writer.flush();
    assert!(decoder
        .validate(&invalid_map, 0, invalid_map.len())
        .is_err());
}

#[test]
fn cbor_stable_and_dag_matrix() {
    let mut stable = CborEncoderStable::new();
    let decoder = CborDecoder::new();

    let left = obj(&[("a", PackValue::Integer(1)), ("b", PackValue::Integer(2))]);
    let right = obj(&[("b", PackValue::Integer(2)), ("a", PackValue::Integer(1))]);
    let left_encoded = stable.encode(&left);
    let right_encoded = stable.encode(&right);
    assert_eq!(left_encoded, right_encoded);
    assert_eq!(decoder.decode(&left_encoded).expect("stable decode"), left);

    let by_len_1 = obj(&[("aa", PackValue::Integer(1)), ("b", PackValue::Integer(2))]);
    let by_len_2 = obj(&[("b", PackValue::Integer(2)), ("aa", PackValue::Integer(1))]);
    assert_eq!(stable.encode(&by_len_1), stable.encode(&by_len_2));

    let mut dag_encoder = CborEncoderDag::new();
    let dag_decoder = CborDecoder::new();
    assert_eq!(
        dag_decoder
            .decode(&dag_encoder.encode(&PackValue::Undefined))
            .expect("decode dag undefined"),
        PackValue::Null
    );
    assert_eq!(
        dag_decoder
            .decode(&dag_encoder.encode(&PackValue::Float(f64::NAN)))
            .expect("decode dag nan"),
        PackValue::Null
    );
    assert_eq!(
        dag_decoder
            .decode(&dag_encoder.encode(&PackValue::Float(f64::INFINITY)))
            .expect("decode dag inf"),
        PackValue::Null
    );

    let tag_42 = obj(&[(
        "b",
        PackValue::Extension(Box::new(JsonPackExtension::new(
            42,
            PackValue::Str("cid".into()),
        ))),
    )]);
    let tag_42_encoded = dag_encoder.encode(&tag_42);
    let dag_read_42 = CborDecoderDag::new()
        .decode(&tag_42_encoded)
        .expect("decode dag tag 42");
    assert_eq!(dag_read_42, tag_42);

    let tag_43 = obj(&[(
        "b",
        PackValue::Extension(Box::new(JsonPackExtension::new(
            43,
            PackValue::Str("cid".into()),
        ))),
    )]);
    let tag_43_encoded = dag_encoder.encode(&tag_43);
    let dag_read_43 = CborDecoderDag::new()
        .decode(&tag_43_encoded)
        .expect("decode dag tag 43");
    assert_eq!(dag_read_43, obj(&[("b", PackValue::Str("cid".into()))]));
}
