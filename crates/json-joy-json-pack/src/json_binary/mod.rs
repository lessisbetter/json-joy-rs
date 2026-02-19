//! json-binary: embed binary data in JSON using data URI strings.
//!
//! Upstream: `packages/json-pack/src/json-binary/`

mod codec;
pub mod constants;

pub use codec::{parse, stringify, stringify_binary, unwrap_binary, wrap_binary};
