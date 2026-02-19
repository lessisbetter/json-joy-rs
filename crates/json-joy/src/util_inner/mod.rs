//! Inner utility helpers for the json-joy crate.
//!
//! Mirrors `packages/json-joy/src/util/`.
//!
//! TypeScript-specific or browser-specific utilities (Defer, throttle, dom,
//! events, iterator polyfill) are not ported.

pub mod diff;
pub mod str_cnt;

pub use str_cnt::str_cnt;
