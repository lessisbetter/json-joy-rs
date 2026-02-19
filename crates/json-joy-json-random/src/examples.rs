//! Upstream-inspired random data examples.
//!
//! Rust divergence:
//! - Upstream exports many `const` template values from `examples.ts`.
//! - This Rust module currently exposes constructor functions. It keeps names
//!   close to upstream and returns `Template` values lazily, which is more
//!   ergonomic for recursive/data-heavy templates in Rust.

use serde_json::Value;

use crate::string::Token;
use crate::structured::{ObjectTemplateField, Template, TemplateJson};

pub fn token_email() -> Token {
    Token::list(vec![
        Token::repeat(3, 12, Token::char_range(97, 122, None)),
        Token::pick(vec![
            Token::literal("."),
            Token::literal("_"),
            Token::literal("-"),
            Token::literal(""),
        ]),
        Token::repeat(0, 5, Token::char_range(97, 122, None)),
        Token::literal("@"),
        Token::pick(vec![
            Token::literal("gmail.com"),
            Token::literal("yahoo.com"),
            Token::literal("example.org"),
            Token::literal("test.com"),
            Token::literal("demo.net"),
        ]),
    ])
}

pub fn token_phone() -> Token {
    Token::list(vec![
        Token::literal("+1-"),
        Token::char_range(50, 57, Some(3)),
        Token::literal("-"),
        Token::char_range(48, 57, Some(3)),
        Token::literal("-"),
        Token::char_range(48, 57, Some(4)),
    ])
}

pub fn token_product_code() -> Token {
    Token::list(vec![
        Token::pick(vec![
            Token::literal("PRD"),
            Token::literal("ITM"),
            Token::literal("SKU"),
        ]),
        Token::literal("-"),
        Token::char_range(65, 90, Some(2)),
        Token::char_range(48, 57, Some(6)),
    ])
}

pub fn token_url() -> Token {
    Token::list(vec![
        Token::literal("https://"),
        Token::repeat(3, 15, Token::char_range(97, 122, None)),
        Token::pick(vec![
            Token::literal(".com"),
            Token::literal(".org"),
            Token::literal(".net"),
            Token::literal(".io"),
        ]),
    ])
}

pub fn token_username() -> Token {
    Token::list(vec![
        Token::pick(vec![
            Token::literal("user"),
            Token::literal("admin"),
            Token::literal("guest"),
            Token::literal("test"),
        ]),
        Token::char_range(48, 57, Some(4)),
    ])
}

pub fn user_profile() -> Template {
    Template::obj(vec![
        ObjectTemplateField::required_literal_key("id", Template::int(Some(1), Some(10_000))),
        ObjectTemplateField::required_literal_key(
            "username",
            Template::str(Some(token_username())),
        ),
        ObjectTemplateField::required_literal_key("email", Template::str(Some(token_email()))),
        ObjectTemplateField::required_literal_key("age", Template::int(Some(18), Some(120))),
        ObjectTemplateField::required_literal_key("isActive", Template::bool(None)),
    ])
}

pub fn user_basic() -> Template {
    Template::obj(vec![
        ObjectTemplateField::required_literal_key("id", Template::int(Some(1), Some(1000))),
        ObjectTemplateField::required_literal_key("name", Template::str(None)),
        ObjectTemplateField::required_literal_key("active", Template::bool(None)),
    ])
}

pub fn api_response() -> Template {
    Template::obj(vec![
        ObjectTemplateField::required_literal_key(
            "status",
            Template::str(Some(Token::pick(vec![
                Token::literal("success"),
                Token::literal("error"),
            ]))),
        ),
        ObjectTemplateField::required_literal_key(
            "timestamp",
            Template::int(Some(1_640_000_000), Some(1_700_000_000)),
        ),
    ])
}

macro_rules! placeholder_template_fn {
    ($($name:ident),* $(,)?) => {
        $(
            pub fn $name() -> Template {
                Template::nil()
            }
        )*
    };
}

// Keep upstream symbol family names discoverable while full example catalog
// is being ported.
placeholder_template_fn!(
    api_response_detailed,
    service_config,
    config_map,
    permissions,
    translations,
    tree,
    comment,
    product,
    order,
    user_token,
    user_role,
    log_entry,
    metric_data,
    coordinates,
    address,
    location,
    transaction,
    bank_account,
    social_post,
    social_profile,
    sensor_reading,
    iot_device,
    patient,
    medical_record,
    student,
    course,
    grade,
    empty_structures,
    unicode_text,
    large_numbers,
    performance_test,
    mixed_types,
    load_test_user,
    all_examples,
);

pub fn gen_user() -> Value {
    TemplateJson::gen(Some(user_profile()), None)
}

pub fn gen_user_basic() -> Value {
    TemplateJson::gen(Some(user_basic()), None)
}

pub fn gen_address() -> Value {
    TemplateJson::gen(Some(address()), None)
}

pub fn gen_product() -> Value {
    TemplateJson::gen(Some(product()), None)
}

pub fn gen_order() -> Value {
    TemplateJson::gen(Some(order()), None)
}

pub fn gen_transaction() -> Value {
    TemplateJson::gen(Some(transaction()), None)
}

pub fn gen_bank_account() -> Value {
    TemplateJson::gen(Some(bank_account()), None)
}

pub fn gen_social_post() -> Value {
    TemplateJson::gen(Some(social_post()), None)
}

pub fn gen_social_profile() -> Value {
    TemplateJson::gen(Some(social_profile()), None)
}

pub fn gen_location() -> Value {
    TemplateJson::gen(Some(location()), None)
}

pub fn gen_api_response() -> Value {
    TemplateJson::gen(Some(api_response()), None)
}

pub fn gen_api_response_detailed() -> Value {
    TemplateJson::gen(Some(api_response_detailed()), None)
}

pub fn gen_service_config() -> Value {
    TemplateJson::gen(Some(service_config()), None)
}

pub fn gen_patient() -> Value {
    TemplateJson::gen(Some(patient()), None)
}

pub fn gen_medical_record() -> Value {
    TemplateJson::gen(Some(medical_record()), None)
}

pub fn gen_student() -> Value {
    TemplateJson::gen(Some(student()), None)
}

pub fn gen_course() -> Value {
    TemplateJson::gen(Some(course()), None)
}

pub fn gen_sensor_reading() -> Value {
    TemplateJson::gen(Some(sensor_reading()), None)
}

pub fn gen_iot_device() -> Value {
    TemplateJson::gen(Some(iot_device()), None)
}

pub fn gen_log_entry() -> Value {
    TemplateJson::gen(Some(log_entry()), None)
}

pub fn gen_metric_data() -> Value {
    TemplateJson::gen(Some(metric_data()), None)
}

pub fn gen_random_example() -> Value {
    TemplateJson::gen(Some(all_examples()), None)
}
