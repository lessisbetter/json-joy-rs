//! MessagePack encoder/decoder family.
//!
//! Upstream: `packages/json-pack/src/msgpack/`

pub mod constants;
pub mod decoder;
pub mod decoder_fast;
pub mod encoder;
pub mod encoder_fast;
pub mod encoder_stable;
pub mod error;
pub mod shallow_read;
pub mod to_json;
pub mod types;
pub mod util;

pub use constants::MsgPackMarker;
pub use decoder::{MsgPackDecoder, MsgPackPathSegment};
pub use decoder_fast::MsgPackDecoderFast;
pub use encoder::MsgPackEncoder;
pub use encoder_fast::MsgPackEncoderFast;
pub use encoder_stable::MsgPackEncoderStable;
pub use error::MsgPackError;
pub use shallow_read::{gen_shallow_reader, ShallowReader};
pub use to_json::MsgPackToJsonConverter;
pub use types::{IMessagePackEncoder, MsgPack};
pub use util::{decode, encode, encode_full};
