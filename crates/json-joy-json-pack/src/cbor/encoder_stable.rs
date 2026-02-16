// Stable encoder alias for JSON-focused Rust port.

#[allow(unused_imports)]
pub use super::encoder_fast::{
    encode_json_to_cbor_bytes, write_cbor_signed, write_cbor_text_like_json_pack,
    write_cbor_uint_major, write_cbor_value_like_json_pack, write_json_like_json_pack,
};
