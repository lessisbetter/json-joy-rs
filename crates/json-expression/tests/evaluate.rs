//! Integration tests for the json-expression `evaluate` function.
//!
//! Mirrors the upstream test suites:
//! - `__tests__/evaluate.spec.ts`
//! - `__tests__/jsonExpressionEvaluateTests.ts`
//! - `__tests__/jsonExpressionUnitTests.ts`

use json_expression::{evaluate, operators_map, EvalCtx, JsValue, Vars};
use serde_json::{json, Value};
use std::sync::Arc;

fn check(expression: Value, expected: Value, data: Value) {
    let ops = Arc::new(operators_map());
    let mut vars = Vars::new(data);
    let mut ctx = EvalCtx::new(&mut vars, ops);
    let result = evaluate(&expression, &mut ctx)
        .unwrap_or_else(|e| panic!("evaluate({}) failed: {}", expression, e));
    let result_val = match result {
        JsValue::Json(v) => v,
        JsValue::Undefined => Value::Null,
        JsValue::Binary(_) => Value::Null,
    };
    assert_eq!(result_val, expected, "expression: {}", expression);
}

fn check_err(expression: Value, data: Value) -> String {
    let ops = Arc::new(operators_map());
    let mut vars = Vars::new(data);
    let mut ctx = EvalCtx::new(&mut vars, ops);
    evaluate(&expression, &mut ctx)
        .err()
        .unwrap_or_else(|| panic!("expected error for {}", expression))
        .to_string()
}

// ----------------------------------------------------------------- Arithmetic

#[test]
fn test_add() {
    check(json!(["add", 1, 2]), json!(3.0), json!(null));
    check(json!(["+", 1, 2]), json!(3.0), json!(null));
    check(json!(["add", 1, ["add", 1, 1]]), json!(3.0), json!(null));
    check(json!(["+", 1, ["+", 1, 1]]), json!(3.0), json!(null));
    check(json!(["add", 1, 1, 1, 1]), json!(4.0), json!(null));
    check(json!(["+", 1, 2, 3, 4]), json!(10.0), json!(null));
    check(json!(["add", "2", "2"]), json!(4.0), json!(null));
    check(json!(["+", "1", "10.5"]), json!(11.5), json!(null));
}

#[test]
fn test_add_arity_error() {
    let err = check_err(json!(["add", 1]), json!(null));
    assert!(err.contains("at least two operands"), "got: {}", err);
    let err = check_err(json!(["+", 1]), json!(null));
    assert!(err.contains("at least two operands"), "got: {}", err);
}

#[test]
fn test_subtract() {
    check(json!(["subtract", 1, 2]), json!(-1.0), json!(null));
    check(json!(["-", 1, 2]), json!(-1.0), json!(null));
    check(json!(["subtract", 1, 1, 1, 1]), json!(-2.0), json!(null));
    check(json!(["-", 1, 2, 3, 4]), json!(-8.0), json!(null));
}

#[test]
fn test_multiply() {
    check(json!(["multiply", 1, 2]), json!(2.0), json!(null));
    check(json!(["*", 3, 2]), json!(6.0), json!(null));
    check(json!(["multiply", 2, 2, 2, 2]), json!(16.0), json!(null));
    check(json!(["*", 1, 2, 3, 4]), json!(24.0), json!(null));
}

#[test]
fn test_divide() {
    check(json!(["/", 10, 2]), json!(5.0), json!(null));
    check(json!(["/", 1, 4]), json!(0.25), json!(null));
}

#[test]
fn test_divide_by_zero() {
    let err = check_err(json!(["/", 1, 0]), json!(null));
    assert!(err.contains("DIVISION_BY_ZERO"), "got: {}", err);
}

#[test]
fn test_mod() {
    check(json!(["%", 10, 3]), json!(1.0), json!(null));
    check(json!(["mod", 7, 2]), json!(1.0), json!(null));
}

#[test]
fn test_min_max() {
    check(json!(["min", 3, 1, 2]), json!(1.0), json!(null));
    check(json!(["max", 3, 1, 2]), json!(3.0), json!(null));
}

#[test]
fn test_round_ceil_floor_trunc() {
    check(json!(["round", 1.5]), json!(2.0), json!(null));
    check(json!(["ceil", 1.1]), json!(2.0), json!(null));
    check(json!(["floor", 1.9]), json!(1.0), json!(null));
    check(json!(["trunc", -1.9]), json!(-1.0), json!(null));
}

