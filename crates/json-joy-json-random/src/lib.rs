//! Random JSON generators and template-driven random data helpers.
//!
//! Upstream mapping (json-random/src):
//! - `RandomJson.ts` -> `random_json.rs`
//! - `number.ts` -> `number.rs`
//! - `string.ts` -> `string.rs`
//! - `util.ts` -> `util.rs`
//! - `structured/*` -> `structured/*`
//! - `examples.ts` -> `examples.rs`
//!
//! Rust divergence note:
//! - Modules use snake_case file names to follow Rust idioms.
//! - Some JS-specific runtime behaviors (e.g. monkey-patching `Math.random`,
//!   `bigint`, `Uint8Array`) are represented with Rust-native equivalents and
//!   documented at implementation sites.

pub mod examples;
pub mod number;
pub mod random_json;
pub mod string;
pub mod structured;
pub mod util;

pub use number::{int, int64};
pub use random_json::{NodeOdds, NodeType, RandomJson, RandomJsonOptions, RootNode};
pub use string::{random_string, Token};
pub use structured::{ObjectTemplateField, Template, TemplateJson, TemplateJsonOpts};
pub use util::{clone_json, deterministic, rnd};
