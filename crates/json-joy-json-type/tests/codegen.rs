//! Integration tests for the json-joy-json-type crate.
//!
//! Ports key tests from:
//! - json-type/src/__tests__/codegen.spec.ts
//! - json-type/src/jtd/__tests__/converter.spec.ts
//! - json-type/src/json-schema/__tests__/converter.spec.ts

use json_joy_json_type::{
    type_def::{ModuleType, TypeBuilder},
    validate, ErrorMode, ValidationResult, ValidatorOptions,
};
use serde_json::{json, Value};

fn t() -> TypeBuilder {
    TypeBuilder::new()
}

fn opts_bool() -> ValidatorOptions {
    ValidatorOptions {
        errors: ErrorMode::Boolean,
        ..Default::default()
    }
}

fn opts_str() -> ValidatorOptions {
    ValidatorOptions {
        errors: ErrorMode::String,
        ..Default::default()
    }
}

fn opts_obj() -> ValidatorOptions {
    ValidatorOptions {
        errors: ErrorMode::Object,
        ..Default::default()
    }
}

/// Run validation across all three error modes. The value should pass (is_ok == true).
fn assert_valid(type_node: &json_joy_json_type::TypeNode, value: Value) {
    for opts in [opts_bool(), opts_str(), opts_obj()] {
        let result = validate(&value, type_node, &opts, &[]);
        assert!(
            result.is_ok(),
            "expected valid for {:?}, got {:?}",
            value,
            result
        );
    }
}

/// Run validation across all three error modes. The value should fail (is_err == true).
fn assert_invalid(type_node: &json_joy_json_type::TypeNode, value: Value) {
    for opts in [opts_bool(), opts_str(), opts_obj()] {
        let result = validate(&value, type_node, &opts, &[]);
        assert!(
            result.is_err(),
            "expected invalid for {:?}, got {:?}",
            value,
            result
        );
    }
}

fn error_code(result: ValidationResult) -> String {
    match result {
        ValidationResult::ObjectError { code, .. } => code,
        _ => panic!("expected ObjectError, got {:?}", result),
    }
}

// ── Any type ─────────────────────────────────────────────────────────────────

#[test]
fn any_accepts_all_values() {
    let type_ = t().any();
    for v in [
        json!(1),
        json!("hello"),
        json!({}),
        json!([]),
        json!(null),
        json!(true),
    ] {
        assert_valid(&type_, v);
    }
}

// ── Bool type ────────────────────────────────────────────────────────────────

#[test]
fn bool_accepts_true_and_false() {
    let type_ = t().bool();
    assert_valid(&type_, json!(true));
    assert_valid(&type_, json!(false));
}

#[test]
fn bool_rejects_non_bool() {
    let type_ = t().bool();
    assert_invalid(&type_, json!(123));
    assert_invalid(&type_, json!("true"));
    assert_invalid(&type_, json!(null));
}

#[test]
fn bool_error_code_is_bool() {
    let type_ = t().bool();
    let result = validate(&json!(123), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "BOOL");
}

// ── Str type ─────────────────────────────────────────────────────────────────

#[test]
fn str_accepts_strings() {
    let type_ = t().str();
    assert_valid(&type_, json!(""));
    assert_valid(&type_, json!("hello"));
    assert_valid(&type_, json!("unicode: 🎉"));
}

#[test]
fn str_rejects_non_strings() {
    let type_ = t().str();
    assert_invalid(&type_, json!(123));
    assert_invalid(&type_, json!(null));
    assert_invalid(&type_, json!(true));
}

#[test]
fn str_error_code_is_str() {
    let type_ = t().str();
    let result = validate(&json!(123), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "STR");
}

#[test]
fn str_min_length() {
    let mut st = json_joy_json_type::type_def::StrType::new();
    st.schema.min = Some(3);
    let type_ = json_joy_json_type::TypeNode::Str(st);

    assert_valid(&type_, json!("abc"));
    assert_valid(&type_, json!("abcd"));
    assert_invalid(&type_, json!(""));
    assert_invalid(&type_, json!("ab"));
}

#[test]
fn str_max_length() {
    let mut st = json_joy_json_type::type_def::StrType::new();
    st.schema.max = Some(5);
    let type_ = json_joy_json_type::TypeNode::Str(st);

    assert_valid(&type_, json!("abcde"));
    assert_invalid(&type_, json!("abcdef"));
}