#[test]
fn test_abs() {
    check(json!(["abs", -5]), json!(5.0), json!(null));
    check(json!(["abs", 5]), json!(5.0), json!(null));
}

#[test]
fn test_pow() {
    check(json!(["**", 2, 10]), json!(1024.0), json!(null));
    check(json!(["pow", 3, 2]), json!(9.0), json!(null));
}

// ----------------------------------------------------------------- Comparison

#[test]
fn test_eq() {
    check(json!(["eq", 1, 1]), json!(true), json!(null));
    check(json!(["==", 1, 2]), json!(false), json!(null));
    check(
        json!(["eq", {"foo": "bar"}, {"foo": "bar"}]),
        json!(true),
        json!(null),
    );
    check(
        json!(["eq", {"foo": "bar"}, {"foo": "baz"}]),
        json!(false),
        json!(null),
    );
}

#[test]
fn test_ne() {
    check(json!(["ne", 1, 2]), json!(true), json!(null));
    check(json!(["!=", 1, 1]), json!(false), json!(null));
}

#[test]
fn test_gt_lt_ge_le() {
    check(json!([">", 2, 1]), json!(true), json!(null));
    check(json!(["gt", 1, 2]), json!(false), json!(null));
    check(json!(["<", 1, 2]), json!(true), json!(null));
    check(json!(["lt", 2, 1]), json!(false), json!(null));
    check(json!([">=", 2, 2]), json!(true), json!(null));
    check(json!(["<=", 1, 2]), json!(true), json!(null));
}

#[test]
fn test_cmp() {
    check(json!(["cmp", 1, 2]), json!(-1_i64), json!(null));
    check(json!(["cmp", 2, 1]), json!(1_i64), json!(null));
    check(json!(["cmp", 1, 1]), json!(0_i64), json!(null));
}

#[test]
fn test_between() {
    check(json!(["=><=", 5, 1, 10]), json!(true), json!(null));
    check(json!(["between", 0, 1, 10]), json!(false), json!(null));
    check(json!(["><", 5, 1, 10]), json!(true), json!(null));
    check(json!(["><", 1, 1, 10]), json!(false), json!(null));
}

// ----------------------------------------------------------------- Logical

#[test]
fn test_and() {
    check(json!(["&&", true, true]), json!(true), json!(null));
    check(json!(["&&", true, false]), json!(false), json!(null));
    check(json!(["and", false, true]), json!(false), json!(null));
    check(json!(["&&", 1, 1]), json!(1_i64), json!(null));
    check(json!(["&&", 1, 0]), json!(0_i64), json!(null));
}

#[test]
fn test_or() {
    check(json!(["||", true, false]), json!(true), json!(null));
    check(json!(["||", false, false]), json!(false), json!(null));
    check(json!(["or", false, true]), json!(true), json!(null));
}

#[test]
fn test_not() {
    check(json!(["!", true]), json!(false), json!(null));
    check(json!(["not", false]), json!(true), json!(null));
    check(json!(["!", 0]), json!(true), json!(null));
}

// ----------------------------------------------------------------- Type

#[test]
fn test_type_operator() {
    check(json!(["type", null]), json!("null"), json!(null));
    check(json!(["type", true]), json!("boolean"), json!(null));
    check(json!(["type", 1]), json!("number"), json!(null));
    check(json!(["type", "hello"]), json!("string"), json!(null));
    check(json!(["type", []]), json!("array"), json!(null));
    check(json!(["type", {}]), json!("object"), json!(null));
}

#[test]
fn test_bool_operator() {
    check(json!(["bool", 0]), json!(false), json!(null));
    check(json!(["bool", 1]), json!(true), json!(null));
    check(json!(["bool", ""]), json!(false), json!(null));
    check(json!(["bool", "hello"]), json!(true), json!(null));
}

#[test]
fn test_num_operator() {
    check(json!(["num", "42"]), json!(42.0), json!(null));
    check(json!(["num", true]), json!(1.0), json!(null));
    check(json!(["num", false]), json!(0.0), json!(null));
    check(json!(["num", null]), json!(0.0), json!(null));
}

#[test]
fn test_str_operator() {
    check(json!(["str", 42]), json!("42"), json!(null));
    check(json!(["str", true]), json!("true"), json!(null));
    check(json!(["str", null]), json!("null"), json!(null));
}

