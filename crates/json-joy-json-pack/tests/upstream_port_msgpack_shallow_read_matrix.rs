use json_joy_json_pack::msgpack::{
    gen_shallow_reader, MsgPackDecoder, MsgPackEncoder, MsgPackError, MsgPackMarker,
    MsgPackPathSegment,
};
use json_joy_json_pack::PackValue;

fn obj(fields: &[(&str, PackValue)]) -> PackValue {
    PackValue::Object(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn assert_shallow_read(document: &PackValue, path: &[MsgPackPathSegment<'_>]) {
    let mut encoder = MsgPackEncoder::new();
    let encoded = encoder.encode(document);

    let mut decoder = MsgPackDecoder::new();
    decoder.reset(&encoded);
    let direct_offset = decoder
        .find_path(path)
        .expect("direct decoder path traversal")
        .inner
        .x;

    let read = gen_shallow_reader(path);
    decoder.reset(&encoded);
    let generated_offset = read(&mut decoder).expect("generated shallow reader");

    assert_eq!(generated_offset, direct_offset);
}

#[test]
fn msgpack_shallow_reader_matrix() {
    let first_level_object = obj(&[
        ("bar", obj(&[])),
        ("baz", PackValue::Integer(123)),
        ("gg", PackValue::Bool(true)),
    ]);
    assert_shallow_read(&first_level_object, &[MsgPackPathSegment::Key("bar")]);
    assert_shallow_read(&first_level_object, &[MsgPackPathSegment::Key("baz")]);
    assert_shallow_read(&first_level_object, &[MsgPackPathSegment::Key("gg")]);

    let second_level_object = obj(&[
        (
            "a",
            obj(&[
                ("bar", obj(&[])),
                ("baz", PackValue::Integer(123)),
                ("gg", PackValue::Bool(true)),
            ]),
        ),
        ("b", obj(&[("mmmm", obj(&[("s", PackValue::Bool(true))]))])),
        ("end", PackValue::Null),
    ]);
    assert_shallow_read(
        &second_level_object,
        &[MsgPackPathSegment::Key("a"), MsgPackPathSegment::Key("bar")],
    );
    assert_shallow_read(
        &second_level_object,
        &[MsgPackPathSegment::Key("a"), MsgPackPathSegment::Key("baz")],
    );
    assert_shallow_read(
        &second_level_object,
        &[
            MsgPackPathSegment::Key("b"),
            MsgPackPathSegment::Key("mmmm"),
            MsgPackPathSegment::Key("s"),
        ],
    );
    assert_shallow_read(&second_level_object, &[MsgPackPathSegment::Key("end")]);

    let first_level_array = PackValue::Array(vec![
        PackValue::Integer(1234),
        PackValue::Str("asdf".into()),
        obj(&[]),
        PackValue::Null,
        PackValue::Bool(false),
    ]);
    assert_shallow_read(&first_level_array, &[MsgPackPathSegment::Index(0)]);
    assert_shallow_read(&first_level_array, &[MsgPackPathSegment::Index(1)]);
    assert_shallow_read(&first_level_array, &[MsgPackPathSegment::Index(2)]);
    assert_shallow_read(&first_level_array, &[MsgPackPathSegment::Index(3)]);
    assert_shallow_read(&first_level_array, &[MsgPackPathSegment::Index(4)]);

    let nested = obj(&[
        (
            "a",
            obj(&[
                (
                    "bar",
                    PackValue::Array(vec![
                        obj(&[
                            ("a", PackValue::Integer(1)),
                            ("2", PackValue::Bool(true)),
                            ("asdf", PackValue::Bool(false)),
                        ]),
                        PackValue::Integer(5),
                    ]),
                ),
                (
                    "baz",
                    PackValue::Array(vec![
                        PackValue::Str("a".into()),
                        PackValue::Str("b".into()),
                        PackValue::Integer(123),
                    ]),
                ),
                ("gg", PackValue::Bool(true)),
            ]),
        ),
        ("b", obj(&[("mmmm", obj(&[("s", PackValue::Bool(true))]))])),
        ("end", PackValue::Null),
    ]);
    assert_shallow_read(
        &nested,
        &[
            MsgPackPathSegment::Key("a"),
            MsgPackPathSegment::Key("bar"),
            MsgPackPathSegment::Index(0),
            MsgPackPathSegment::Key("2"),
        ],
    );
    assert_shallow_read(
        &nested,
        &[
            MsgPackPathSegment::Key("b"),
            MsgPackPathSegment::Key("mmmm"),
            MsgPackPathSegment::Key("s"),
        ],
    );
}

#[test]
fn msgpack_shallow_reader_error_matrix() {
    let document = obj(&[
        (
            "a",
            obj(&[("bar", obj(&[])), ("baz", PackValue::Integer(123))]),
        ),
        ("end", PackValue::Null),
    ]);
    let mut encoder = MsgPackEncoder::new();
    let encoded = encoder.encode(&document);
    let mut decoder = MsgPackDecoder::new();

    let missing_key = gen_shallow_reader(&[MsgPackPathSegment::Key("asdf")]);
    decoder.reset(&encoded);
    assert!(matches!(
        missing_key(&mut decoder),
        Err(MsgPackError::KeyNotFound)
    ));

    let array = PackValue::Array(vec![
        PackValue::Integer(1234),
        PackValue::Str("asdf".into()),
        obj(&[]),
        PackValue::Null,
        PackValue::Bool(false),
    ]);
    let encoded_array = encoder.encode(&array);
    let out_of_bounds = gen_shallow_reader(&[MsgPackPathSegment::Index(5)]);
    decoder.reset(&encoded_array);
    assert!(matches!(
        out_of_bounds(&mut decoder),
        Err(MsgPackError::IndexOutOfBounds)
    ));
}

#[test]
fn msgpack_constants_matrix() {
    assert_eq!(MsgPackMarker::Null as u8, 0xc0);
    assert_eq!(MsgPackMarker::Undefined as u8, 0xc1);
    assert_eq!(MsgPackMarker::False as u8, 0xc2);
    assert_eq!(MsgPackMarker::True as u8, 0xc3);
}
