use json_joy_json_pack::xdr::{
    XdrDiscriminant, XdrSchema, XdrSchemaValidator, XdrUnionValue, XdrValue,
};

#[test]
fn xdr_schema_validator_schema_matrix() {
    let validator = XdrSchemaValidator::new();

    let primitive_cases = vec![
        XdrSchema::Void,
        XdrSchema::Int,
        XdrSchema::UnsignedInt,
        XdrSchema::Boolean,
        XdrSchema::Hyper,
        XdrSchema::UnsignedHyper,
        XdrSchema::Float,
        XdrSchema::Double,
        XdrSchema::Quadruple,
    ];
    for schema in primitive_cases {
        assert!(validator.validate_schema(&schema));
    }

    assert!(validator.validate_schema(&XdrSchema::Enum(vec![
        ("RED".into(), 0),
        ("GREEN".into(), 1),
        ("BLUE".into(), 2),
    ])));
    assert!(!validator.validate_schema(&XdrSchema::Enum(vec![
        ("RED".into(), 0),
        ("GREEN".into(), 1),
        ("BLUE".into(), 1),
    ])));

    assert!(validator.validate_schema(&XdrSchema::Struct(vec![
        (Box::new(XdrSchema::Int), "id".into()),
        (Box::new(XdrSchema::Str(None)), "name".into()),
    ])));
    assert!(!validator.validate_schema(&XdrSchema::Struct(vec![
        (Box::new(XdrSchema::Int), "id".into()),
        (Box::new(XdrSchema::Str(None)), "id".into()),
    ])));
    assert!(!validator.validate_schema(&XdrSchema::Struct(vec![(
        Box::new(XdrSchema::Int),
        "".into(),
    )])));

    assert!(validator.validate_schema(&XdrSchema::Union {
        arms: vec![
            (XdrDiscriminant::Int(0), Box::new(XdrSchema::Int)),
            (
                XdrDiscriminant::Str("red".into()),
                Box::new(XdrSchema::Str(None))
            ),
            (XdrDiscriminant::Bool(true), Box::new(XdrSchema::Boolean),),
        ],
        default: Some(Box::new(XdrSchema::Void)),
    }));
    assert!(!validator.validate_schema(&XdrSchema::Union {
        arms: vec![],
        default: None,
    }));
    assert!(!validator.validate_schema(&XdrSchema::Union {
        arms: vec![
            (XdrDiscriminant::Int(0), Box::new(XdrSchema::Int)),
            (XdrDiscriminant::Int(0), Box::new(XdrSchema::Str(None))),
        ],
        default: None,
    }));
}