#[test]
fn test_type_checks() {
    check(json!(["nil?", null]), json!(true), json!(null));
    check(json!(["nil?", 0]), json!(false), json!(null));
    check(json!(["bool?", true]), json!(true), json!(null));
    check(json!(["bool?", 1]), json!(false), json!(null));
    check(json!(["num?", 1]), json!(true), json!(null));
    check(json!(["num?", "1"]), json!(false), json!(null));
    check(json!(["str?", "hello"]), json!(true), json!(null));
    check(json!(["str?", 1]), json!(false), json!(null));
    check(json!(["arr?", []]), json!(true), json!(null));
    check(json!(["arr?", {}]), json!(false), json!(null));
    check(json!(["obj?", {}]), json!(true), json!(null));
    check(json!(["obj?", []]), json!(false), json!(null));
}

// ----------------------------------------------------------------- Input / get

#[test]
fn test_get_from_data() {
    let data = json!({"a": {"b": {"c": 1}}});
    check(json!(["$", "/a/b/c"]), json!(1), data.clone());
    check(json!(["get", "/a/b/c"]), json!(1), data.clone());
}

#[test]
fn test_get_with_varname() {
    let data = json!({"x": 42});
    check(json!(["$", "/x"]), json!(42), data);
}

#[test]
fn test_get_not_found_throws() {
    let err = check_err(json!(["$", "/missing"]), json!(null));
    assert!(err.contains("NOT_FOUND"), "got: {}", err);
}

#[test]
fn test_get_with_default() {
    check(json!(["$", "/missing", 99]), json!(99), json!(null));
}

#[test]
fn test_defined() {
    let data = json!({"a": 1});
    check(json!(["$?", "/a"]), json!(true), data.clone());
    check(json!(["$?", "/b"]), json!(false), data);
}

// ----------------------------------------------------------------- Branching

#[test]
fn test_if() {
    check(json!(["?", true, 1, 2]), json!(1), json!(null));
    check(json!(["if", false, 1, 2]), json!(2), json!(null));
    check(json!(["?", 0, "yes", "no"]), json!("no"), json!(null));
}

#[test]
fn test_throw() {
    let err = check_err(json!(["throw", "oops"]), json!(null));
    assert!(err.contains("oops"), "got: {}", err);
}

// ----------------------------------------------------------------- Container

#[test]
fn test_len() {
    check(json!(["len", "hello"]), json!(5_i64), json!(null));
    check(json!(["len", [[1, 2, 3]]]), json!(3_i64), json!(null));
    check(json!(["len", {"a": 1, "b": 2}]), json!(2_i64), json!(null));
}

#[test]
fn test_member() {
    check(json!(["[]", [[10, 20, 30]], 1]), json!(20), json!(null));
    check(json!(["member", {"a": 99}, "a"]), json!(99), json!(null));
}

// ----------------------------------------------------------------- String

#[test]
fn test_cat() {
    check(
        json!([".", "hello", " ", "world"]),
        json!("hello world"),
        json!(null),
    );
    check(json!(["cat", "foo", "bar"]), json!("foobar"), json!(null));
}

#[test]
fn test_contains() {
    check(
        json!(["contains", "foobar", "oba"]),
        json!(true),
        json!(null),
    );
    check(
        json!(["contains", "foobar", "xyz"]),
        json!(false),
        json!(null),
    );
}

#[test]
fn test_starts() {
    check(json!(["starts", "foobar", "foo"]), json!(true), json!(null));
    check(
        json!(["starts", "foobar", "bar"]),
        json!(false),
        json!(null),
    );
}

#[test]
fn test_ends() {
    check(json!(["ends", "foobar", "bar"]), json!(true), json!(null));
    check(json!(["ends", "foobar", "foo"]), json!(false), json!(null));
}

#[test]
fn test_substr() {
    check(
        json!(["substr", "hello world", 0, 5]),
        json!("hello"),
        json!(null),
    );
    check(
        json!(["substr", "hello world", 6, 11]),
        json!("world"),
        json!(null),
    );
}

#[test]
fn test_email_validator() {
    check(
        json!(["email?", "user@example.com"]),
        json!(true),
        json!(null),
    );
    check(json!(["email?", "not-an-email"]), json!(false), json!(null));
}

#[test]
fn test_uuid_validator() {
    check(
        json!(["uuid?", "550e8400-e29b-41d4-a716-446655440000"]),
        json!(true),
        json!(null),
    );
    check(json!(["uuid?", "not-a-uuid"]), json!(false), json!(null));
}

