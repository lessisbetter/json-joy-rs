//! JSON Patch implementation (RFC 6902 + extensions).
//!
//! Mirrors `packages/json-joy/src/json-patch/`.
//!
//! # Operations
//!
//! All standard RFC 6902 operations are supported:
//! `add`, `remove`, `replace`, `copy`, `move`, `test`.
//!
//! Extensions from json-joy:
//! `str_ins`, `str_del`, `flip`, `inc`, `split`, `merge`, `extend`.
//!
//! Predicate operations (first-order):
//! `defined`, `undefined`, `contains`, `ends`, `starts`, `in`, `less`,
//! `more`, `matches`, `test_type`, `test_string`, `test_string_len`, `type`.
//!
//! Second-order predicate operations:
//! `and`, `not`, `or`.

pub mod types;
pub mod apply;
pub mod codec;

pub use types::{ApplyPatchOptions, JsonPatchType, Op, OpResult, PatchError, PatchResult};
pub use apply::{apply_op, apply_ops, apply_patch};
pub use codec::json::{from_json, from_json_patch, to_json, to_json_patch};
