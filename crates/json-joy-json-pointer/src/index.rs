//! Mirrors upstream `index.ts`.

pub use crate::find::find;
pub use crate::find_by_pointer::find_by_pointer;
pub use crate::get::{get, get_mut};
pub use crate::types::{Path, PathStep, Reference, ReferenceKey};
pub use crate::util::{
    escape_component, format_json_pointer, is_child, is_integer, is_path_equal, is_root,
    is_valid_index, parent, parse_json_pointer, parse_json_pointer_relaxed, to_path,
    unescape_component,
};
pub use crate::validate::{validate_json_pointer, validate_path, ValidationError};
