//! Native model API slice inspired by upstream `json-crdt/model/api/*`.
//!
//! This module intentionally starts with a compact surface that is already
//! useful for runtime orchestration and test-port mapping:
//! - bootstrap from patches (`Model.fromPatches`-like),
//! - batch apply (`Model.applyBatch`-like),
//! - path lookup (`api.find`-like),
//! - basic mutators (`set`, `obj_put`, `arr_push`, `str_ins`).

use crate::diff_runtime::{diff_model_to_patch_bytes, DiffError};
use crate::model::ModelError;
use crate::model_runtime::{ApplyError, RuntimeModel};
use crate::patch::{ConValue, DecodedOp, Patch, Timestamp};
use crate::patch_builder::encode_patch_from_ops;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

mod events;
mod path;

pub use events::{BatchChangeEvent, ChangeEvent, ChangeEventOrigin, ScopedChangeEvent};
pub use path::PathStep;
use path::{get_path_mut, parse_json_pointer, split_parent, value_at_path};

include!("types.rs");
include!("lifecycle.rs");
include!("ops.rs");
include!("handles.rs");
