use serde_json::Value;

use crate::string::Token;

/// Schema (template) for random JSON generation.
///
/// Rust divergence: recursive references are represented as function pointers.
#[derive(Clone)]
pub enum Template {
    Lit(Value),
    Num(Option<f64>, Option<f64>),
    Int(Option<i64>, Option<i64>),
    /// Upstream uses JS `bigint`; Rust representation uses `i64`.
    Int64(Option<i64>, Option<i64>),
    Float(Option<f64>, Option<f64>),
    Str(Option<Token>),
    Bool(Option<bool>),
    Bin {
        min: Option<usize>,
        max: Option<usize>,
        omin: Option<u8>,
        omax: Option<u8>,
    },
    Nil,
    Arr {
        min: Option<usize>,
        max: Option<usize>,
        item: Option<Box<Template>>,
        head: Vec<Template>,
        tail: Vec<Template>,
    },
    Obj(Vec<ObjectTemplateField>),
    Map {
        key: Option<Token>,
        value: Option<Box<Template>>,
        min: Option<usize>,
        max: Option<usize>,
    },
    Or(Vec<Template>),
    Recursive(fn() -> Template),
}

impl Template {
    pub fn lit(value: Value) -> Self {
        Self::Lit(value)
    }

    pub fn num(min: Option<f64>, max: Option<f64>) -> Self {
        Self::Num(min, max)
    }

    pub fn int(min: Option<i64>, max: Option<i64>) -> Self {
        Self::Int(min, max)
    }

    pub fn int64(min: Option<i64>, max: Option<i64>) -> Self {
        Self::Int64(min, max)
    }

    pub fn float(min: Option<f64>, max: Option<f64>) -> Self {
        Self::Float(min, max)
    }

    pub fn str(token: Option<Token>) -> Self {
        Self::Str(token)
    }

    pub fn bool(value: Option<bool>) -> Self {
        Self::Bool(value)
    }

    pub fn bin(min: Option<usize>, max: Option<usize>, omin: Option<u8>, omax: Option<u8>) -> Self {
        Self::Bin {
            min,
            max,
            omin,
            omax,
        }
    }

    pub fn nil() -> Self {
        Self::Nil
    }

    pub fn arr(
        min: Option<usize>,
        max: Option<usize>,
        item: Option<Template>,
        head: Vec<Template>,
        tail: Vec<Template>,
    ) -> Self {
        Self::Arr {
            min,
            max,
            item: item.map(Box::new),
            head,
            tail,
        }
    }

    pub fn obj(fields: Vec<ObjectTemplateField>) -> Self {
        Self::Obj(fields)
    }

    pub fn map(
        key: Option<Token>,
        value: Option<Template>,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Self {
        Self::Map {
            key,
            value: value.map(Box::new),
            min,
            max,
        }
    }

    pub fn or(options: Vec<Template>) -> Self {
        Self::Or(options)
    }

    pub fn recursive(make: fn() -> Template) -> Self {
        Self::Recursive(make)
    }
}

#[derive(Clone)]
pub struct ObjectTemplateField {
    pub key: Option<Token>,
    pub value: Option<Template>,
    /// Omission probability in range [0, 1].
    pub optionality: Option<f64>,
}

impl ObjectTemplateField {
    pub fn new(key: Option<Token>, value: Option<Template>, optionality: Option<f64>) -> Self {
        Self {
            key,
            value,
            optionality,
        }
    }

    pub fn required_literal_key(key: &str, value: Template) -> Self {
        Self::new(Some(Token::literal(key)), Some(value), Some(0.0))
    }

    pub fn optional_literal_key(key: &str, value: Template, optionality: f64) -> Self {
        Self::new(Some(Token::literal(key)), Some(value), Some(optionality))
    }
}
