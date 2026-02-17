//! json-joy-util - Utility functions for json-joy
//!
//! This crate provides utility functions ported from the TypeScript `json-joy` util package.

pub mod fuzzer;
pub mod has_own_property;
pub mod is_empty;
pub mod json_clone;
pub mod json_equal;
pub mod json_size;
pub mod lazy_function;
pub mod obj_key_cmp;
pub mod sort;
pub mod strings;
pub mod types;

// Re-exports for convenience
pub use fuzzer::{Fuzzer, Random};
pub use has_own_property::{has_own_property, has_own_property_hashmap, has_own_property_map, has_own_property_value};
pub use is_empty::{is_empty, is_empty_hashmap, is_empty_map, is_empty_value};
pub use json_clone::{clone, clone_binary, clone_value_with_binary, JsonBinary};
pub use json_equal::{deep_equal, deep_equal_binary};
pub use json_size::{json_size, json_size_approx, json_size_fast, max_encoding_capacity, utf8_size};
pub use lazy_function::{lazy, Lazy, LazyFn};
pub use obj_key_cmp::obj_key_cmp;
pub use sort::{insertion_sort, insertion_sort_by, insertion_sort_by_key};
pub use strings::{as_string, escape, is_letter, is_punctuation, is_whitespace, word_wrap, CharPredicate, WrapOptions};
pub use types::{Branded, MaybeArray};
