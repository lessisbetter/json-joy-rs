//! JSON Expression evaluator — port of `@jsonjoy.com/json-expression`.
//!
//! # Overview
//!
//! This crate implements a JSON expression language where expressions are
//! JSON arrays of the form `[operator, ...operands]`.
//!
//! # Example
//!
//! ```
//! use json_expression::{evaluate, EvalCtx, Vars, operators_map};
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let expr = json!(["+", 1, 2]);
//! let mut vars = Vars::new(json!(null));
//! let ops = Arc::new(operators_map());
//! let mut ctx = EvalCtx::new(&mut vars, ops);
//! let result = evaluate(&expr, &mut ctx).unwrap();
//!
//! assert_eq!(result, json_expression::JsValue::Json(json!(3.0)));
//! ```

pub mod codegen;
pub mod codegen_steps;
pub mod error;
pub mod eval_ctx;
pub mod evaluate;
pub mod operators;
pub mod types;
pub mod util;
pub mod vars;

// Re-export the core public API
pub use codegen::{JsonExpressionCodegen, JsonExpressionCodegenOptions, JsonExpressionFn};
pub use error::JsError;
pub use eval_ctx::EvalCtx;
pub use evaluate::evaluate;
pub use operators::operators_map;
pub use types::{Arity, JsValue, OperatorDefinition, OperatorMap};
pub use vars::Vars;