#[test]
fn test_ip4_validator() {
    check(json!(["ip4?", "192.168.1.1"]), json!(true), json!(null));
    check(
        json!(["ip4?", "999.999.999.999"]),
        json!(false),
        json!(null),
    );
}

#[test]
fn test_date_validator() {
    check(json!(["date?", "2023-01-15"]), json!(true), json!(null));
    check(json!(["date?", "2023-13-01"]), json!(false), json!(null));
    check(json!(["date?", "not-a-date"]), json!(false), json!(null));
}

// ----------------------------------------------------------------- Bitwise

#[test]
fn test_bitwise() {
    check(json!(["&", 0b1010, 0b1100]), json!(0b1000_i64), json!(null));
    check(json!(["|", 0b1010, 0b1100]), json!(0b1110_i64), json!(null));
    check(json!(["^", 0b1010, 0b1100]), json!(0b0110_i64), json!(null));
    check(json!(["~", 0]), json!(-1_i64), json!(null));
}

// ----------------------------------------------------------------- Array

#[test]
fn test_concat() {
    // Array literals must be wrapped as [[arr]] (single-element literal syntax)
    check(
        json!(["concat", [[1, 2]], [[3, 4]]]),
        json!([1, 2, 3, 4]),
        json!(null),
    );
    check(
        json!(["++", [[1]], [[2]], [[3]]]),
        json!([1, 2, 3]),
        json!(null),
    );
}

#[test]
fn test_push() {
    check(json!(["push", [[1, 2]], 3]), json!([1, 2, 3]), json!(null));
}

#[test]
fn test_head() {
    check(
        json!(["head", [[1, 2, 3, 4, 5]], 3]),
        json!([1, 2, 3]),
        json!(null),
    );
    check(
        json!(["head", [[1, 2, 3, 4, 5]], -2]),
        json!([4, 5]),
        json!(null),
    );
}

#[test]
fn test_sort() {
    // JS default sort is lexicographic string comparison — numbers stay as numbers
    check(json!(["sort", [[3, 1, 2]]]), json!([1, 2, 3]), json!(null));
}

#[test]
fn test_reverse() {
    check(
        json!(["reverse", [[1, 2, 3]]]),
        json!([3, 2, 1]),
        json!(null),
    );
}

#[test]
fn test_in() {
    check(json!(["in", [[1, 2, 3]], 2]), json!(true), json!(null));
    check(json!(["in", [[1, 2, 3]], 5]), json!(false), json!(null));
}

#[test]
fn test_slice() {
    check(
        json!(["slice", [[1, 2, 3, 4, 5]], 1, 3]),
        json!([2, 3]),
        json!(null),
    );
}

#[test]
fn test_zip() {
    check(
        json!(["zip", [[1, 2]], [["a", "b"]]]),
        json!([[1, "a"], [2, "b"]]),
        json!(null),
    );
}

#[test]
fn test_index_of() {
    check(
        json!(["indexOf", [[1, 2, 3]], 2]),
        json!(1_i64),
        json!(null),
    );
    check(
        json!(["indexOf", [[1, 2, 3]], 9]),
        json!(-1_i64),
        json!(null),
    );
}

#[test]
fn test_from_entries() {
    check(
        json!(["fromEntries", [[["a", 1], ["b", 2]]]]),
        json!({"a": 1, "b": 2}),
        json!(null),
    );
}

#[test]
fn test_filter() {
    let data = json!(null);
    check(
        json!(["filter", [[1, 2, 3, 4, 5]], ["x"], [">", ["$", "x"], 3]]),
        json!([4, 5]),
        data,
    );
}

#[test]
fn test_map() {
    check(
        json!(["map", [[1, 2, 3]], ["x"], ["+", ["$", "x"], 10]]),
        json!([11.0, 12.0, 13.0]),
        json!(null),
    );
}

#[test]
fn test_reduce() {
    check(
        json!([
            "reduce",
            [[1, 2, 3, 4]],
            0,
            ["acc"],
            ["x"],
            ["+", ["$", "acc"], ["$", "x"]]
        ]),
        json!(10.0),
        json!(null),
    );
}

// ----------------------------------------------------------------- Object

