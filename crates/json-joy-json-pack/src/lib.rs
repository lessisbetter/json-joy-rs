//! CBOR-focused `json-pack` primitives for workspace-wide reuse.
//!
//! Upstream reference:
//! - `/Users/nchapman/Code/json-joy/packages/json-pack/src/cbor/*`

pub mod cbor;

pub use cbor::{
    cbor_to_json, cbor_to_json_owned, decode_cbor_value, decode_cbor_value_with_consumed,
    decode_json_from_cbor_bytes, encode_cbor_value, encode_json_to_cbor_bytes, json_to_cbor,
    validate_cbor_exact_size, write_cbor_signed, write_cbor_text_like_json_pack,
    write_cbor_uint_major, write_json_like_json_pack, CborError, CborJsonValueCodec,
};

#[cfg(test)]
mod tests {
    use super::cbor::*;
    use serde_json::json;

    #[test]
    fn json_cbor_roundtrip_matrix() {
        let cases = vec![
            json!(null),
            json!(true),
            json!(123),
            json!("hello"),
            json!([1, 2, 3]),
            json!({"a": 1, "b": [true, null, "x"]}),
        ];
        for case in cases {
            let cbor = json_to_cbor(&case);
            let bin = encode_cbor_value(&cbor).expect("encode cbor");
            let decoded = decode_cbor_value(&bin).expect("decode cbor");
            let back = cbor_to_json(&decoded).expect("cbor to json");
            assert_eq!(back, case);
        }
    }

    #[test]
    fn json_pack_text_header_behavior_uses_max_utf8_size_guess() {
        // Six 3-byte codepoints: actual bytes=18 (fits short header), but
        // json-pack-style encoding uses max-size guess (6*4=24) => 0x78 length.
        let s = "€€€€€€";
        let mut out = Vec::new();
        write_cbor_text_like_json_pack(&mut out, s);
        assert_eq!(out[0], 0x78);
        assert_eq!(out[1], 18);
    }

    #[test]
    fn json_bytes_roundtrip_via_json_pack_encoding() {
        let value = json!({
            "k": ["x", 1, -2, true, null, {"nested": "v"}]
        });
        let bytes = encode_json_to_cbor_bytes(&value).expect("json-pack encode");
        let decoded = decode_json_from_cbor_bytes(&bytes).expect("decode to json");
        assert_eq!(decoded, value);
    }

    #[test]
    fn cbor_json_value_codec_roundtrip() {
        let codec = CborJsonValueCodec;
        let value = json!({"a":[1,2,3],"b":"x"});
        let bytes = codec.encode(&value).expect("encode");
        assert!(validate_cbor_exact_size(&bytes, bytes.len()).is_ok());
        let out = codec.decode(&bytes).expect("decode");
        assert_eq!(out, value);
    }
}
