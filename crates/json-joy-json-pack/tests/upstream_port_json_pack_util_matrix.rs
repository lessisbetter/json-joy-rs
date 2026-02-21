use json_joy_json_pack::util::buffers::to_data_uri;
use json_joy_json_pack::util::{CompressionTable, DecompressionTable};
use json_joy_json_pack::PackValue;

fn obj(fields: &[(&str, PackValue)]) -> PackValue {
    PackValue::Object(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

#[test]
fn compression_table_walk_matrix() {
    let table = CompressionTable::create(&PackValue::Integer(42)).unwrap();
    assert_eq!(table.get_table(), &[PackValue::Integer(42)]);

    let json = obj(&[
        ("foo", PackValue::Str("bar".into())),
        ("baz", PackValue::Integer(42)),
        ("gg", PackValue::Str("foo".into())),
        ("true", PackValue::Bool(false)),
    ]);
    let table = CompressionTable::create(&json).unwrap();
    assert_eq!(
        table.get_table(),
        &[
            PackValue::Integer(42),
            PackValue::Str("bar".into()),
            PackValue::Str("baz".into()),
            PackValue::Bool(false),
            PackValue::Str("foo".into()),
            PackValue::Str("gg".into()),
            PackValue::Str("true".into()),
        ]
    );

    let json = obj(&[
        (
            "foo",
            PackValue::Array(vec![
                PackValue::Integer(-3),
                PackValue::Integer(12),
                PackValue::Integer(42),
                PackValue::Integer(12_345),
            ]),
        ),
        ("baz", PackValue::Integer(42)),
    ]);
    let table = CompressionTable::create(&json).unwrap();
    assert_eq!(
        table.get_table(),
        &[
            PackValue::Integer(-3),
            PackValue::Integer(15),
            PackValue::Integer(30),
            PackValue::Integer(12_303),
            PackValue::Str("baz".into()),
            PackValue::Str("foo".into()),
        ]
    );

    let json = obj(&[
        (
            "foo",
            PackValue::Array(vec![
                PackValue::Integer(5),
                PackValue::Integer(1),
                PackValue::Integer(2),
                PackValue::Integer(4),
                PackValue::Integer(8),
                PackValue::Integer(16),
                PackValue::Integer(17),
                PackValue::Integer(22),
            ]),
        ),
        ("baz", PackValue::Float(-1.5)),
    ]);
    let table = CompressionTable::create(&json).unwrap();
    assert_eq!(
        table.get_table(),
        &[
            PackValue::Integer(1),
            PackValue::Integer(1),
            PackValue::Integer(2),
            PackValue::Integer(1),
            PackValue::Integer(3),
            PackValue::Integer(8),
            PackValue::Integer(1),
            PackValue::Integer(5),
            PackValue::Float(-1.5),
            PackValue::Str("baz".into()),
            PackValue::Str("foo".into()),
        ]
    );

    let json = obj(&[
        ("z1", PackValue::Float(-0.0)),
        ("z2", PackValue::Float(0.0)),
        ("n1", PackValue::Float(f64::NAN)),
        ("n2", PackValue::Float(f64::NAN)),
    ]);
    let table = CompressionTable::create(&json).unwrap();
    assert_eq!(table.get_table().len(), 6);
    let z1_idx = table.get_index(&PackValue::Float(-0.0)).unwrap();
    let z2_idx = table.get_index(&PackValue::Float(0.0)).unwrap();
    let n_idx = table.get_index(&PackValue::Float(f64::NAN)).unwrap();
    assert_eq!(z1_idx, z2_idx);
    assert!(matches!(table.get_table()[n_idx], PackValue::Float(f) if f.is_nan()));
}

#[test]
fn compression_table_compress_matrix() {
    let json = obj(&[
        ("foo", PackValue::Str("bar".into())),
        ("baz", PackValue::Integer(42)),
        ("gg", PackValue::Str("foo".into())),
        ("true", PackValue::Bool(false)),
    ]);
    let table = CompressionTable::create(&json).unwrap();
    let compressed = table.compress(&json).unwrap();
    assert_eq!(
        compressed,
        obj(&[
            ("2", PackValue::Integer(0)),
            ("4", PackValue::Integer(1)),
            ("5", PackValue::Integer(4)),
            ("6", PackValue::Integer(3)),
        ])
    );

    let json1 = obj(&[("foo", PackValue::Str("bar".into()))]);
    let json2 = obj(&[(
        "foo",
        PackValue::Array(vec![
            PackValue::Integer(0),
            PackValue::Integer(0),
            PackValue::Integer(5),
            PackValue::Integer(5),
        ]),
    )]);

    let mut table = CompressionTable::new();
    table.walk(&json1).unwrap();
    table.walk(&json2).unwrap();
    table.finalize().unwrap();

    let compressed1 = table.compress(&json1).unwrap();
    let compressed2 = table.compress(&json2).unwrap();

    assert_eq!(
        table.get_table(),
        &[
            PackValue::Integer(0),
            PackValue::Integer(5),
            PackValue::Str("bar".into()),
            PackValue::Str("foo".into()),
        ]
    );
    assert_eq!(compressed1, obj(&[("3", PackValue::Integer(2))]));
    assert_eq!(
        compressed2,
        obj(&[(
            "3",
            PackValue::Array(vec![
                PackValue::Integer(0),
                PackValue::Integer(0),
                PackValue::Integer(1),
                PackValue::Integer(1),
            ])
        )])
    );
}

#[test]
fn decompression_table_matrix() {
    let json = obj(&[
        (
            "a",
            PackValue::Array(vec![
                PackValue::Integer(-10),
                PackValue::Integer(-5),
                PackValue::Integer(5),
                PackValue::Integer(100),
            ]),
        ),
        (
            "b",
            PackValue::Array(vec![
                PackValue::Bool(true),
                PackValue::Bool(false),
                PackValue::Null,
                PackValue::Null,
            ]),
        ),
        ("c", PackValue::Str("c".into())),
    ]);

    let table = CompressionTable::create(&json).unwrap();
    let compressed = table.compress(&json).unwrap();

    let mut decompression_table = DecompressionTable::new();
    decompression_table.import_table(table.get_table());

    assert_eq!(
        decompression_table
            .get_literal(table.get_index(&PackValue::Bool(true)).unwrap())
            .unwrap(),
        &PackValue::Bool(true)
    );
    assert_eq!(
        decompression_table
            .get_literal(table.get_index(&PackValue::Bool(false)).unwrap())
            .unwrap(),
        &PackValue::Bool(false)
    );
    assert_eq!(
        decompression_table
            .get_literal(table.get_index(&PackValue::Null).unwrap())
            .unwrap(),
        &PackValue::Null
    );
    assert_eq!(
        decompression_table
            .get_literal(table.get_index(&PackValue::Str("a".into())).unwrap())
            .unwrap(),
        &PackValue::Str("a".into())
    );

    let decompressed = decompression_table.decompress(&compressed).unwrap();
    assert_eq!(decompressed, json);
}

#[test]
fn to_data_uri_matrix() {
    assert_eq!(
        to_data_uri(&[0, 1, 2, 3], &[]),
        "data:application/octet-stream;base64,AAECAw=="
    );
    assert_eq!(
        to_data_uri(
            &[1, 2, 3],
            &[
                ("ext".to_owned(), "33".to_owned()),
                ("v".to_owned(), "1".to_owned())
            ],
        ),
        "data:application/octet-stream;base64;ext=33;v=1,AQID"
    );
}