#[test]
fn str_min_max_length() {
    let mut st = json_joy_json_type::type_def::StrType::new();
    st.schema.min = Some(3);
    st.schema.max = Some(5);
    let type_ = json_joy_json_type::TypeNode::Str(st);

    assert_valid(&type_, json!("abc"));
    assert_valid(&type_, json!("abcd"));
    assert_valid(&type_, json!("abcde"));
    assert_invalid(&type_, json!("ab"));
    assert_invalid(&type_, json!("abcdef"));
}

#[test]
fn str_len_error_code() {
    let mut st = json_joy_json_type::type_def::StrType::new();
    st.schema.min = Some(3);
    let type_ = json_joy_json_type::TypeNode::Str(st);
    let result = validate(&json!("ab"), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "STR_LEN");
}

// ── Num type ─────────────────────────────────────────────────────────────────

#[test]
fn num_accepts_numbers() {
    let type_ = t().num();
    assert_valid(&type_, json!(123));
    assert_valid(&type_, json!(-123));
    assert_valid(&type_, json!(0));
    assert_valid(&type_, json!(1.5));
}

#[test]
fn num_rejects_non_numbers() {
    let type_ = t().num();
    assert_invalid(&type_, json!("123"));
    assert_invalid(&type_, json!(""));
    assert_invalid(&type_, json!(null));
    assert_invalid(&type_, json!(true));
}

#[test]
fn num_error_code_is_num() {
    let type_ = t().num();
    let result = validate(&json!("123"), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "NUM");
}

#[test]
fn num_integer_format_rejects_floats() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::I);
    let type_ = json_joy_json_type::TypeNode::Num(n);

    assert_valid(&type_, json!(123));
    assert_valid(&type_, json!(-123));
    assert_invalid(&type_, json!(123.4));
    assert_invalid(&type_, json!(-1.1));
}

#[test]
fn num_unsigned_format_rejects_negatives() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::U);
    let type_ = json_joy_json_type::TypeNode::Num(n);

    assert_valid(&type_, json!(0));
    assert_valid(&type_, json!(123));
    assert_invalid(&type_, json!(-1));
}

#[test]
fn num_u8_range() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::U8);
    let type_ = json_joy_json_type::TypeNode::Num(n);

    assert_valid(&type_, json!(0));
    assert_valid(&type_, json!(255));
    assert_invalid(&type_, json!(256));
    assert_invalid(&type_, json!(-1));
}

#[test]
fn num_i8_range() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::I8);
    let type_ = json_joy_json_type::TypeNode::Num(n);

    assert_valid(&type_, json!(-128));
    assert_valid(&type_, json!(127));
    assert_invalid(&type_, json!(128));
    assert_invalid(&type_, json!(-129));
}

#[test]
fn num_u32_range() {
    use json_joy_json_type::schema::NumFormat;
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.format = Some(NumFormat::U32);
    let type_ = json_joy_json_type::TypeNode::Num(n);

    assert_valid(&type_, json!(0));
    assert_valid(&type_, json!(4294967295u64));
    assert_invalid(&type_, json!(4294967296u64));
    assert_invalid(&type_, json!(-1));
}

// ── Const type ───────────────────────────────────────────────────────────────

#[test]
fn con_accepts_exact_value() {
    let type_ = t().Const(json!("foo"), None);
    assert_valid(&type_, json!("foo"));
    assert_invalid(&type_, json!("bar"));
    assert_invalid(&type_, json!(123));
    assert_invalid(&type_, json!(null));
}

#[test]
fn con_bool_const() {
    let type_ = t().Const(json!(true), None);
    assert_valid(&type_, json!(true));
    assert_invalid(&type_, json!(false));
    assert_invalid(&type_, json!("true"));
}

#[test]
fn con_zero_falsy_const() {
    let type_ = t().Const(json!(0), None);
    assert_valid(&type_, json!(0));
    assert_invalid(&type_, json!(1));
}

#[test]
fn con_empty_string_const() {
    let type_ = t().Const(json!(""), None);
    assert_valid(&type_, json!(""));
    assert_invalid(&type_, json!("a"));
    assert_invalid(&type_, json!(null));
}

#[test]
fn con_null_const() {
    let type_ = t().Const(json!(null), None);
    assert_valid(&type_, json!(null));
    assert_invalid(&type_, json!("null"));
    assert_invalid(&type_, json!(0));
}

