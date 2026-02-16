//! Core primitives for json-joy-rs.

pub mod codec_indexed_binary;
pub mod codec_sidecar_binary;
pub mod crdt_binary;
pub mod diff_runtime;
pub mod less_db_compat;
pub mod model;
pub mod model_api;
pub mod model_runtime;
pub mod patch;
pub mod patch_builder;
pub mod patch_clock_codec;
pub mod patch_compact_binary_codec;
pub mod patch_compact_codec;
pub mod patch_compaction;
pub mod patch_log;
pub mod patch_verbose_codec;
pub mod schema;
pub mod util_diff;

use rand::Rng;

/// Minimum valid session id for json-joy logical clocks.
pub const MIN_SESSION_ID: u64 = 65_536;

/// Returns `true` when the provided session id is valid.
pub fn is_valid_session_id(sid: u64) -> bool {
    sid >= MIN_SESSION_ID
}

/// Generates a random session id that satisfies json-joy constraints.
pub fn generate_session_id() -> u64 {
    let mut rng = rand::thread_rng();
    rng.gen_range(MIN_SESSION_ID..=i64::MAX as u64)
}

/// Returns the crate version at compile time.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
