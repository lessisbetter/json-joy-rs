//! XDR (External Data Representation) encoder/decoder.
//!
//! Upstream reference: `json-pack/src/xdr/`
//! Reference: RFC 4506

pub mod decoder;
pub mod encoder;
pub mod schema_decoder;
pub mod schema_encoder;
pub mod schema_validator;
pub mod types;

pub use decoder::{XdrDecodeError, XdrDecoder};
pub use encoder::XdrEncoder;
pub use schema_decoder::XdrSchemaDecoder;
pub use schema_encoder::{XdrEncodeError, XdrSchemaEncoder};
pub use schema_validator::XdrSchemaValidator;
pub use types::{XdrDiscriminant, XdrSchema, XdrUnionValue, XdrValue};
