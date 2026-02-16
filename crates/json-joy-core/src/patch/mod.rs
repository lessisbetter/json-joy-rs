//! JSON CRDT Patch binary handling.
//!
//! Implementation note:
//! - At this milestone we preserve exact wire bytes and decode enough semantic
//!   operation payload to drive fixture-based runtime application tests.
//! - Validation behavior is intentionally aligned with upstream Node decoder
//!   behavior observed via compatibility fixtures (including permissive
//!   handling for many malformed payloads).

use ciborium::value::Value;
use thiserror::Error;

include!("types.rs");
include!("rewrite.rs");
include!("decode.rs");
include!("encode.rs");
