//! SSH 2.0 binary protocol encoding (RFC 4251).
//!
//! Upstream reference: `json-pack/src/ssh/`

mod decoder;
mod encoder;
pub mod error;

pub use decoder::SshDecoder;
pub use encoder::SshEncoder;
pub use error::SshError;