#[test]
fn test_keys() {
    let result_val = {
        let ops = Arc::new(operators_map());
        let mut vars = Vars::new(json!(null));
        let mut ctx = EvalCtx::new(&mut vars, ops);
        let result = evaluate(&json!(["keys", {"b": 2, "a": 1}]), &mut ctx).unwrap();
        match result {
            JsValue::Json(v) => v,
            _ => panic!("not json"),
        }
    };
    // Keys order may vary, just check it has both keys
    let arr = result_val.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr.contains(&json!("a")));
    assert!(arr.contains(&json!("b")));
}

#[test]
fn test_entries() {
    // entries returns [[key, value], ...] — order not guaranteed for objects
    let ops = Arc::new(operators_map());
    let mut vars = Vars::new(json!(null));
    let mut ctx = EvalCtx::new(&mut vars, ops);
    let result = evaluate(&json!(["entries", {"a": 1}]), &mut ctx).unwrap();
    match result {
        JsValue::Json(Value::Array(arr)) => {
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0], json!(["a", 1]));
        }
        _ => panic!("expected array"),
    }
}

#[test]
fn test_o_set() {
    check(
        json!(["o.set", {}, "a", 1, "b", 2]),
        json!({"a": 1, "b": 2}),
        json!(null),
    );
}

#[test]
fn test_o_del() {
    check(
        json!(["o.del", {"a": 1, "b": 2}, "a"]),
        json!({"b": 2}),
        json!(null),
    );
}

// ----------------------------------------------------------------- Literals

#[test]
fn test_literal_value() {
    // Non-array values are literals
    check(json!(42), json!(42), json!(null));
    check(json!("hello"), json!("hello"), json!(null));
    check(json!(true), json!(true), json!(null));
    check(json!(null), json!(null), json!(null));
}

#[test]
fn test_single_element_array_is_literal() {
    // [x] is a literal wrapper — returns x
    check(json!([[1, 2, 3]]), json!([1, 2, 3]), json!(null));
    check(json!(["hello"]), json!("hello"), json!(null));
}

// ----------------------------------------------------------------- Regression: short-circuit &&/||

#[test]
fn test_and_short_circuit() {
    // The right-hand side must NOT be evaluated when left is already falsy.
    // Before the fix, ["throw", "boom"] would execute and produce an error.
    check(
        json!(["&&", false, ["throw", "boom"]]),
        json!(false),
        json!(null),
    );
    check(
        json!(["&&", 0, ["throw", "boom"]]),
        json!(0_i64),
        json!(null),
    );
    // Three operands: short-circuit fires at the second position
    check(
        json!(["&&", 1, false, ["throw", "boom"]]),
        json!(false),
        json!(null),
    );
    // Returns the final evaluated value when all are truthy
    check(json!(["&&", 1, 2, 3]), json!(3_i64), json!(null));
}

#[test]
fn test_or_short_circuit() {
    // The right-hand side must NOT be evaluated when left is already truthy.
    check(
        json!(["||", true, ["throw", "boom"]]),
        json!(true),
        json!(null),
    );
    check(
        json!(["||", 1, ["throw", "boom"]]),
        json!(1_i64),
        json!(null),
    );
    // Three operands: short-circuit fires at the second position
    check(
        json!(["||", false, true, ["throw", "boom"]]),
        json!(true),
        json!(null),
    );
    // Returns the final evaluated value when all are falsy
    check(json!(["||", false, 0, null]), json!(null), json!(null));
}

// ----------------------------------------------------------------- Regression: NaN/Infinity → null

#[test]
fn test_non_finite_arithmetic_returns_null() {
    // sqrt(-1) is NaN in JS; JSON.stringify(NaN) = "null"
    // Before the fix, f64_to_jsval(NaN) would panic via serde_json::json!(NaN)
    check(json!(["sqrt", -1]), json!(null), json!(null));
    // ln(0) is -Infinity in JS; JSON.stringify(-Infinity) = "null"
    check(json!(["ln", 0]), json!(null), json!(null));
    // exp(1000) overflows to +Infinity
    check(json!(["exp", 1000]), json!(null), json!(null));
}

// ----------------------------------------------------------------- Regression: IPv6 regex corruption

