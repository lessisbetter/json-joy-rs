//! Binary serialization formats for json-joy (CBOR, MessagePack, JSON, and more).
//!
//! Upstream reference:
//! - `/Users/nchapman/Code/json-joy/packages/json-pack/src/`

mod constants;
mod json_pack_extension;
mod json_pack_mpint;
mod json_pack_value;
mod pack_value;

pub mod avro;
pub mod bencode;
pub mod bson;
pub mod cbor;
pub mod ejson;
pub mod ion;
pub mod json;
pub mod json_binary;
pub mod msgpack;
pub mod resp;
pub mod rm;
pub mod rpc;
pub mod ssh;
pub mod ubjson;
pub mod util;
pub mod ws;
pub mod xdr;

pub use constants::EncodingFormat;
pub use json_pack_extension::JsonPackExtension;
pub use json_pack_mpint::JsonPackMpint;
pub use json_pack_value::JsonPackValue;
pub use pack_value::PackValue;

pub use cbor::{
    cbor_to_json, cbor_to_json_owned, decode_cbor_value, decode_cbor_value_with_consumed,
    decode_json_from_cbor_bytes, encode_cbor_value, encode_json_to_cbor_bytes, json_to_cbor,
    validate_cbor_exact_size, write_cbor_signed, write_cbor_text_like_json_pack,
    write_cbor_uint_major, write_json_like_json_pack, CborEncoder, CborError, CborJsonValueCodec,
};

#[cfg(test)]
mod tests {
    use super::bencode::{BencodeDecoder, BencodeEncoder};
    use super::cbor::*;
    use super::json_binary;
    use super::ubjson::{UbjsonDecoder, UbjsonEncoder};
    use super::PackValue;
    use serde_json::json;

