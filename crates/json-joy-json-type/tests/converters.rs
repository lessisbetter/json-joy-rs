//! Integration tests for the json-joy-json-type converters.
//!
//! Ports key tests from:
//! - json-type/src/jtd/__tests__/converter.spec.ts
//! - json-type/src/json-schema/__tests__/converter.spec.ts

use json_joy_json_type::{
    json_schema::type_to_json_schema,
    jtd::{to_jtd_form, types::JtdType, JtdForm},
    type_def::TypeBuilder,
};
use serde_json::json;

fn t() -> TypeBuilder {
    TypeBuilder::new()
}

// ── JTD Converter ─────────────────────────────────────────────────────────────

#[test]
fn jtd_str_type() {
    let form = to_jtd_form(&t().str());
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::String
        }
    ));
}

#[test]
fn jtd_bool_type() {
    let form = to_jtd_form(&t().bool());
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::Boolean
        }
    ));
}

#[test]
fn jtd_num_with_u8_format() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::U8);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    let form = to_jtd_form(&type_);
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::Uint8
        }
    ));
}

#[test]
fn jtd_num_default_is_float64() {
    let form = to_jtd_form(&t().num());
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::Float64
        }
    ));
}

#[test]
fn jtd_any_type() {
    let form = to_jtd_form(&t().any());
    assert!(matches!(form, JtdForm::Empty { nullable: true }));
}

#[test]
fn jtd_const_string() {
    let type_ = t().Const(json!("hello"), None);
    let form = to_jtd_form(&type_);
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::String
        }
    ));
}

#[test]
fn jtd_const_number_uint8() {
    let type_ = t().Const(json!(255), None);
    let form = to_jtd_form(&type_);
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::Uint8
        }
    ));
}

#[test]
fn jtd_const_bool() {
    let type_ = t().Const(json!(true), None);
    let form = to_jtd_form(&type_);
    assert!(matches!(
        form,
        JtdForm::Type {
            type_: JtdType::Boolean
        }
    ));
}

#[test]
fn jtd_array_of_str() {
    let type_ = t().Array(t().str(), None);
    let form = to_jtd_form(&type_);
    match form {
        JtdForm::Elements { elements } => {
            assert!(matches!(
                *elements,
                JtdForm::Type {
                    type_: JtdType::String
                }
            ));
        }
        _ => panic!("expected Elements form"),
    }
}

#[test]
fn jtd_object_with_required_and_optional() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![
        KeyType::new("name", t().str()),
        KeyType::new_opt("age", t().num()),
    ]);
    let form = to_jtd_form(&type_);
    match form {
        JtdForm::Properties {
            properties,
            optional_properties,
            ..
        } => {
            assert!(properties.contains_key("name"));
            assert!(optional_properties.contains_key("age"));
            assert!(!properties.contains_key("age"));
        }
        _ => panic!("expected Properties form"),
    }
}

#[test]
fn jtd_map_type() {
    let type_ = t().Map(t().str(), None, None);
    let form = to_jtd_form(&type_);
    match form {
        JtdForm::Values { values } => {
            assert!(matches!(
                *values,
                JtdForm::Type {
                    type_: JtdType::String
                }
            ));
        }
        _ => panic!("expected Values form"),
    }
}

#[test]
fn jtd_ref_type() {
    let type_ = t().Ref("MyType");
    let form = to_jtd_form(&type_);
    match form {
        JtdForm::Ref { ref_ } => assert_eq!(ref_, "MyType"),
        _ => panic!("expected Ref form"),
    }
}

// ── JSON Schema Converter ─────────────────────────────────────────────────────

#[test]
fn json_schema_str() {
    let schema = type_to_json_schema(&t().str());
    assert_eq!(schema["type"], json!("string"));
}

#[test]
fn json_schema_num() {
    let schema = type_to_json_schema(&t().num());
    assert_eq!(schema["type"], json!("number"));
}

#[test]
fn json_schema_integer_format() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::I32);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    let schema = type_to_json_schema(&type_);
    assert_eq!(schema["type"], json!("integer"));
}

#[test]
fn json_schema_bool() {
    let schema = type_to_json_schema(&t().bool());
    assert_eq!(schema["type"], json!("boolean"));
}

#[test]
fn json_schema_any() {
    let schema = type_to_json_schema(&t().any());
    assert!(schema["type"].is_array());
}

#[test]
fn json_schema_arr() {
    let schema = type_to_json_schema(&t().Array(t().str(), None));
    assert_eq!(schema["type"], json!("array"));
    assert_eq!(schema["items"]["type"], json!("string"));
}

#[test]
fn json_schema_obj_with_required_and_optional() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![
        KeyType::new("name", t().str()),
        KeyType::new_opt("age", t().num()),
    ]);
    let schema = type_to_json_schema(&type_);
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["properties"]["name"]["type"], json!("string"));
    assert_eq!(schema["properties"]["age"]["type"], json!("number"));
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("name")));
    assert!(!required.contains(&json!("age")));
}

#[test]
fn json_schema_map() {
    let schema = type_to_json_schema(&t().Map(t().str(), None, None));
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["patternProperties"][".*"]["type"], json!("string"));
}

#[test]
fn json_schema_or() {
    let schema = type_to_json_schema(&t().Or(vec![t().str(), t().num()]));
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
}

#[test]
fn json_schema_ref() {
    let schema = type_to_json_schema(&t().Ref("MyType"));
    assert_eq!(schema["$ref"], json!("#/$defs/MyType"));
}

#[test]
fn json_schema_str_with_min_max() {
    let mut st = json_joy_json_type::type_def::StrType::new();
    st.schema.min = Some(3);
    st.schema.max = Some(10);
    let type_ = json_joy_json_type::TypeNode::Str(st);
    let schema = type_to_json_schema(&type_);
    assert_eq!(schema["minLength"], json!(3));
    assert_eq!(schema["maxLength"], json!(10));
}

#[test]
fn json_schema_num_with_range() {
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.gt = Some(0.0);
    n.schema.lte = Some(100.0);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    let schema = type_to_json_schema(&type_);
    assert_eq!(schema["exclusiveMinimum"], json!(0.0));
    assert_eq!(schema["maximum"], json!(100.0));
}

#[test]
fn json_schema_bin() {
    let schema = type_to_json_schema(&t().bin());
    assert_eq!(schema["type"], json!("binary"));
}