#[test]
fn con_error_code_is_const() {
    let type_ = t().Const(json!("foo"), None);
    let result = validate(&json!("bar"), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "CONST");
}

// ── Arr type ─────────────────────────────────────────────────────────────────

#[test]
fn arr_any_accepts_arrays() {
    let type_ = t().Array(t().any(), None);
    assert_valid(&type_, json!([]));
    assert_valid(&type_, json!([1]));
    assert_valid(&type_, json!([1, "a"]));
    assert_valid(&type_, json!([1, {}]));
}

#[test]
fn arr_rejects_non_arrays() {
    let type_ = t().Array(t().any(), None);
    assert_invalid(&type_, json!({}));
    assert_invalid(&type_, json!(null));
    assert_invalid(&type_, json!(123));
}

#[test]
fn arr_validates_element_type() {
    let type_ = t().Array(t().num(), None);
    assert_valid(&type_, json!([1, 2, 3]));
    assert_invalid(&type_, json!([1, "a"]));
}

#[test]
fn arr_element_error_path() {
    let type_ = t().Array(t().num(), None);
    let result = validate(&json!([1, "a"]), &type_, &opts_obj(), &[]);
    match result {
        ValidationResult::ObjectError { path, code, .. } => {
            assert_eq!(code, "NUM");
            assert_eq!(path, vec![json!(1)]);
        }
        _ => panic!("expected ObjectError"),
    }
}

#[test]
fn arr_error_code_is_arr() {
    let type_ = t().Array(t().num(), None);
    let result = validate(&json!({}), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "ARR");
}

// ── Obj type ─────────────────────────────────────────────────────────────────

#[test]
fn obj_accepts_objects() {
    let type_ = t().Object(vec![]);
    assert_valid(&type_, json!({}));
    assert_valid(&type_, json!({"a": "b"}));
}

#[test]
fn obj_rejects_non_objects() {
    let type_ = t().Object(vec![]);
    assert_invalid(&type_, json!(null));
    assert_invalid(&type_, json!([]));
    assert_invalid(&type_, json!(123));
}

#[test]
fn obj_required_key_presence() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![KeyType::new("foo", t().any())]);
    assert_valid(&type_, json!({"foo": 123}));
    let result = validate(&json!({}), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "KEY");
}

#[test]
fn obj_extra_key_check() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![KeyType::new("foo", t().any())]);
    let opts = ValidatorOptions {
        errors: ErrorMode::Object,
        skip_object_extra_fields_check: false,
        ..Default::default()
    };
    let result = validate(&json!({"foo": 1, "bar": 2}), &type_, &opts, &[]);
    assert_eq!(error_code(result), "KEYS");
}

#[test]
fn obj_extra_key_check_skipped() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![KeyType::new("foo", t().any())]);
    let opts = ValidatorOptions {
        errors: ErrorMode::Object,
        skip_object_extra_fields_check: true,
        ..Default::default()
    };
    let result = validate(&json!({"foo": 1, "bar": 2}), &type_, &opts, &[]);
    assert!(result.is_ok());
}

#[test]
fn obj_nested_key_error_path() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![KeyType::new("num", t().num())]);
    let opts = opts_obj();
    let result = validate(&json!({"num": "not_a_num"}), &type_, &opts, &[]);
    match result {
        ValidationResult::ObjectError { path, code, .. } => {
            assert_eq!(code, "NUM");
            assert_eq!(path, vec![json!("num")]);
        }
        _ => panic!("expected ObjectError"),
    }
}

// ── Map type ─────────────────────────────────────────────────────────────────

#[test]
fn map_accepts_objects() {
    let type_ = t().Map(t().any(), None, None);
    assert_valid(&type_, json!({}));
    assert_valid(&type_, json!({"a": "b"}));
    assert_valid(&type_, json!({"a": 123}));
    assert_valid(&type_, json!({"a": null}));
}

#[test]
fn map_rejects_arrays() {
    let type_ = t().Map(t().any(), None, None);
    assert_invalid(&type_, json!([]));
}

#[test]
fn map_validates_value_type() {
    let type_ = t().Map(t().num(), None, None);
    assert_valid(&type_, json!({"a": 123}));
    let result = validate(&json!({"a": "123"}), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result.clone()), "NUM");
    match result {
        ValidationResult::ObjectError { path, .. } => {
            assert_eq!(path, vec![json!("a")]);
        }
        _ => panic!("expected ObjectError"),
    }
}

