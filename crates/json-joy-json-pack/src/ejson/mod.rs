//! EJSON v2 (MongoDB Extended JSON) encoding and decoding.
//!
//! Upstream reference: `json-pack/src/ejson/`
//!
//! EJSON is a superset of JSON that preserves BSON type information using
//! `$`-prefixed wrapper objects (e.g. `{"$oid":"..."}`, `{"$numberInt":"..."}`).
//!
//! Two encoding modes are supported:
//! - **Canonical**: all numbers and dates use explicit type wrappers.
//! - **Relaxed** (default): native JSON types are used where lossless.

pub mod decoder;
pub mod encoder;
pub mod error;
pub mod value;

pub use decoder::EjsonDecoder;
pub use encoder::{EjsonEncoder, EjsonEncoderOptions};
pub use error::{EjsonDecodeError, EjsonEncodeError};
pub use value::EjsonValue;