#[test]
fn xdr_schema_validator_value_matrix() {
    let validator = XdrSchemaValidator::new();

    assert!(validator.validate_value(&XdrValue::Void, &XdrSchema::Void));
    assert!(validator.validate_value(&XdrValue::Int(42), &XdrSchema::Int));
    assert!(!validator.validate_value(&XdrValue::Str("42".into()), &XdrSchema::Int));
    assert!(validator.validate_value(&XdrValue::UnsignedInt(42), &XdrSchema::UnsignedInt));
    assert!(!validator.validate_value(&XdrValue::Int(-1), &XdrSchema::UnsignedInt));
    assert!(validator.validate_value(&XdrValue::Bool(true), &XdrSchema::Boolean));

    let enum_schema = XdrSchema::Enum(vec![
        ("RED".into(), 0),
        ("GREEN".into(), 1),
        ("BLUE".into(), 2),
    ]);
    assert!(validator.validate_value(&XdrValue::Enum("RED".into()), &enum_schema));
    assert!(!validator.validate_value(&XdrValue::Enum("YELLOW".into()), &enum_schema));

    assert!(validator.validate_value(&XdrValue::Bytes(vec![1, 2, 3, 4]), &XdrSchema::Opaque(4)));
    assert!(!validator.validate_value(&XdrValue::Bytes(vec![1, 2, 3]), &XdrSchema::Opaque(4)));
    assert!(validator.validate_value(
        &XdrValue::Bytes(vec![1, 2, 3]),
        &XdrSchema::VarOpaque(Some(10))
    ));
    assert!(!validator.validate_value(
        &XdrValue::Bytes(vec![1; 11]),
        &XdrSchema::VarOpaque(Some(10))
    ));

    assert!(validator.validate_value(&XdrValue::Str("hello".into()), &XdrSchema::Str(Some(10))));
    assert!(!validator.validate_value(
        &XdrValue::Str("this is too long".into()),
        &XdrSchema::Str(Some(10))
    ));

    let array_schema = XdrSchema::Array {
        element: Box::new(XdrSchema::Int),
        size: 3,
    };
    assert!(validator.validate_value(
        &XdrValue::Array(vec![XdrValue::Int(1), XdrValue::Int(2), XdrValue::Int(3)]),
        &array_schema
    ));
    assert!(!validator.validate_value(
        &XdrValue::Array(vec![XdrValue::Int(1), XdrValue::Int(2)]),
        &array_schema
    ));

    let varray_schema = XdrSchema::VarArray {
        element: Box::new(XdrSchema::Int),
        max_size: Some(5),
    };
    assert!(validator.validate_value(
        &XdrValue::Array(vec![XdrValue::Int(1), XdrValue::Int(2), XdrValue::Int(3)]),
        &varray_schema
    ));
    assert!(!validator.validate_value(
        &XdrValue::Array(vec![
            XdrValue::Int(1),
            XdrValue::Int(2),
            XdrValue::Int(3),
            XdrValue::Int(4),
            XdrValue::Int(5),
            XdrValue::Int(6),
        ]),
        &varray_schema
    ));

    let struct_schema = XdrSchema::Struct(vec![
        (Box::new(XdrSchema::Int), "id".into()),
        (Box::new(XdrSchema::Str(None)), "name".into()),
    ]);
    assert!(validator.validate_value(
        &XdrValue::Struct(vec![
            ("id".into(), XdrValue::Int(42)),
            ("name".into(), XdrValue::Str("test".into())),
        ]),
        &struct_schema
    ));
    assert!(!validator.validate_value(
        &XdrValue::Struct(vec![("id".into(), XdrValue::Int(42))]),
        &struct_schema
    ));

    let union_schema = XdrSchema::Union {
        arms: vec![
            (XdrDiscriminant::Int(0), Box::new(XdrSchema::Int)),
            (XdrDiscriminant::Int(1), Box::new(XdrSchema::Str(None))),
        ],
        default: None,
    };
    assert!(validator.validate_value(
        &XdrValue::Union(Box::new(XdrUnionValue {
            discriminant: XdrDiscriminant::Int(0),
            value: XdrValue::Int(42),
        })),
        &union_schema
    ));
    assert!(!validator.validate_value(
        &XdrValue::Union(Box::new(XdrUnionValue {
            discriminant: XdrDiscriminant::Int(2),
            value: XdrValue::Int(42),
        })),
        &union_schema
    ));

    let union_default_schema = XdrSchema::Union {
        arms: vec![(XdrDiscriminant::Int(0), Box::new(XdrSchema::Int))],
        default: Some(Box::new(XdrSchema::Str(None))),
    };
    assert!(validator.validate_value(
        &XdrValue::Union(Box::new(XdrUnionValue {
            discriminant: XdrDiscriminant::Int(2),
            value: XdrValue::Str("fallback".into()),
        })),
        &union_default_schema
    ));

    let optional_schema = XdrSchema::Optional(Box::new(XdrSchema::Int));
    assert!(validator.validate_value(&XdrValue::Optional(None), &optional_schema));
    assert!(validator.validate_value(
        &XdrValue::Optional(Some(Box::new(XdrValue::Int(11)))),
        &optional_schema
    ));
    assert!(!validator.validate_value(
        &XdrValue::Optional(Some(Box::new(XdrValue::Str("bad".into())))),
        &optional_schema
    ));

    assert!(validator.validate_value(&XdrValue::Int(1), &XdrSchema::Const(123)));
}