// ── Or type ──────────────────────────────────────────────────────────────────

#[test]
fn or_validates_first_matching_type() {
    let type_ = t().Or(vec![t().num(), t().str()]);
    assert_valid(&type_, json!(123));
    assert_valid(&type_, json!("hello"));
    assert_invalid(&type_, json!(null));
    assert_invalid(&type_, json!([]));
}

#[test]
fn or_error_code_is_or() {
    let type_ = t().Or(vec![t().num(), t().str()]);
    let result = validate(&json!(null), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result), "OR");
}

// ── Tuple type ───────────────────────────────────────────────────────────────

#[test]
fn tuple_validates_positional_elements() {
    let type_ = t().Tuple(vec![t().num(), t().str()], None, None);
    assert_valid(&type_, json!([0, ""]));
    // wrong type at position 0
    let result = validate(&json!(["", ""]), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result.clone()), "NUM");
    match result {
        ValidationResult::ObjectError { path, .. } => {
            assert_eq!(path, vec![json!(0)]);
        }
        _ => panic!(),
    }
    // wrong type at position 1
    let result = validate(&json!([0, 1]), &type_, &opts_obj(), &[]);
    assert_eq!(error_code(result.clone()), "STR");
    match result {
        ValidationResult::ObjectError { path, .. } => {
            assert_eq!(path, vec![json!(1)]);
        }
        _ => panic!(),
    }
}

// ── Boolean error mode ───────────────────────────────────────────────────────

#[test]
fn boolean_error_mode() {
    let type_ = t().num();
    let opts = ValidatorOptions {
        errors: ErrorMode::Boolean,
        ..Default::default()
    };
    assert_eq!(
        validate(&json!(123), &type_, &opts, &[]),
        ValidationResult::Ok
    );
    assert_eq!(
        validate(&json!("x"), &type_, &opts, &[]),
        ValidationResult::BoolError
    );
}

// ── String error mode ────────────────────────────────────────────────────────

#[test]
fn string_error_mode() {
    use json_joy_json_type::type_def::KeyType;
    let type_ = t().Object(vec![KeyType::new("num", t().num())]);
    let opts = ValidatorOptions {
        errors: ErrorMode::String,
        ..Default::default()
    };
    let result = validate(&json!({"num": "bad"}), &type_, &opts, &[]);
    match result {
        ValidationResult::StringError(s) => {
            // Should be a non-empty JSON path string
            assert!(!s.is_empty());
        }
        _ => panic!("expected StringError, got {:?}", result),
    }
}

// ── Module + Ref ─────────────────────────────────────────────────────────────

#[test]
fn ref_resolves_through_module() {
    use json_joy_json_type::type_def::{BaseInfo, RefType};
    use json_joy_json_type::TypeNode;
    use std::sync::Arc;

    let module = Arc::new(ModuleType::new());
    // Register a 'MyNum' alias
    module.alias(
        "MyNum",
        json_joy_json_type::schema::Schema::Num(json_joy_json_type::schema::NumSchema::default()),
    );

    let mut ref_type = RefType::new("MyNum");
    ref_type.base = BaseInfo::new().with_system(Some(module));
    let type_ = TypeNode::Ref(ref_type);

    assert_valid(&type_, json!(42));
    assert_invalid(&type_, json!("not a number"));
}

// ── Num range constraints ────────────────────────────────────────────────────

#[test]
fn num_gt_constraint() {
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.gt = Some(5.0);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    assert_valid(&type_, json!(6));
    assert_invalid(&type_, json!(5));
    assert_invalid(&type_, json!(4));
}

#[test]
fn num_gte_constraint() {
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.gte = Some(5.0);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    assert_valid(&type_, json!(5));
    assert_valid(&type_, json!(6));
    assert_invalid(&type_, json!(4));
}

#[test]
fn num_lt_constraint() {
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.lt = Some(10.0);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    assert_valid(&type_, json!(9));
    assert_invalid(&type_, json!(10));
    assert_invalid(&type_, json!(11));
}

#[test]
fn num_lte_constraint() {
    let mut n = json_joy_json_type::type_def::NumType::new();
    n.schema.lte = Some(10.0);
    let type_ = json_joy_json_type::TypeNode::Num(n);
    assert_valid(&type_, json!(10));
    assert_valid(&type_, json!(9));
    assert_invalid(&type_, json!(11));
}
