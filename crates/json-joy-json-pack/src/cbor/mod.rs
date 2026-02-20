//! CBOR module layout aligned to upstream `json-pack/src/cbor/*` family.
#![allow(dead_code)]

mod codec;
mod constants;
mod convert;
mod decoder;
mod decoder_base;
mod decoder_dag;
mod encoder;
mod encoder_dag;
mod encoder_fast;
mod encoder_stable;
mod error;

pub use codec::CborJsonValueCodec;
pub use convert::{cbor_to_json, cbor_to_json_owned, json_to_cbor};
pub use decoder::{
    decode_cbor_value, decode_cbor_value_with_consumed, decode_json_from_cbor_bytes,
    validate_cbor_exact_size, CborDecoder,
};
pub use decoder_dag::CborDecoderDag;
pub use encoder::{encode_cbor_value, CborEncoder};
pub use encoder_dag::CborEncoderDag;
pub use encoder_fast::{
    encode_json_to_cbor_bytes, write_cbor_signed, write_cbor_text_like_json_pack,
    write_cbor_uint_major, write_json_like_json_pack, CborEncoderFast,
};
pub use encoder_stable::CborEncoderStable;
pub use error::CborError;