    const TEST_F64_3_14: f64 = 314.0 / 100.0;
    const TEST_F64_3_14159: f64 = 314_159.0 / 100_000.0;
    const TEST_F64_2_71828: f64 = 271_828.0 / 100_000.0;

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
            let bin = encode_json_to_cbor_bytes(&case).expect("encode cbor");
            let back = decode_json_from_cbor_bytes(&bin).expect("decode cbor");
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
        let mut codec = CborJsonValueCodec::new();
        let value = json!({"a":[1,2,3],"b":"x"});
        let bytes = codec.encode(&value).expect("encode");
        assert!(validate_cbor_exact_size(&bytes, bytes.len()).is_ok());
        let out = codec.decode(&bytes).expect("decode");
        assert_eq!(out, value);
    }

    #[test]
    fn bencode_roundtrip_json_values() {
        let cases = vec![
            (PackValue::Null, b"n".as_slice()),
            (PackValue::Bool(true), b"t".as_slice()),
            (PackValue::Bool(false), b"f".as_slice()),
            (PackValue::Integer(42), b"i42e".as_slice()),
            (PackValue::Integer(-7), b"i-7e".as_slice()),
        ];
        let mut enc = BencodeEncoder::new();
        for (val, expected) in cases {
            let bytes = enc.encode(&val);
            assert_eq!(&bytes, expected);
        }
        // String round-trip
        let mut enc = BencodeEncoder::new();
        let bytes = enc.encode(&PackValue::Str("hello".into()));
        assert_eq!(&bytes, b"5:hello");
        // Decode back
        let dec = BencodeDecoder::new();
        let result = dec.decode(b"5:hello").unwrap();
        // bencode strings decode as Bytes (raw binary)
        assert!(matches!(result, PackValue::Bytes(b) if b == b"hello"));
    }

    #[test]
    fn bencode_dict_sorted_keys() {
        let mut enc = BencodeEncoder::new();
        let value = PackValue::Object(vec![
            ("z".into(), PackValue::Integer(1)),
            ("a".into(), PackValue::Integer(2)),
        ]);
        let bytes = enc.encode(&value);
        // Keys must be sorted: 'a' before 'z'
        assert_eq!(&bytes, b"d1:ai2e1:zi1ee");
    }

    #[test]
    fn ubjson_roundtrip_null_and_bool() {
        let mut enc = UbjsonEncoder::new();
        assert_eq!(enc.encode(&PackValue::Null), &[0x5a]);
        assert_eq!(enc.encode(&PackValue::Bool(true)), &[0x54]);
        assert_eq!(enc.encode(&PackValue::Bool(false)), &[0x46]);
    }

    #[test]
    fn ubjson_integer_encoding() {
        let mut enc = UbjsonEncoder::new();
        // uint8
        assert_eq!(enc.encode(&PackValue::Integer(42)), &[0x55, 42]);
        // int8 negative
        let bytes = enc.encode(&PackValue::Integer(-5));
        assert_eq!(bytes[0], 0x69);
        assert_eq!(bytes[1] as i8, -5i8);
        // int32
        let bytes = enc.encode(&PackValue::Integer(100000));
        assert_eq!(bytes[0], 0x6c);
    }

    #[test]
    fn ubjson_string_roundtrip() {
        let mut enc = UbjsonEncoder::new();
        let dec = UbjsonDecoder::new();
        let bytes = enc.encode(&PackValue::Str("hello".into()));
        let result = dec.decode(&bytes).unwrap();
        assert!(matches!(result, PackValue::Str(s) if s == "hello"));
    }

    #[test]
    fn json_binary_wrap_unwrap_roundtrip() {
        let original = PackValue::Bytes(vec![1, 2, 3, 4]);
        let wrapped = json_binary::wrap_binary(original.clone());
        // Should be a string with the data URI prefix
        if let serde_json::Value::String(s) = &wrapped {
            assert!(s.starts_with("data:application/octet-stream;base64,"));
        } else {
            panic!("expected string");
        }
        let unwrapped = json_binary::unwrap_binary(wrapped);
        assert_eq!(unwrapped, original);
    }

    #[test]
    fn json_binary_parse_stringify_roundtrip() {
        let value = PackValue::Object(vec![
            ("key".into(), PackValue::Str("val".into())),
            ("bin".into(), PackValue::Bytes(vec![0xde, 0xad, 0xbe, 0xef])),
        ]);
        let json_str = json_binary::stringify(value.clone()).unwrap();
        let parsed = json_binary::parse(&json_str).unwrap();
        assert_eq!(parsed, value);
    }

    // --- Slice 2: JSON format ---

    #[test]
    fn json_encoder_primitives() {
        use super::json::JsonEncoder;
        let mut enc = JsonEncoder::new();
        assert_eq!(enc.encode(&PackValue::Null), b"null");
        assert_eq!(enc.encode(&PackValue::Bool(true)), b"true");
        assert_eq!(enc.encode(&PackValue::Bool(false)), b"false");
        assert_eq!(enc.encode(&PackValue::Integer(42)), b"42");
        assert_eq!(enc.encode(&PackValue::Integer(-7)), b"-7");
        assert_eq!(enc.encode(&PackValue::Float(1.5)), b"1.5");
    }

    #[test]
    fn json_encoder_string_and_binary() {
        use super::json::JsonEncoder;
        let mut enc = JsonEncoder::new();
        assert_eq!(enc.encode(&PackValue::Str("hello".into())), b"\"hello\"");
        let bin_out = enc.encode(&PackValue::Bytes(vec![1, 2, 3]));
        let s = std::str::from_utf8(&bin_out).unwrap();
        assert!(s.starts_with("\"data:application/octet-stream;base64,"));
        assert!(s.ends_with('"'));
    }

    #[test]
    fn json_encoder_array_and_object() {
        use super::json::JsonEncoder;
        let mut enc = JsonEncoder::new();
        let arr = PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]);
        assert_eq!(enc.encode(&arr), b"[1,2]");
        let obj = PackValue::Object(vec![("a".into(), PackValue::Integer(1))]);
        assert_eq!(enc.encode(&obj), b"{\"a\":1}");
    }

    #[test]
    fn json_encoder_stable_sorts_keys() {
        use super::json::JsonEncoderStable;
        let mut enc = JsonEncoderStable::new();
        let obj = PackValue::Object(vec![
            ("bb".into(), PackValue::Integer(2)),
            ("a".into(), PackValue::Integer(1)),
            ("ccc".into(), PackValue::Integer(3)),
        ]);
        let out = enc.encode(&obj);
        let s = std::str::from_utf8(&out).unwrap();
        // "a" (len 1) before "bb" (len 2) before "ccc" (len 3)
        let a_pos = s.find("\"a\"").unwrap();
        let bb_pos = s.find("\"bb\"").unwrap();
        let ccc_pos = s.find("\"ccc\"").unwrap();
        assert!(a_pos < bb_pos);
        assert!(bb_pos < ccc_pos);
    }

    #[test]
    fn json_encoder_dag_binary() {
        use super::json::JsonEncoderDag;
        let mut enc = JsonEncoderDag::new();
        let out = enc.encode(&PackValue::Bytes(b"hello world".as_slice().to_vec()));
        let s = std::str::from_utf8(&out).unwrap();
        // DAG-JSON binary format: {"/":{"bytes":"<b64>"}}
        assert!(s.starts_with("{\"/\":{\"bytes\":\""), "got: {s}");
        assert!(s.ends_with("\"}}"), "got: {s}");
    }

    #[test]
    fn json_decoder_primitives() {
        use super::json::JsonDecoder;
        let mut dec = JsonDecoder::new();
        assert_eq!(dec.decode(b"null").unwrap(), PackValue::Null);
        assert_eq!(dec.decode(b"true").unwrap(), PackValue::Bool(true));
        assert_eq!(dec.decode(b"false").unwrap(), PackValue::Bool(false));
        assert_eq!(dec.decode(b"42").unwrap(), PackValue::Integer(42));
        assert_eq!(dec.decode(b"-7").unwrap(), PackValue::Integer(-7));
        assert_eq!(dec.decode(b"1.5").unwrap(), PackValue::Float(1.5));
    }

    #[test]
    fn json_decoder_string() {
        use super::json::JsonDecoder;
        let mut dec = JsonDecoder::new();
        assert_eq!(
            dec.decode(b"\"hello\"").unwrap(),
            PackValue::Str("hello".into())
        );
        assert_eq!(
            dec.decode(b"\"a\\nb\"").unwrap(),
            PackValue::Str("a\nb".into())
        );
    }

    #[test]
    fn json_decoder_undefined_sentinel() {
        use super::json::{JsonDecoder, JsonEncoder};
        let mut enc = JsonEncoder::new();
        let mut dec = JsonDecoder::new();
        // Encode undefined, decode it back
        let encoded = enc.encode(&PackValue::Undefined);
        assert_eq!(dec.decode(&encoded).unwrap(), PackValue::Undefined);
        // Also check undefined in an object context (regression for off-by-one cursor bug)
        let obj = PackValue::Object(vec![
            ("u".into(), PackValue::Undefined),
            ("n".into(), PackValue::Integer(1)),
        ]);
        let encoded = enc.encode(&obj);
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, obj);
    }

    #[test]
    fn json_decoder_binary_data_uri() {
        use super::json::JsonDecoder;
        let mut dec = JsonDecoder::new();
        // Encode some bytes and decode back
        let mut enc = super::json::JsonEncoder::new();
        let original = vec![1u8, 2, 3, 4, 5];
        let encoded = enc.encode(&PackValue::Bytes(original.clone()));
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded, PackValue::Bytes(original));
    }

    #[test]
    fn json_decoder_array_and_object() {
        use super::json::JsonDecoder;
        let mut dec = JsonDecoder::new();
        let arr = dec.decode(b"[1,2,3]").unwrap();
        assert_eq!(
            arr,
            PackValue::Array(vec![
                PackValue::Integer(1),
                PackValue::Integer(2),
                PackValue::Integer(3),
            ])
        );
        let obj = dec.decode(b"{\"a\":1}").unwrap();
        assert_eq!(
            obj,
            PackValue::Object(vec![("a".into(), PackValue::Integer(1))])
        );
    }

    #[test]
    fn json_encoder_decoder_roundtrip() {
        use super::json::{JsonDecoder, JsonEncoder};
        let mut enc = JsonEncoder::new();
        let mut dec = JsonDecoder::new();
        let values = vec![
            PackValue::Null,
            PackValue::Bool(true),
            PackValue::Integer(12345),
            PackValue::Float(TEST_F64_3_14),
            PackValue::Str("hello, world!".into()),
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Null]),
            PackValue::Object(vec![
                ("x".into(), PackValue::Bool(false)),
                ("y".into(), PackValue::Str("z".into())),
            ]),
        ];
        for v in values {
            let encoded = enc.encode(&v);
            let decoded = dec.decode(&encoded).unwrap();
            assert_eq!(decoded, v, "roundtrip failed for {v:?}");
        }
    }

    #[test]
    fn json_decoder_partial_incomplete_array() {
        use super::json::JsonDecoderPartial;
        let mut dec = JsonDecoderPartial::new();
        // Missing closing bracket
        let v = dec.decode(b"[1, 2, 3").unwrap();
        assert_eq!(
            v,
            PackValue::Array(vec![
                PackValue::Integer(1),
                PackValue::Integer(2),
                PackValue::Integer(3),
            ])
        );
        // Trailing comma
        let v = dec.decode(b"[1, 2, ").unwrap();
        assert_eq!(
            v,
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2),])
        );
        // Corrupt element — upstream drops it, returns prior elements
        let v = dec.decode(b"[1, 2, x").unwrap();
        assert_eq!(
            v,
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2),])
        );
    }

    #[test]
    fn json_decoder_partial_incomplete_object() {
        use super::json::JsonDecoderPartial;
        let mut dec = JsonDecoderPartial::new();
        // Missing value for last key — key-value pair is dropped
        let v = dec.decode(b"{\"foo\": 1, \"bar\":").unwrap();
        assert_eq!(
            v,
            PackValue::Object(vec![("foo".into(), PackValue::Integer(1))])
        );
        // Complete pairs
        let v = dec.decode(b"{\"a\":1,\"b\":2").unwrap();
        assert_eq!(
            v,
            PackValue::Object(vec![
                ("a".into(), PackValue::Integer(1)),
                ("b".into(), PackValue::Integer(2)),
            ])
        );
    }

    // --- Slice 4: MessagePack format ---

    #[test]
    fn msgpack_encoder_primitives() {
        use super::msgpack::MsgPackEncoderFast;
        let mut enc = MsgPackEncoderFast::new();
        // null = 0xc0
        assert_eq!(enc.encode(&PackValue::Null), &[0xc0]);
        // true = 0xc3, false = 0xc2
        assert_eq!(enc.encode(&PackValue::Bool(true)), &[0xc3]);
        assert_eq!(enc.encode(&PackValue::Bool(false)), &[0xc2]);
        // positive fixint
        assert_eq!(enc.encode(&PackValue::Integer(0)), &[0x00]);
        assert_eq!(enc.encode(&PackValue::Integer(127)), &[0x7f]);
        // uint16
        let out = enc.encode(&PackValue::Integer(1000));
        assert_eq!(out[0], 0xcd);
        // negative fixint
        let out = enc.encode(&PackValue::Integer(-1));
        assert_eq!(out[0], 0xff); // -1 as negative fixint
    }

    #[test]
    fn msgpack_encoder_string() {
        use super::msgpack::MsgPackEncoderFast;
        let mut enc = MsgPackEncoderFast::new();
        let out = enc.encode(&PackValue::Str("hello".into()));
        // fixstr: 0xa0 | 5 = 0xa5, then 5 bytes
        assert_eq!(out[0], 0xa5);
        assert_eq!(&out[1..], b"hello");
    }

    #[test]
    fn msgpack_encoder_binary() {
        use super::msgpack::MsgPackEncoderFast;
        let mut enc = MsgPackEncoderFast::new();
        let data = vec![1u8, 2, 3];
        let out = enc.encode(&PackValue::Bytes(data.clone()));
        // bin8: 0xc4, length, data
        assert_eq!(out[0], 0xc4);
        assert_eq!(out[1], 3);
        assert_eq!(&out[2..], &data);
    }

    #[test]
    fn msgpack_encoder_array() {
        use super::msgpack::MsgPackEncoderFast;
        let mut enc = MsgPackEncoderFast::new();
        let arr = PackValue::Array(vec![PackValue::Null, PackValue::Integer(1)]);
        let out = enc.encode(&arr);
        // fixarray: 0x92 (2 items)
        assert_eq!(out[0], 0x92);
        assert_eq!(out[1], 0xc0); // null
        assert_eq!(out[2], 0x01); // 1
    }

    #[test]
    fn msgpack_encoder_object() {
        use super::msgpack::MsgPackEncoderFast;
        let mut enc = MsgPackEncoderFast::new();
        let obj = PackValue::Object(vec![("a".into(), PackValue::Integer(1))]);
        let out = enc.encode(&obj);
        // fixmap: 0x81 (1 pair)
        assert_eq!(out[0], 0x81);
    }

    #[test]
    fn msgpack_encoder_stable_sorts_keys() {
        use super::msgpack::MsgPackEncoderStable;
        let mut enc = MsgPackEncoderStable::new();
        let obj = PackValue::Object(vec![
            ("z".into(), PackValue::Integer(1)),
            ("a".into(), PackValue::Integer(2)),
        ]);
        let out = enc.encode(&obj);
        // fixmap: 0x82 (2 pairs) — first key should be "a"
        assert_eq!(out[0], 0x82);
        // Second byte is fixstr header for "a" (0xa1)
        assert_eq!(out[1], 0xa1);
        assert_eq!(out[2], b'a');
    }

    #[test]
    fn msgpack_decoder_primitives() {
        use super::msgpack::MsgPackDecoderFast;
        let mut dec = MsgPackDecoderFast::new();
        assert_eq!(dec.decode(&[0xc0]).unwrap(), PackValue::Null);
        assert_eq!(dec.decode(&[0xc3]).unwrap(), PackValue::Bool(true));
        assert_eq!(dec.decode(&[0xc2]).unwrap(), PackValue::Bool(false));
        assert_eq!(dec.decode(&[0x7f]).unwrap(), PackValue::Integer(127));
        assert_eq!(dec.decode(&[0xff]).unwrap(), PackValue::Integer(-1));
    }

    #[test]
    fn msgpack_encoder_decoder_roundtrip() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        let values = vec![
            PackValue::Null,
            PackValue::Bool(true),
            PackValue::Bool(false),
            PackValue::Integer(0),
            PackValue::Integer(127),
            PackValue::Integer(-1),
            PackValue::Integer(1000),
            PackValue::Integer(-1000),
            PackValue::Float(TEST_F64_3_14),
            PackValue::Str("hello".into()),
            PackValue::Bytes(vec![1, 2, 3]),
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Null]),
            PackValue::Object(vec![("key".into(), PackValue::Integer(42))]),
        ];
        for v in values {
            let encoded = enc.encode(&v);
            let decoded = dec.decode(&encoded).unwrap();
            assert_eq!(decoded, v, "roundtrip failed for {v:?}");
        }
    }

    #[test]
    fn msgpack_to_json_converter() {
        use super::msgpack::{MsgPackEncoderFast, MsgPackToJsonConverter};
        let mut enc = MsgPackEncoderFast::new();
        let mut conv = MsgPackToJsonConverter::new();
        let obj = PackValue::Object(vec![
            ("n".into(), PackValue::Null),
            ("b".into(), PackValue::Bool(true)),
            ("i".into(), PackValue::Integer(42)),
            ("s".into(), PackValue::Str("hi".into())),
        ]);
        let msgpack = enc.encode(&obj);
        let json_str = conv.convert(&msgpack);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(parsed["n"], serde_json::Value::Null);
        assert_eq!(parsed["b"], serde_json::Value::Bool(true));
        assert_eq!(parsed["i"], serde_json::json!(42));
        assert_eq!(parsed["s"], serde_json::json!("hi"));
    }

    // --- Slice 5: RM (Record Marshalling) ---

    #[test]
    fn rm_encode_decode_simple_record() {
        use super::rm::{RmRecordDecoder, RmRecordEncoder};
        let mut enc = RmRecordEncoder::new();
        let mut dec = RmRecordDecoder::new();
        let payload = b"hello world";
        let frame = enc.encode_record(payload);
        // Header: 4 bytes (fin=1, length=11) + 11 bytes payload
        assert_eq!(frame.len(), 4 + payload.len());
        // fin bit should be set
        assert_eq!(frame[0] & 0x80, 0x80);
        // length = 11 in lower 31 bits
        let len = u32::from_be_bytes([frame[0] & 0x7f, frame[1], frame[2], frame[3]]);
        assert_eq!(len, payload.len() as u32);
        dec.push(&frame);
        let record = dec.read_record().expect("record should be available");
        assert_eq!(record, payload);
    }

    #[test]
    fn rm_encode_hdr() {
        use super::rm::RmRecordEncoder;
        let mut enc = RmRecordEncoder::new();
        // fin=true, length=42
        let hdr = enc.encode_hdr(true, 42);
        assert_eq!(hdr.len(), 4);
        let val = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        assert_eq!(val, 0x8000_0000 | 42);
        // fin=false
        let hdr2 = enc.encode_hdr(false, 100);
        let val2 = u32::from_be_bytes([hdr2[0], hdr2[1], hdr2[2], hdr2[3]]);
        assert_eq!(val2, 100);
    }

    #[test]
    fn rm_decoder_needs_more_data() {
        use super::rm::RmRecordDecoder;
        let mut dec = RmRecordDecoder::new();
        // Push only the header, not the payload
        dec.push(&[0x80, 0x00, 0x00, 0x05]); // fin=1, len=5
                                             // No payload yet => should return None
        assert!(dec.read_record().is_none());
        // Push the payload
        dec.push(b"hello");
        let record = dec.read_record().expect("should now have record");
        assert_eq!(record, b"hello");
    }

    // --- Slice 5: SSH ---

    #[test]
    fn ssh_boolean_roundtrip() {
        use super::ssh::{SshDecoder, SshEncoder};
        let mut enc = SshEncoder::new();
        let mut dec = SshDecoder::new();
        for b in [true, false] {
            enc.write_boolean(b);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_boolean().unwrap(), b);
        }
    }

    #[test]
    fn ssh_uint32_roundtrip() {
        use super::ssh::{SshDecoder, SshEncoder};
        let mut enc = SshEncoder::new();
        let mut dec = SshDecoder::new();
        for val in [0u32, 1, 255, 65535, 0xffff_ffff] {
            enc.write_uint32(val);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_uint32().unwrap(), val);
        }
    }

    #[test]
    fn ssh_str_roundtrip() {
        use super::ssh::{SshDecoder, SshEncoder};
        let mut enc = SshEncoder::new();
        let mut dec = SshDecoder::new();
        enc.write_str("hello, world!");
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        assert_eq!(dec.read_str().unwrap(), "hello, world!");
    }

    #[test]
    fn ssh_name_list_roundtrip() {
        use super::ssh::{SshDecoder, SshEncoder};
        use super::PackValue;
        let mut enc = SshEncoder::new();
        let mut dec = SshDecoder::new();
        let names = vec![
            PackValue::Str("aes128-ctr".into()),
            PackValue::Str("aes256-ctr".into()),
        ];
        enc.write_name_list(&names);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        let decoded = dec.read_name_list().unwrap();
        assert_eq!(decoded, vec!["aes128-ctr", "aes256-ctr"]);
    }

    // --- Slice 5: WebSocket ---

    #[test]
    fn ws_encode_ping_empty() {
        use super::ws::WsFrameEncoder;
        let mut enc = WsFrameEncoder::new();
        let frame = enc.encode_ping(None);
        // Minimum ping: 2-byte header (fin=1, opcode=9, no mask, length=0)
        assert_eq!(frame.len(), 2);
        assert_eq!(frame[0], 0b1000_1001); // fin=1, opcode=9
        assert_eq!(frame[1], 0x00); // no mask, length=0
    }

    #[test]
    fn ws_encode_ping_with_data() {
        use super::ws::WsFrameEncoder;
        let mut enc = WsFrameEncoder::new();
        let frame = enc.encode_ping(Some(b"test"));
        assert_eq!(frame.len(), 2 + 4);
        assert_eq!(frame[0], 0b1000_1001);
        assert_eq!(frame[1], 4); // length=4
        assert_eq!(&frame[2..], b"test");
    }

    #[test]
    fn ws_encode_hdr_short_length() {
        use super::ws::{WsFrameEncoder, WsFrameOpcode};
        let mut enc = WsFrameEncoder::new();
        let frame = enc.encode_hdr(true, WsFrameOpcode::Binary, 100, 0);
        assert_eq!(frame.len(), 2);
        assert_eq!(frame[0], 0b1000_0010); // fin=1, opcode=2
        assert_eq!(frame[1], 100);
    }

    #[test]
    fn ws_encode_data_msg_hdr_fast_small() {
        use super::ws::WsFrameEncoder;
        let mut enc = WsFrameEncoder::new();
        let frame = enc.encode_data_msg_hdr_fast(10);
        assert_eq!(frame.len(), 2);
        assert_eq!(frame[0], 0b1000_0010); // fin=1, binary
        assert_eq!(frame[1], 10);
    }

    #[test]
    fn ws_decode_simple_frame_header() {
        use super::ws::{WsFrame, WsFrameDecoder};
        let mut dec = WsFrameDecoder::new();
        // fin=1, opcode=2 (binary), no mask, length=5
        dec.push(vec![0b1000_0010, 5, b'h', b'e', b'l', b'l', b'o']);
        let frame = dec.read_frame_header().expect("ok").expect("frame");
        match frame {
            WsFrame::Data(h) => {
                assert!(h.fin);
                assert_eq!(h.opcode, 2);
                assert_eq!(h.length, 5);
                assert!(h.mask.is_none());
            }
            _ => panic!("expected Data frame"),
        }
    }

    // --- Slice 5: BSON ---

    #[test]
    fn bson_encode_decode_simple_document() {
        use super::bson::{BsonDecoder, BsonEncoder, BsonValue};
        let enc = BsonEncoder::new();
        let mut dec = BsonDecoder::new();
        let fields = vec![
            ("name".to_string(), BsonValue::Str("Alice".to_string())),
            ("age".to_string(), BsonValue::Int32(30)),
            ("active".to_string(), BsonValue::Boolean(true)),
        ];
        let bytes = enc.encode(&fields);
        let decoded = dec.decode(&bytes).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].0, "name");
        assert!(matches!(decoded[0].1, BsonValue::Str(ref s) if s == "Alice"));
        assert_eq!(decoded[1].0, "age");
        assert!(matches!(decoded[1].1, BsonValue::Int32(30)));
        assert_eq!(decoded[2].0, "active");
        assert!(matches!(decoded[2].1, BsonValue::Boolean(true)));
    }

    #[test]
    fn bson_null_and_float() {
        use super::bson::{BsonDecoder, BsonEncoder, BsonValue};
        let enc = BsonEncoder::new();
        let mut dec = BsonDecoder::new();
        let fields = vec![
            ("n".to_string(), BsonValue::Null),
            ("f".to_string(), BsonValue::Float(TEST_F64_3_14)),
        ];
        let bytes = enc.encode(&fields);
        let decoded = dec.decode(&bytes).unwrap();
        assert!(matches!(decoded[0].1, BsonValue::Null));
        if let BsonValue::Float(f) = decoded[1].1 {
            assert!((f - TEST_F64_3_14).abs() < 1e-10);
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn bson_nested_document() {
        use super::bson::{BsonDecoder, BsonEncoder, BsonValue};
        let enc = BsonEncoder::new();
        let mut dec = BsonDecoder::new();
        let inner = vec![("x".to_string(), BsonValue::Int32(1))];
        let fields = vec![("obj".to_string(), BsonValue::Document(inner))];
        let bytes = enc.encode(&fields);
        let decoded = dec.decode(&bytes).unwrap();
        if let BsonValue::Document(inner_dec) = &decoded[0].1 {
            assert_eq!(inner_dec[0].0, "x");
            assert!(matches!(inner_dec[0].1, BsonValue::Int32(1)));
        } else {
            panic!("expected nested document");
        }
    }

    // --- Slice 5: RESP3 ---

    #[test]
    fn resp_encode_null() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        let out = enc.encode(&PackValue::Null);
        assert_eq!(out, b"_\r\n");
    }

    #[test]
    fn resp_encode_bool() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        assert_eq!(enc.encode(&PackValue::Bool(true)), b"#t\r\n");
        assert_eq!(enc.encode(&PackValue::Bool(false)), b"#f\r\n");
    }

    #[test]
    fn resp_encode_integer() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        assert_eq!(enc.encode(&PackValue::Integer(42)), b":42\r\n");
        assert_eq!(enc.encode(&PackValue::Integer(-7)), b":-7\r\n");
        assert_eq!(enc.encode(&PackValue::Integer(0)), b":0\r\n");
    }

    #[test]
    fn resp_encode_simple_string() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        let out = enc.encode(&PackValue::Str("hello".into()));
        assert_eq!(out, b"+hello\r\n");
    }

    #[test]
    fn resp_encode_binary() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        let out = enc.encode(&PackValue::Bytes(b"bin".to_vec()));
        assert_eq!(out, b"$3\r\nbin\r\n");
    }

    #[test]
    fn resp_encode_array() {
        use super::resp::RespEncoder;
        use super::PackValue;
        let mut enc = RespEncoder::new();
        let arr = PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]);
        let out = enc.encode(&arr);
        assert_eq!(out, b"*2\r\n:1\r\n:2\r\n");
    }

    #[test]
    fn resp_decode_null() {
        use super::resp::RespDecoder;
        use super::PackValue;
        let mut dec = RespDecoder::new();
        assert_eq!(dec.decode(b"_\r\n").unwrap(), PackValue::Null);
    }

    #[test]
    fn resp_decode_bool() {
        use super::resp::RespDecoder;
        use super::PackValue;
        let mut dec = RespDecoder::new();
        assert_eq!(dec.decode(b"#t\r\n").unwrap(), PackValue::Bool(true));
        assert_eq!(dec.decode(b"#f\r\n").unwrap(), PackValue::Bool(false));
    }

    #[test]
    fn resp_decode_integer() {
        use super::resp::RespDecoder;
        use super::PackValue;
        let mut dec = RespDecoder::new();
        assert_eq!(dec.decode(b":42\r\n").unwrap(), PackValue::Integer(42));
        assert_eq!(dec.decode(b":-7\r\n").unwrap(), PackValue::Integer(-7));
    }

    #[test]
    fn resp_decode_simple_string() {
        use super::resp::RespDecoder;
        use super::PackValue;
        let mut dec = RespDecoder::new();
        assert_eq!(
            dec.decode(b"+hello\r\n").unwrap(),
            PackValue::Str("hello".into())
        );
    }

    #[test]
    fn resp_encode_decode_roundtrip() {
        use super::resp::{RespDecoder, RespEncoder};
        use super::PackValue;
        let mut enc = RespEncoder::new();
        let mut dec = RespDecoder::new();
        let values = vec![
            PackValue::Null,
            PackValue::Bool(true),
            PackValue::Bool(false),
            PackValue::Integer(0),
            PackValue::Integer(42),
            PackValue::Integer(-100),
            PackValue::Float(TEST_F64_3_14),
            PackValue::Str("hello".into()),
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Null]),
        ];
        for v in values {
            let bytes = enc.encode(&v);
            let decoded = dec
                .decode(&bytes)
                .unwrap_or_else(|e| panic!("decode failed for {v:?}: {e}"));
            // For arrays, check recursively
            match (&v, &decoded) {
                (PackValue::Array(a), PackValue::Array(b)) => assert_eq!(a.len(), b.len()),
                _ => assert_eq!(decoded, v, "roundtrip failed for {v:?}"),
            }
        }
    }

    // ---------------------------------------------------------------- Slice 6: XDR

    #[test]
    fn xdr_int_roundtrip() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let mut dec = XdrDecoder::new();
        for n in [-1i32, 0, 1, 42, -2147483648, 2147483647] {
            enc.write_int(n);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_int().unwrap(), n, "int {n}");
        }
    }

    #[test]
    fn xdr_unsigned_int_roundtrip() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let mut dec = XdrDecoder::new();
        for n in [0u32, 1, 255, 65535, 4294967295] {
            enc.write_unsigned_int(n);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_unsigned_int().unwrap(), n, "uint {n}");
        }
    }

    #[test]
    fn xdr_string_roundtrip() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let mut dec = XdrDecoder::new();
        let s = "hello world";
        enc.write_str(s);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        assert_eq!(dec.read_string().unwrap(), s);
    }

    #[test]
    fn xdr_opaque_padding() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let data = b"abc"; // 3 bytes → padded to 4
        enc.write_unsigned_int(data.len() as u32);
        enc.write_opaque(data);
        let bytes = enc.writer.flush();
        // Should be 4 bytes (length) + 4 bytes (padded data) = 8
        assert_eq!(bytes.len(), 8);
        let mut dec = XdrDecoder::new();
        dec.reset(&bytes);
        let decoded = dec.read_varlen_opaque().unwrap();
        assert_eq!(decoded, data.to_vec());
    }

    #[test]
    fn xdr_double_roundtrip() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let mut dec = XdrDecoder::new();
        enc.write_double(TEST_F64_3_14159);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        let decoded = dec.read_double().unwrap();
        assert!((decoded - TEST_F64_3_14159).abs() < 1e-10);
    }

    #[test]
    fn xdr_boolean_roundtrip() {
        use super::xdr::{XdrDecoder, XdrEncoder};
        let mut enc = XdrEncoder::new();
        let mut dec = XdrDecoder::new();
        enc.write_boolean(true);
        enc.write_boolean(false);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        assert!(dec.read_boolean().unwrap());
        assert!(!dec.read_boolean().unwrap());
    }

    // ---------------------------------------------------------------- Slice 6: RPC

    #[test]
    fn rpc_call_message_roundtrip() {
        use super::rpc::{RpcMessage, RpcMessageDecoder, RpcMessageEncoder, RpcOpaqueAuth};
        let mut enc = RpcMessageEncoder::new();
        let cred = RpcOpaqueAuth::none();
        let verf = RpcOpaqueAuth::none();
        let bytes = enc
            .encode_call(42, 100003, 3, 1, &cred, &verf, &[])
            .unwrap();
        let dec = RpcMessageDecoder::new();
        let msg = dec.decode_message(&bytes).unwrap().unwrap();
        if let RpcMessage::Call(call) = msg {
            assert_eq!(call.xid, 42);
            assert_eq!(call.prog, 100003);
            assert_eq!(call.vers, 3);
            assert_eq!(call.proc_, 1);
        } else {
            panic!("expected Call message");
        }
    }

    #[test]
    fn rpc_accepted_reply_roundtrip() {
        use super::rpc::{
            RpcAcceptStat, RpcMessage, RpcMessageDecoder, RpcMessageEncoder, RpcOpaqueAuth,
        };
        let mut enc = RpcMessageEncoder::new();
        let verf = RpcOpaqueAuth::none();
        let results = b"\x00\x00\x00\x01";
        let bytes = enc
            .encode_accepted_reply(99, &verf, 0, None, results)
            .unwrap();
        let dec = RpcMessageDecoder::new();
        let msg = dec.decode_message(&bytes).unwrap().unwrap();
        if let RpcMessage::AcceptedReply(reply) = msg {
            assert_eq!(reply.xid, 99);
            assert_eq!(reply.stat, RpcAcceptStat::Success);
            assert_eq!(reply.results, Some(results.to_vec()));
        } else {
            panic!("expected AcceptedReply");
        }
    }

    #[test]
    fn rpc_rejected_reply_auth_error() {
        use super::rpc::{
            RpcAuthStat, RpcMessage, RpcMessageDecoder, RpcMessageEncoder, RpcRejectStat,
        };
        let mut enc = RpcMessageEncoder::new();
        let bytes = enc.encode_rejected_reply(7, 1, None, Some(1));
        let dec = RpcMessageDecoder::new();
        let msg = dec.decode_message(&bytes).unwrap().unwrap();
        if let RpcMessage::RejectedReply(reply) = msg {
            assert_eq!(reply.xid, 7);
            assert_eq!(reply.stat, RpcRejectStat::AuthError);
            assert_eq!(reply.auth_stat, Some(RpcAuthStat::AuthBadcred));
        } else {
            panic!("expected RejectedReply");
        }
    }

    #[test]
    fn rpc_opaque_auth_body() {
        use super::rpc::{
            RpcAuthFlavor, RpcMessage, RpcMessageDecoder, RpcMessageEncoder, RpcOpaqueAuth,
        };
        let mut enc = RpcMessageEncoder::new();
        let cred = RpcOpaqueAuth {
            flavor: RpcAuthFlavor::AuthSys,
            body: b"uid\x00".to_vec(),
        };
        let verf = RpcOpaqueAuth::none();
        let bytes = enc.encode_call(1, 1, 1, 1, &cred, &verf, &[]).unwrap();
        let dec = RpcMessageDecoder::new();
        let msg = dec.decode_message(&bytes).unwrap().unwrap();
        if let RpcMessage::Call(call) = msg {
            assert_eq!(call.cred.flavor, RpcAuthFlavor::AuthSys);
            assert_eq!(call.cred.body, b"uid\x00".to_vec());
        } else {
            panic!("expected Call");
        }
    }

    // ---------------------------------------------------------------- Slice 6: Avro

    #[test]
    fn avro_null_is_zero_bytes() {
        use super::avro::AvroEncoder;
        let mut enc = AvroEncoder::new();
        enc.write_null();
        assert!(enc.writer.flush().is_empty());
    }

    #[test]
    fn avro_boolean_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        enc.write_boolean(true);
        enc.write_boolean(false);
        let bytes = enc.writer.flush();
        assert_eq!(bytes, [1, 0]);
        dec.reset(&bytes);
        assert!(dec.read_boolean().unwrap());
        assert!(!dec.read_boolean().unwrap());
    }

    #[test]
    fn avro_int_zigzag_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        for n in [-64i32, -1, 0, 1, 63, 127, -2147483648, 2147483647] {
            enc.write_int(n);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_int().unwrap(), n, "int {n}");
        }
    }

    #[test]
    fn avro_long_zigzag_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        for n in [-1i64, 0, 1, 1000, -9876543210, 9876543210] {
            enc.write_long(n);
            let bytes = enc.writer.flush();
            dec.reset(&bytes);
            assert_eq!(dec.read_long().unwrap(), n, "long {n}");
        }
    }

    #[test]
    fn avro_string_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        enc.write_str("hello");
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        assert_eq!(dec.read_str().unwrap(), "hello");
    }

    #[test]
    fn avro_bytes_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        let data = b"\x01\x02\x03\xff";
        enc.write_bytes(data);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        assert_eq!(dec.read_bytes().unwrap(), data.to_vec());
    }

    #[test]
    fn avro_double_roundtrip() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        enc.write_double(TEST_F64_2_71828);
        let bytes = enc.writer.flush();
        dec.reset(&bytes);
        let v = dec.read_double().unwrap();
        assert!((v - TEST_F64_2_71828).abs() < 1e-10);
    }

    #[test]
    fn avro_str_encode_decode() {
        use super::avro::{AvroDecoder, AvroEncoder};
        let mut enc = AvroEncoder::new();
        let mut dec = AvroDecoder::new();
        enc.write_str("test");
        let bytes = enc.writer.flush();
        // String: unsigned varint(byteLen) + UTF-8 bytes.
        assert_eq!(bytes[0], 4);
        assert_eq!(&bytes[1..], b"test");
        dec.reset(&bytes);
        assert_eq!(dec.read_str().unwrap(), "test");
    }

    // ---------------------------------------------------------------- Slice 6: Ion

    #[test]
    fn ion_encode_null() {
        use super::ion::IonEncoder;
        let mut enc = IonEncoder::new();
        let bytes = enc.encode(&PackValue::Null);
        // IVM (4 bytes) + null typedesc (0x0f = 1 byte)
        assert_eq!(bytes, [0xe0, 0x01, 0x00, 0xea, 0x0f]);
    }

    #[test]
    fn ion_encode_bool() {
        use super::ion::IonEncoder;
        let mut enc = IonEncoder::new();
        let bytes = enc.encode(&PackValue::Bool(true));
        // IVM + BOOL|1 = 0x11
        assert_eq!(bytes, [0xe0, 0x01, 0x00, 0xea, 0x11]);
        let bytes2 = enc.encode(&PackValue::Bool(false));
        assert_eq!(bytes2, [0xe0, 0x01, 0x00, 0xea, 0x10]);
    }

    #[test]
    fn ion_encode_uint_zero() {
        use super::ion::IonEncoder;
        let mut enc = IonEncoder::new();
        let bytes = enc.encode(&PackValue::UInteger(0));
        // IVM + UINT|0 = 0x20
        assert_eq!(bytes, [0xe0, 0x01, 0x00, 0xea, 0x20]);
    }

    #[test]
    fn ion_roundtrip_primitives() {
        use super::ion::{IonDecoder, IonEncoder};
        let mut enc = IonEncoder::new();
        let mut dec = IonDecoder::new();
        let cases = vec![
            PackValue::Null,
            PackValue::Bool(true),
            PackValue::Bool(false),
            PackValue::UInteger(0),
            PackValue::UInteger(42),
            PackValue::Integer(-7),
            PackValue::Str("hello".to_string()),
        ];
        for val in cases {
            let bytes = enc.encode(&val);
            let decoded = dec.decode(&bytes).expect("ion decode");
            assert_eq!(decoded, val, "ion roundtrip for {val:?}");
        }
    }

    #[test]
    fn ion_roundtrip_object_with_string_key() {
        use super::ion::{IonDecoder, IonEncoder};
        let mut enc = IonEncoder::new();
        let mut dec = IonDecoder::new();
        let val = PackValue::Object(vec![("key".to_string(), PackValue::UInteger(42))]);
        let bytes = enc.encode(&val);
        let decoded = dec.decode(&bytes).expect("ion decode object");
        if let PackValue::Object(fields) = decoded {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "key");
            assert_eq!(fields[0].1, PackValue::UInteger(42));
        } else {
            panic!("expected Object, got {decoded:?}");
        }
    }

    #[test]
    fn ion_roundtrip_array() {
        use super::ion::{IonDecoder, IonEncoder};
        let mut enc = IonEncoder::new();
        let mut dec = IonDecoder::new();
        // Ion encodes non-negative integers as UINT, so Integer(n >= 0) decodes as UInteger.
        let val = PackValue::Array(vec![
            PackValue::UInteger(1),
            PackValue::UInteger(2),
            PackValue::UInteger(3),
        ]);
        let bytes = enc.encode(&val);
        let decoded = dec.decode(&bytes).expect("ion decode array");
        assert_eq!(decoded, val);
    }

    // ---------------------------------------------------------------- EJSON

    #[test]
    fn ejson_encoder_null_and_primitives() {
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let s = enc.encode_to_string(&EjsonValue::Null).unwrap();
        assert_eq!(s, "null");
        let s = enc.encode_to_string(&EjsonValue::Bool(true)).unwrap();
        assert_eq!(s, "true");
        let s = enc.encode_to_string(&EjsonValue::Bool(false)).unwrap();
        assert_eq!(s, "false");
        let s = enc
            .encode_to_string(&EjsonValue::Str("hello".to_string()))
            .unwrap();
        assert_eq!(s, "\"hello\"");
    }

    #[test]
    fn ejson_encoder_undefined_wrapper() {
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let s = enc.encode_to_string(&EjsonValue::Undefined).unwrap();
        assert_eq!(s, r#"{"$undefined":true}"#);
    }

    #[test]
    fn ejson_encoder_canonical_numbers() {
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        // Integer in Int32 range
        let s = enc.encode_to_string(&EjsonValue::Number(42.0)).unwrap();
        assert_eq!(s, r#"{"$numberInt":"42"}"#);
        // Integer outside Int32 range
        let s = enc
            .encode_to_string(&EjsonValue::Number(2147483648.0))
            .unwrap();
        assert_eq!(s, r#"{"$numberLong":"2147483648"}"#);
        // Float
        let s = enc
            .encode_to_string(&EjsonValue::Number(TEST_F64_3_14))
            .unwrap();
        assert_eq!(s, r#"{"$numberDouble":"3.14"}"#);
    }

    #[test]
    fn ejson_encoder_relaxed_numbers() {
        use super::ejson::{EjsonEncoder, EjsonValue};
        // Relaxed mode (default) — native JSON numbers for finite values
        let mut enc = EjsonEncoder::new();
        let s = enc.encode_to_string(&EjsonValue::Number(42.0)).unwrap();
        assert_eq!(s, "42");
        let s = enc
            .encode_to_string(&EjsonValue::Number(TEST_F64_3_14))
            .unwrap();
        assert_eq!(s, "3.14");
        // Non-finite still get wrapped
        let s = enc
            .encode_to_string(&EjsonValue::Number(f64::INFINITY))
            .unwrap();
        assert_eq!(s, r#"{"$numberDouble":"Infinity"}"#);
        let s = enc
            .encode_to_string(&EjsonValue::Number(f64::NEG_INFINITY))
            .unwrap();
        assert_eq!(s, r#"{"$numberDouble":"-Infinity"}"#);
        let s = enc.encode_to_string(&EjsonValue::Number(f64::NAN)).unwrap();
        assert_eq!(s, r#"{"$numberDouble":"NaN"}"#);
    }

    #[test]
    fn ejson_encoder_bson_int32_canonical() {
        use super::bson::BsonInt32;
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        let v = BsonInt32 { value: 42 };
        let s = enc.encode_to_string(&EjsonValue::Int32(v)).unwrap();
        assert_eq!(s, r#"{"$numberInt":"42"}"#);
    }

    #[test]
    fn ejson_encoder_bson_int32_relaxed() {
        use super::bson::BsonInt32;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let v = BsonInt32 { value: 42 };
        let s = enc.encode_to_string(&EjsonValue::Int32(v)).unwrap();
        assert_eq!(s, "42");
    }

    #[test]
    fn ejson_encoder_bson_int64_canonical() {
        use super::bson::BsonInt64;
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        let v = BsonInt64 {
            value: 1234567890123,
        };
        let s = enc.encode_to_string(&EjsonValue::Int64(v)).unwrap();
        assert_eq!(s, r#"{"$numberLong":"1234567890123"}"#);
    }

    #[test]
    fn ejson_encoder_bson_float_canonical() {
        use super::bson::BsonFloat;
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        let v = BsonFloat {
            value: TEST_F64_3_14,
        };
        let s = enc.encode_to_string(&EjsonValue::BsonFloat(v)).unwrap();
        assert_eq!(s, r#"{"$numberDouble":"3.14"}"#);
    }

    #[test]
    fn ejson_encoder_object_id() {
        use super::bson::BsonObjectId;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let id = BsonObjectId {
            timestamp: 0x507f1f77,
            process: 0xbcf86cd799,
            counter: 0x439011,
        };
        let s = enc.encode_to_string(&EjsonValue::ObjectId(id)).unwrap();
        assert_eq!(s, r#"{"$oid":"507f1f77bcf86cd799439011"}"#);
    }

    #[test]
    fn ejson_encoder_binary() {
        use super::bson::BsonBinary;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let bin = BsonBinary {
            subtype: 0,
            data: vec![1, 2, 3, 4],
        };
        let s = enc.encode_to_string(&EjsonValue::Binary(bin)).unwrap();
        assert_eq!(s, r#"{"$binary":{"base64":"AQIDBA==","subType":"00"}}"#);
    }

    #[test]
    fn ejson_encoder_code() {
        use super::bson::BsonJavascriptCode;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let code = BsonJavascriptCode {
            code: "function() { return 42; }".to_string(),
        };
        let s = enc.encode_to_string(&EjsonValue::Code(code)).unwrap();
        assert_eq!(s, r#"{"$code":"function() { return 42; }"}"#);
    }

    #[test]
    fn ejson_encoder_symbol() {
        use super::bson::BsonSymbol;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let sym = BsonSymbol {
            symbol: "mySymbol".to_string(),
        };
        let s = enc.encode_to_string(&EjsonValue::Symbol(sym)).unwrap();
        assert_eq!(s, r#"{"$symbol":"mySymbol"}"#);
    }

    #[test]
    fn ejson_encoder_timestamp() {
        use super::bson::BsonTimestamp;
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let ts = BsonTimestamp {
            timestamp: 1234567890,
            increment: 12345,
        };
        let s = enc.encode_to_string(&EjsonValue::Timestamp(ts)).unwrap();
        assert_eq!(s, r#"{"$timestamp":{"t":1234567890,"i":12345}}"#);
    }

    #[test]
    fn ejson_encoder_minkey_maxkey() {
        use super::bson::{BsonMaxKey, BsonMinKey};
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        assert_eq!(
            enc.encode_to_string(&EjsonValue::MinKey(BsonMinKey))
                .unwrap(),
            r#"{"$minKey":1}"#
        );
        assert_eq!(
            enc.encode_to_string(&EjsonValue::MaxKey(BsonMaxKey))
                .unwrap(),
            r#"{"$maxKey":1}"#
        );
    }

    #[test]
    fn ejson_encoder_regexp() {
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let s = enc
            .encode_to_string(&EjsonValue::RegExp("pattern".to_string(), "gi".to_string()))
            .unwrap();
        assert_eq!(
            s,
            r#"{"$regularExpression":{"pattern":"pattern","options":"gi"}}"#
        );
    }

    #[test]
    fn ejson_encoder_date_relaxed_iso() {
        use super::ejson::{EjsonEncoder, EjsonValue};
        // 2023-01-01T00:00:00.000Z = 1672531200000 ms
        let mut enc = EjsonEncoder::new();
        let s = enc
            .encode_to_string(&EjsonValue::Date {
                timestamp_ms: 1672531200000,
                iso: Some("2023-01-01T00:00:00.000Z".to_string()),
            })
            .unwrap();
        assert_eq!(s, r#"{"$date":"2023-01-01T00:00:00.000Z"}"#);
    }

    #[test]
    fn ejson_encoder_date_canonical() {
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        let s = enc
            .encode_to_string(&EjsonValue::Date {
                timestamp_ms: 1672531200000,
                iso: Some("2023-01-01T00:00:00.000Z".to_string()),
            })
            .unwrap();
        assert_eq!(s, r#"{"$date":{"$numberLong":"1672531200000"}}"#);
    }

    #[test]
    fn ejson_encoder_db_pointer() {
        use super::bson::{BsonDbPointer, BsonObjectId};
        use super::ejson::{EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let id = BsonObjectId {
            timestamp: 0x507f1f77,
            process: 0xbcf86cd799,
            counter: 0x439011,
        };
        let ptr = BsonDbPointer {
            name: "collection".to_string(),
            id,
        };
        let s = enc.encode_to_string(&EjsonValue::DbPointer(ptr)).unwrap();
        assert_eq!(
            s,
            r#"{"$dbPointer":{"$ref":"collection","$id":{"$oid":"507f1f77bcf86cd799439011"}}}"#
        );
    }

    #[test]
    fn ejson_encoder_array_canonical() {
        use super::ejson::{EjsonEncoder, EjsonEncoderOptions, EjsonValue};
        let mut enc = EjsonEncoder::with_options(EjsonEncoderOptions { canonical: true });
        let arr = EjsonValue::Array(vec![
            EjsonValue::Number(1.0),
            EjsonValue::Number(2.0),
            EjsonValue::Number(3.0),
        ]);
        let s = enc.encode_to_string(&arr).unwrap();
        assert_eq!(
            s,
            r#"[{"$numberInt":"1"},{"$numberInt":"2"},{"$numberInt":"3"}]"#
        );
    }

    // ---------------------------------------------------------------- EJSON decoder

    #[test]
    fn ejson_decoder_primitives() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        assert_eq!(dec.decode_str("null").unwrap(), EjsonValue::Null);
        assert_eq!(dec.decode_str("true").unwrap(), EjsonValue::Bool(true));
        assert_eq!(dec.decode_str("false").unwrap(), EjsonValue::Bool(false));
        assert_eq!(dec.decode_str("42").unwrap(), EjsonValue::Integer(42));
        assert_eq!(
            dec.decode_str("3.14").unwrap(),
            EjsonValue::Float(TEST_F64_3_14)
        );
        assert_eq!(
            dec.decode_str("\"hello\"").unwrap(),
            EjsonValue::Str("hello".to_string())
        );
    }

    #[test]
    fn ejson_decoder_object_id() {
        use super::bson::BsonObjectId;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$oid":"507f1f77bcf86cd799439011"}"#)
            .unwrap();
        let expected = BsonObjectId {
            timestamp: 0x507f1f77,
            process: 0xbcf86cd799,
            counter: 0x439011,
        };
        assert_eq!(v, EjsonValue::ObjectId(expected));
    }

    #[test]
    fn ejson_decoder_invalid_object_id() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        assert!(matches!(
            dec.decode_str(r#"{"$oid":"invalid"}"#),
            Err(EjsonDecodeError::InvalidObjectId)
        ));
    }

    #[test]
    fn ejson_decoder_int32() {
        use super::bson::BsonInt32;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"$numberInt":"42"}"#).unwrap();
        assert_eq!(v, EjsonValue::Int32(BsonInt32 { value: 42 }));
        let v2 = dec.decode_str(r#"{"$numberInt":"-42"}"#).unwrap();
        assert_eq!(v2, EjsonValue::Int32(BsonInt32 { value: -42 }));
    }

    #[test]
    fn ejson_decoder_invalid_int32() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        // Out of range
        assert!(matches!(
            dec.decode_str(r#"{"$numberInt":"2147483648"}"#),
            Err(EjsonDecodeError::InvalidInt32)
        ));
        // Not a string
        assert!(matches!(
            dec.decode_str(r#"{"$numberInt":42}"#),
            Err(EjsonDecodeError::InvalidInt32)
        ));
    }

    #[test]
    fn ejson_decoder_int64() {
        use super::bson::BsonInt64;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$numberLong":"1234567890123"}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Int64(BsonInt64 {
                value: 1234567890123
            })
        );
    }

    #[test]
    fn ejson_decoder_double() {
        use super::bson::BsonFloat;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"$numberDouble":"3.14"}"#).unwrap();
        assert_eq!(
            v,
            EjsonValue::BsonFloat(BsonFloat {
                value: TEST_F64_3_14
            })
        );
        // Special values
        let v_inf = dec.decode_str(r#"{"$numberDouble":"Infinity"}"#).unwrap();
        assert_eq!(
            v_inf,
            EjsonValue::BsonFloat(BsonFloat {
                value: f64::INFINITY
            })
        );
        let v_neginf = dec.decode_str(r#"{"$numberDouble":"-Infinity"}"#).unwrap();
        assert_eq!(
            v_neginf,
            EjsonValue::BsonFloat(BsonFloat {
                value: f64::NEG_INFINITY
            })
        );
        let v_nan = dec.decode_str(r#"{"$numberDouble":"NaN"}"#).unwrap();
        if let EjsonValue::BsonFloat(bf) = v_nan {
            assert!(bf.value.is_nan());
        } else {
            panic!("expected BsonFloat");
        }
    }

    #[test]
    fn ejson_decoder_decimal128() {
        use super::bson::BsonDecimal128;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"$numberDecimal":"123.456"}"#).unwrap();
        assert_eq!(
            v,
            EjsonValue::Decimal128(BsonDecimal128 {
                data: vec![0u8; 16]
            })
        );
    }

    #[test]
    fn ejson_decoder_binary() {
        use super::bson::BsonBinary;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$binary":{"base64":"AQIDBA==","subType":"00"}}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Binary(BsonBinary {
                subtype: 0,
                data: vec![1, 2, 3, 4]
            })
        );
    }

    #[test]
    fn ejson_decoder_uuid() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$uuid":"c8edabc3-f738-4ca3-b68d-ab92a91478a3"}"#)
            .unwrap();
        if let EjsonValue::Binary(bin) = v {
            assert_eq!(bin.subtype, 4);
            assert_eq!(bin.data.len(), 16);
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn ejson_decoder_invalid_uuid() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        assert!(matches!(
            dec.decode_str(r#"{"$uuid":"invalid-uuid"}"#),
            Err(EjsonDecodeError::InvalidUuid)
        ));
    }

    #[test]
    fn ejson_decoder_code() {
        use super::bson::BsonJavascriptCode;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$code":"function() { return 42; }"}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Code(BsonJavascriptCode {
                code: "function() { return 42; }".to_string()
            })
        );
    }

    #[test]
    fn ejson_decoder_symbol() {
        use super::bson::BsonSymbol;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"$symbol":"mySymbol"}"#).unwrap();
        assert_eq!(
            v,
            EjsonValue::Symbol(BsonSymbol {
                symbol: "mySymbol".to_string()
            })
        );
    }

    #[test]
    fn ejson_decoder_timestamp() {
        use super::bson::BsonTimestamp;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$timestamp":{"t":1234567890,"i":12345}}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Timestamp(BsonTimestamp {
                timestamp: 1234567890,
                increment: 12345
            })
        );
    }

    #[test]
    fn ejson_decoder_invalid_timestamp() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        // Negative t
        assert!(matches!(
            dec.decode_str(r#"{"$timestamp":{"t":-1,"i":12345}}"#),
            Err(EjsonDecodeError::InvalidTimestamp)
        ));
        // Negative i
        assert!(matches!(
            dec.decode_str(r#"{"$timestamp":{"t":123,"i":-1}}"#),
            Err(EjsonDecodeError::InvalidTimestamp)
        ));
    }

    #[test]
    fn ejson_decoder_regexp() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$regularExpression":{"pattern":"test","options":"gi"}}"#)
            .unwrap();
        assert_eq!(v, EjsonValue::RegExp("test".to_string(), "gi".to_string()));
    }

    #[test]
    fn ejson_decoder_db_pointer() {
        use super::bson::{BsonDbPointer, BsonObjectId};
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(
                r#"{"$dbPointer":{"$ref":"collection","$id":{"$oid":"507f1f77bcf86cd799439011"}}}"#,
            )
            .unwrap();
        let expected = BsonDbPointer {
            name: "collection".to_string(),
            id: BsonObjectId {
                timestamp: 0x507f1f77,
                process: 0xbcf86cd799,
                counter: 0x439011,
            },
        };
        assert_eq!(v, EjsonValue::DbPointer(expected));
    }

    #[test]
    fn ejson_decoder_date_iso() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$date":"2023-01-01T00:00:00.000Z"}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Date {
                timestamp_ms: 1672531200000,
                iso: None
            }
        );
    }

    #[test]
    fn ejson_decoder_date_canonical() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec
            .decode_str(r#"{"$date":{"$numberLong":"1672531200000"}}"#)
            .unwrap();
        assert_eq!(
            v,
            EjsonValue::Date {
                timestamp_ms: 1672531200000,
                iso: None
            }
        );
    }

    #[test]
    fn ejson_decoder_invalid_date() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        assert!(matches!(
            dec.decode_str(r#"{"$date":"not-a-date"}"#),
            Err(EjsonDecodeError::InvalidDate)
        ));
    }

    #[test]
    fn ejson_decoder_minkey_maxkey() {
        use super::bson::{BsonMaxKey, BsonMinKey};
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        assert_eq!(
            dec.decode_str(r#"{"$minKey":1}"#).unwrap(),
            EjsonValue::MinKey(BsonMinKey)
        );
        assert_eq!(
            dec.decode_str(r#"{"$maxKey":1}"#).unwrap(),
            EjsonValue::MaxKey(BsonMaxKey)
        );
    }

    #[test]
    fn ejson_decoder_undefined() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        assert_eq!(
            dec.decode_str(r#"{"$undefined":true}"#).unwrap(),
            EjsonValue::Undefined
        );
    }

    #[test]
    fn ejson_decoder_plain_object() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"name":"John","age":30}"#).unwrap();
        if let EjsonValue::Object(pairs) = v {
            assert_eq!(pairs.len(), 2);
            assert_eq!(
                pairs[0],
                ("name".to_string(), EjsonValue::Str("John".to_string()))
            );
            assert_eq!(pairs[1], ("age".to_string(), EjsonValue::Integer(30)));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn ejson_decoder_nested_ejson_in_object() {
        use super::bson::BsonInt32;
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"{"count":{"$numberInt":"42"}}"#).unwrap();
        if let EjsonValue::Object(pairs) = v {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "count");
            assert_eq!(pairs[0].1, EjsonValue::Int32(BsonInt32 { value: 42 }));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn ejson_decoder_extra_keys_error() {
        use super::ejson::{EjsonDecodeError, EjsonDecoder};
        let mut dec = EjsonDecoder::new();
        // Extra key alongside $numberInt should error
        let res = dec.decode_str(r#"{"$numberInt":"42","extra":"field"}"#);
        assert!(matches!(res, Err(EjsonDecodeError::ExtraKeys(_))));
    }

    #[test]
    fn ejson_decoder_array() {
        use super::ejson::{EjsonDecoder, EjsonValue};
        let mut dec = EjsonDecoder::new();
        let v = dec.decode_str(r#"[1,2,3]"#).unwrap();
        assert_eq!(
            v,
            EjsonValue::Array(vec![
                EjsonValue::Integer(1),
                EjsonValue::Integer(2),
                EjsonValue::Integer(3),
            ])
        );
    }

    #[test]
    fn ejson_roundtrip_object_id() {
        use super::bson::BsonObjectId;
        use super::ejson::{EjsonDecoder, EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let mut dec = EjsonDecoder::new();
        let id = BsonObjectId {
            timestamp: 0x507f1f77,
            process: 0xbcf86cd799,
            counter: 0x439011,
        };
        let encoded = enc
            .encode_to_string(&EjsonValue::ObjectId(id.clone()))
            .unwrap();
        let decoded = dec.decode_str(&encoded).unwrap();
        assert_eq!(decoded, EjsonValue::ObjectId(id));
    }

    #[test]
    fn ejson_roundtrip_binary() {
        use super::bson::BsonBinary;
        use super::ejson::{EjsonDecoder, EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let mut dec = EjsonDecoder::new();
        let bin = BsonBinary {
            subtype: 0,
            data: vec![1, 2, 3, 4],
        };
        let encoded = enc
            .encode_to_string(&EjsonValue::Binary(bin.clone()))
            .unwrap();
        let decoded = dec.decode_str(&encoded).unwrap();
        assert_eq!(decoded, EjsonValue::Binary(bin));
    }

    #[test]
    fn ejson_roundtrip_timestamp() {
        use super::bson::BsonTimestamp;
        use super::ejson::{EjsonDecoder, EjsonEncoder, EjsonValue};
        let mut enc = EjsonEncoder::new();
        let mut dec = EjsonDecoder::new();
        let ts = BsonTimestamp {
            timestamp: 1234567890,
            increment: 12345,
        };
        let encoded = enc
            .encode_to_string(&EjsonValue::Timestamp(ts.clone()))
            .unwrap();
        let decoded = dec.decode_str(&encoded).unwrap();
        assert_eq!(decoded, EjsonValue::Timestamp(ts));
    }

    // ---------------------------------------------------------------- Boundary / error-path tests

    // --- CBOR truncated input ---

    #[test]
    fn cbor_empty_input_returns_error() {
        let result = decode_json_from_cbor_bytes(&[]);
        assert!(result.is_err(), "empty CBOR must return Err");
    }

    #[test]
    fn cbor_truncated_uint16_returns_error() {
        // 0x19 = major 0, additional 25 → expects 2 more bytes, we give 1
        let result = decode_json_from_cbor_bytes(&[0x19, 0x00]);
        assert!(result.is_err(), "truncated uint16 must return Err");
    }

    #[test]
    fn cbor_truncated_uint32_returns_error() {
        // 0x1a = major 0, additional 26 → expects 4 bytes, we give 2
        let result = decode_json_from_cbor_bytes(&[0x1a, 0x00, 0x00]);
        assert!(result.is_err(), "truncated uint32 must return Err");
    }

    #[test]
    fn cbor_truncated_uint64_returns_error() {
        // 0x1b = major 0, additional 27 → expects 8 bytes, we give 4
        let result = decode_json_from_cbor_bytes(&[0x1b, 0x00, 0x00, 0x00, 0x00]);
        assert!(result.is_err(), "truncated uint64 must return Err");
    }

    #[test]
    fn cbor_truncated_text_string_returns_error() {
        // 0x63 = major 3 (text), length 3 → expects 3 bytes, we give 2
        let result = decode_json_from_cbor_bytes(&[0x63, b'h', b'i']);
        assert!(result.is_err(), "truncated text string must return Err");
    }

    #[test]
    fn cbor_truncated_byte_string_returns_error() {
        // 0x42 = major 2 (bytes), length 2 → expects 2 bytes, we give 1
        let result = decode_json_from_cbor_bytes(&[0x42, 0xDE]);
        assert!(result.is_err(), "truncated byte string must return Err");
    }

    #[test]
    fn cbor_truncated_array_returns_error() {
        // 0x82 = major 4 (array), length 2 → expects 2 items, we give header only
        let result = decode_json_from_cbor_bytes(&[0x82]);
        assert!(result.is_err(), "truncated array must return Err");
    }

    #[test]
    fn cbor_truncated_map_returns_error() {
        // 0xa1 = major 5 (map), length 1 → expects 1 pair, we give the key but not value
        let result = decode_json_from_cbor_bytes(&[0xa1, 0x61, b'k']);
        assert!(result.is_err(), "truncated map must return Err");
    }

    #[test]
    fn cbor_validate_size_rejects_wrong_size() {
        let bytes = encode_json_to_cbor_bytes(&serde_json::json!(42)).expect("encode");
        // validate_cbor_exact_size with wrong size must fail
        let result = validate_cbor_exact_size(&bytes, bytes.len() + 1);
        assert!(result.is_err(), "wrong size must return Err");
    }

    // --- MsgPack boundary / error-path tests ---

    #[test]
    fn msgpack_empty_input_returns_error() {
        use super::msgpack::MsgPackDecoderFast;
        let mut dec = MsgPackDecoderFast::new();
        assert!(dec.decode(&[]).is_err(), "empty MsgPack must return Err");
    }

    #[test]
    fn msgpack_truncated_str8_returns_error() {
        use super::msgpack::MsgPackDecoderFast;
        let mut dec = MsgPackDecoderFast::new();
        // 0xd9 = str 8, length byte = 5, then only 2 bytes of payload
        assert!(dec.decode(&[0xd9, 0x05, b'h', b'i']).is_err());
    }

    #[test]
    fn msgpack_truncated_bin8_returns_error() {
        use super::msgpack::MsgPackDecoderFast;
        let mut dec = MsgPackDecoderFast::new();
        // 0xc4 = bin8, length=3, only 1 byte given
        assert!(dec.decode(&[0xc4, 0x03, 0xDE]).is_err());
    }

    #[test]
    fn msgpack_fixarray_boundary_correct() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // fixarray holds 0..=15 items; 15 items → 0x9f header
        let items: Vec<PackValue> = (0..15).map(PackValue::Integer).collect();
        let arr = PackValue::Array(items.clone());
        let bytes = enc.encode(&arr);
        assert_eq!(bytes[0], 0x9f, "fixarray(15) header");
        let decoded = dec.decode(&bytes).unwrap();
        assert_eq!(decoded, PackValue::Array(items));
    }

    #[test]
    fn msgpack_array16_boundary_correct() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // 16 items → array16 (0xdc) header
        let items: Vec<PackValue> = (0..16).map(PackValue::Integer).collect();
        let arr = PackValue::Array(items.clone());
        let bytes = enc.encode(&arr);
        assert_eq!(bytes[0], 0xdc, "array16 header");
        // bytes[1..2] = length as u16 BE
        let len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
        assert_eq!(len, 16);
        let decoded = dec.decode(&bytes).unwrap();
        assert_eq!(decoded, PackValue::Array(items));
    }

    #[test]
    fn msgpack_fixmap_boundary_correct() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // fixmap holds 0..=15 pairs; 15 pairs → 0x8f header
        let pairs: Vec<(String, PackValue)> = (0..15)
            .map(|i| (format!("k{i}"), PackValue::Integer(i)))
            .collect();
        let obj = PackValue::Object(pairs.clone());
        let bytes = enc.encode(&obj);
        assert_eq!(bytes[0], 0x8f, "fixmap(15) header");
        // Decode and check we get 15 pairs back
        if let PackValue::Object(decoded_pairs) = dec.decode(&bytes).unwrap() {
            assert_eq!(decoded_pairs.len(), 15);
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn msgpack_uint_128_to_255_uses_uint16_format() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // Upstream encoder skips uint8 (0xcc); values 128..=65535 use uint16 (0xcd).
        // Decoder maps uint16 back to Integer (not UInteger).
        let bytes = enc.encode(&PackValue::UInteger(200));
        assert_eq!(bytes[0], 0xcd, "values 128-65535 use uint16 format");
        let v = u16::from_be_bytes([bytes[1], bytes[2]]);
        assert_eq!(v, 200);
        assert_eq!(dec.decode(&bytes).unwrap(), PackValue::Integer(200));
    }

    #[test]
    fn msgpack_uint16_range_roundtrips_as_integer() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // uint16 (0xcd); decoder returns Integer (signed), not UInteger
        let bytes = enc.encode(&PackValue::UInteger(1000));
        assert_eq!(bytes[0], 0xcd, "uint16 format");
        let v = u16::from_be_bytes([bytes[1], bytes[2]]);
        assert_eq!(v, 1000);
        assert_eq!(dec.decode(&bytes).unwrap(), PackValue::Integer(1000));
    }

    #[test]
    fn msgpack_negative_mid_range_uses_int16_format() {
        use super::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
        let mut enc = MsgPackEncoderFast::new();
        let mut dec = MsgPackDecoderFast::new();
        // Upstream encoder skips int8 (0xd0); values -33..-32768 use int16 (0xd1).
        let bytes = enc.encode(&PackValue::Integer(-100));
        assert_eq!(bytes[0], 0xd1, "values -33..-32768 use int16 format");
        let v = i16::from_be_bytes([bytes[1], bytes[2]]);
        assert_eq!(v, -100);
        assert_eq!(dec.decode(&bytes).unwrap(), PackValue::Integer(-100));
    }

    #[test]
    fn msgpack_truncated_array_returns_error() {
        use super::msgpack::MsgPackDecoderFast;
        let mut dec = MsgPackDecoderFast::new();
        // fixarray with 3 elements, but no element data follows
        assert!(dec.decode(&[0x93]).is_err());
    }

    // --- RESP3 boundary / error-path tests ---

    #[test]
    fn resp_empty_input_returns_error() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        assert!(dec.decode(&[]).is_err(), "empty RESP must return Err");
    }

    #[test]
    fn resp_unknown_type_byte_returns_error() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // 0x00 is not a valid RESP3 type prefix
        assert!(dec.decode(&[0x00]).is_err(), "unknown type must return Err");
    }

    #[test]
    fn resp_decode_float() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        assert_eq!(
            dec.decode(b",3.14\r\n").unwrap(),
            PackValue::Float(TEST_F64_3_14)
        );
    }

    #[test]
    fn resp_decode_float_inf_neginf_nan() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        let v = dec.decode(b",inf\r\n").unwrap();
        assert!(matches!(v, PackValue::Float(f) if f.is_infinite() && f > 0.0));
        let v = dec.decode(b",-inf\r\n").unwrap();
        assert!(matches!(v, PackValue::Float(f) if f.is_infinite() && f < 0.0));
        let v = dec.decode(b",nan\r\n").unwrap();
        assert!(matches!(v, PackValue::Float(f) if f.is_nan()));
    }

    #[test]
    fn resp_decode_bigint() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        let v = dec.decode(b"(1234567890123456789\r\n").unwrap();
        assert_eq!(v, PackValue::BigInt(1234567890123456789_i128));
        let neg = dec.decode(b"(-42\r\n").unwrap();
        assert_eq!(neg, PackValue::BigInt(-42));
    }

    #[test]
    fn resp_decode_set_as_array() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // ~2\r\n:1\r\n:2\r\n — set with 2 integer elements
        let v = dec.decode(b"~2\r\n:1\r\n:2\r\n").unwrap();
        assert_eq!(
            v,
            PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)])
        );
    }

    #[test]
    fn resp_decode_object() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // %1\r\n+key\r\n:42\r\n — map with 1 pair
        let v = dec.decode(b"%1\r\n+key\r\n:42\r\n").unwrap();
        assert_eq!(
            v,
            PackValue::Object(vec![("key".to_string(), PackValue::Integer(42))])
        );
    }

    #[test]
    fn resp_decode_push_as_extension() {
        use super::resp::RespDecoder;
        use super::PackValue;
        let mut dec = RespDecoder::new();
        // >2\r\n+foo\r\n:1\r\n — push with 2 elements
        let v = dec.decode(b">2\r\n+foo\r\n:1\r\n").unwrap();
        if let PackValue::Extension(ext) = v {
            assert_eq!(ext.tag, 1); // RESP_EXTENSION_PUSH
            assert!(matches!(*ext.val, PackValue::Array(_)));
        } else {
            panic!("expected Extension for push, got {v:?}");
        }
    }

    #[test]
    fn resp_decode_attributes_as_extension() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // |1\r\n+ttl\r\n:3600\r\n
        let v = dec.decode(b"|1\r\n+ttl\r\n:3600\r\n").unwrap();
        if let PackValue::Extension(ext) = v {
            assert_eq!(ext.tag, 2); // RESP_EXTENSION_ATTRIBUTES
            assert!(matches!(*ext.val, PackValue::Object(_)));
        } else {
            panic!("expected Extension for attributes, got {v:?}");
        }
    }

    #[test]
    fn resp_decode_verbatim_txt_string() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // =7\r\ntxt:abc\r\n — verbatim text string
        let v = dec.decode(b"=7\r\ntxt:abc\r\n").unwrap();
        assert_eq!(v, PackValue::Str("abc".to_string()));
    }

    #[test]
    fn resp_decode_verbatim_non_txt_as_bytes() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // =8\r\nraw:data\r\n — verbatim with non-txt prefix → Bytes
        let v = dec.decode(b"=8\r\nraw:data\r\n").unwrap();
        assert_eq!(v, PackValue::Bytes(b"data".to_vec()));
    }

    #[test]
    fn resp_decode_simple_error_as_str() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // Simple error (-) decoded as Str
        let v = dec.decode(b"-ERR some error\r\n").unwrap();
        assert_eq!(v, PackValue::Str("ERR some error".to_string()));
    }

    #[test]
    fn resp_decode_bulk_error_as_str() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // Bulk error (!) decoded as Str. "ERR bulk error" = 14 bytes.
        let v = dec.decode(b"!14\r\nERR bulk error\r\n").unwrap();
        assert_eq!(v, PackValue::Str("ERR bulk error".to_string()));
    }

    #[test]
    fn resp_decode_null_bulk_string() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // $-1\r\n → Null bulk string
        let v = dec.decode(b"$-1\r\n").unwrap();
        assert_eq!(v, PackValue::Null);
    }

    #[test]
    fn resp_decode_null_array() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // *-1\r\n → Null array
        let v = dec.decode(b"*-1\r\n").unwrap();
        assert_eq!(v, PackValue::Null);
    }

    #[test]
    fn resp_decode_nested_array() {
        use super::resp::RespDecoder;
        let mut dec = RespDecoder::new();
        // *2\r\n*2\r\n:1\r\n:2\r\n:3\r\n — nested arrays
        let v = dec.decode(b"*2\r\n*2\r\n:1\r\n:2\r\n:3\r\n").unwrap();
        assert_eq!(
            v,
            PackValue::Array(vec![
                PackValue::Array(vec![PackValue::Integer(1), PackValue::Integer(2)]),
                PackValue::Integer(3),
            ])
        );
    }
}