#[test]
fn test_ip6_validator_with_embedded_ipv4() {
    // IPv6 with embedded IPv4 — requires the `2[0-4]` octet class to be correct.
    // Before the fix, the regex contained `2[0-4}` (14 occurrences), corrupting the
    // character class and causing mis-matches on some embedded-IPv4 addresses.
    check(
        json!(["ip6?", "::ffff:192.168.1.1"]),
        json!(true),
        json!(null),
    );
    check(
        json!(["ip6?", "::ffff:192.168.245.100"]),
        json!(true),
        json!(null),
    );
    check(json!(["ip6?", "::ffff:10.0.0.1"]), json!(true), json!(null));
    // Pure IPv6 still works
    check(json!(["ip6?", "2001:db8::1"]), json!(true), json!(null));
    check(json!(["ip6?", "::1"]), json!(true), json!(null));
    check(json!(["ip6?", "not-an-ipv6"]), json!(false), json!(null));
}

// ----------------------------------------------------------------- Regression: slice negative indices

#[test]
fn test_slice_negative_indices() {
    // JS: [1,2,3,4,5].slice(-2, 5) → [4, 5]
    // Before the fix, -2 was clamped to 0 → [1,2,3,4,5] (wrong)
    check(
        json!(["slice", [[1, 2, 3, 4, 5]], -2, 5]),
        json!([4, 5]),
        json!(null),
    );
    // JS: [1,2,3,4,5].slice(1, -1) → [2, 3, 4]
    check(
        json!(["slice", [[1, 2, 3, 4, 5]], 1, -1]),
        json!([2, 3, 4]),
        json!(null),
    );
    // Both negative: .slice(-3, -1) → [3, 4]
    check(
        json!(["slice", [[1, 2, 3, 4, 5]], -3, -1]),
        json!([3, 4]),
        json!(null),
    );
}

// ----------------------------------------------------------------- Regression: substr negative indices

#[test]
fn test_substr_negative_indices() {
    // JS: "hello".slice(-3, -1) = "ll"
    // Before the fix, negative indices were clamped to 0 → "" (wrong)
    check(json!(["substr", "hello", -3, -1]), json!("ll"), json!(null));
    // JS: "hello".slice(-3, 5) = "llo"
    check(json!(["substr", "hello", -3, 5]), json!("llo"), json!(null));
}

// ----------------------------------------------------------------- Regression: member() Unicode string bounds

#[test]
fn test_member_unicode_bounds() {
    // "😀" is 1 Unicode scalar (4 UTF-8 bytes). Before the fix, s.len() returned 4
    // (byte length), so index 1 would pass the bounds check but return "\0" instead
    // of Undefined.
    check(json!(["[]", "😀", 1]), json!(null), json!(null));
    // "😀a" has 2 chars. Both indices should resolve correctly.
    check(json!(["[]", "😀a", 0]), json!("😀"), json!(null));
    check(json!(["[]", "😀a", 1]), json!("a"), json!(null));
    // Index 2 is out of bounds → Undefined → null
    check(json!(["[]", "😀a", 2]), json!(null), json!(null));
}

// ----------------------------------------------------------------- Regression: int() wrapping semantics

#[test]
fn test_int_wrapping_semantics() {
    // JS `~~3000000000` wraps to -1294967296 (ToInt32 wrapping, not saturating).
    // `~(-1294967296)` = 1294967295.
    // Before the fix, saturating cast gave i32::MAX = 2147483647, so ~2147483647 = -2147483648.
    check(
        json!(["~", 3000000000_i64]),
        json!(1294967295_i64),
        json!(null),
    );
}

// ----------------------------------------------------------------- Regression: jp.add new key

#[test]
fn test_jp_add_new_key() {
    // Before the fix, jp.add called find(&doc, full_path) which fails when the key
    // doesn't exist yet — the entire point of the "add" operation.
    check(
        json!(["jp.add", {"a": 1}, "/b", 2]),
        json!({"a": 1, "b": 2}),
        json!(null),
    );
    // Array append via "-"
    check(
        json!(["jp.add", [[1, 2]], "/-", 3]),
        json!([1, 2, 3]),
        json!(null),
    );
    // Add to nested object
    check(
        json!(["jp.add", {"x": {}}, "/x/y", 42]),
        json!({"x": {"y": 42}}),
        json!(null),
    );
}

// ----------------------------------------------------------------- Evaluate combined

#[test]
fn test_nested_expressions() {
    check(
        json!(["&&", [">", ["$", "/a"], 0], ["<", ["$", "/a"], 100]]),
        json!(true),
        json!({"a": 50}),
    );
    check(
        json!(["&&", [">", ["$", "/a"], 0], ["<", ["$", "/a"], 100]]),
        json!(false),
        json!({"a": 150}),
    );
}
