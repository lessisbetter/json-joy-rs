//! Binary codec for the JSON CRDT Patch protocol.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/binary/`.

mod encoder;
mod decoder;

pub use encoder::Encoder;
pub use decoder::{Decoder, DecodeError};

use crate::json_crdt_patch::patch::Patch;

/// Encodes a patch to binary using a shared encoder instance.
pub fn encode(patch: &Patch) -> Vec<u8> {
    let mut enc = Encoder::new();
    enc.encode(patch)
}

/// Decodes a binary blob into a patch.
pub fn decode(data: &[u8]) -> Result<Patch, DecodeError> {
    let dec = Decoder::new();
    dec.decode(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use json_joy_json_pack::PackValue;
    use crate::json_crdt_patch::clock::{ts, interval};
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use crate::json_crdt_patch::patch::Patch;

    fn sid() -> u64 { 1 }
    fn t(time: u64) -> crate::json_crdt_patch::clock::Ts { ts(sid(), time) }
    fn other(time: u64) -> crate::json_crdt_patch::clock::Ts { ts(2, time) }

    fn roundtrip(ops: Vec<Op>) -> Patch {
        let mut patch = Patch::new();
        for op in ops {
            patch.ops.push(op);
        }
        let bytes = encode(&patch);
        decode(&bytes).expect("decode must succeed")
    }

    #[test]
    fn new_con_null() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Null) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_con_bool() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Bool(true)) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_con_integer() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Integer(-42)) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_con_float() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Float(3.14)) }];
        let out = roundtrip(ops.clone());
        match &out.ops[0] {
            Op::NewCon { val: ConValue::Val(PackValue::Float(f)), .. } => {
                assert!((f - 3.14_f64).abs() < 1e-10, "float mismatch: {f}");
            }
            other => panic!("unexpected op: {other:?}"),
        }
    }

    #[test]
    fn new_con_string() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Str("hello".to_string())) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_con_binary() {
        let ops = vec![Op::NewCon { id: t(1), val: ConValue::Val(PackValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_con_ref() {
        let ops = vec![Op::NewCon { id: t(2), val: ConValue::Ref(other(5)) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_val() {
        let ops = vec![Op::NewVal { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_obj() {
        let ops = vec![Op::NewObj { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_vec() {
        let ops = vec![Op::NewVec { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_str() {
        let ops = vec![Op::NewStr { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_bin() {
        let ops = vec![Op::NewBin { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn new_arr() {
        let ops = vec![Op::NewArr { id: t(1) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_val() {
        let ops = vec![Op::InsVal { id: t(10), obj: t(1), val: t(5) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_obj() {
        let ops = vec![Op::InsObj {
            id: t(10),
            obj: t(1),
            data: vec![("key".to_string(), t(5)), ("other".to_string(), other(3))],
        }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_vec() {
        let ops = vec![Op::InsVec {
            id: t(10),
            obj: t(1),
            data: vec![(0, t(5)), (1, other(3))],
        }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_str() {
        let ops = vec![Op::InsStr { id: t(10), obj: t(1), after: t(0), data: "hello".to_string() }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_bin() {
        let ops = vec![Op::InsBin { id: t(10), obj: t(1), after: t(0), data: vec![1, 2, 3] }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn ins_arr() {
        let ops = vec![Op::InsArr { id: t(10), obj: t(1), after: t(0), data: vec![t(5), other(7)] }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn upd_arr() {
        let ops = vec![Op::UpdArr { id: t(10), obj: t(1), after: t(5), val: other(3) }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn del_single_range() {
        // interval(stamp, tick_offset, span): deletes 2 ticks starting from t(3)+0
        let ops = vec![Op::Del { id: t(10), obj: t(1), what: vec![interval(t(3), 0, 2)] }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn del_multi_range() {
        let ops = vec![Op::Del {
            id: t(10),
            obj: t(1),
            what: vec![interval(t(3), 0, 1), interval(other(7), 0, 3)],
        }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn nop() {
        let ops = vec![Op::Nop { id: t(1), len: 5 }];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn multi_op_patch() {
        // A patch containing multiple different operations.
        let ops = vec![
            Op::NewStr { id: t(1) },
            Op::InsStr { id: t(2), obj: t(1), after: t(0), data: "hi".to_string() },
            Op::Del { id: t(4), obj: t(1), what: vec![interval(t(2), 0, 1)] },
            Op::Nop { id: t(5), len: 1 },
        ];
        let out = roundtrip(ops.clone());
        assert_eq!(out.ops, ops);
    }

    #[test]
    fn empty_input_returns_error() {
        assert!(decode(&[]).is_err());
        assert!(decode(&[0u8]).is_err());
        assert!(decode(&[0u8, 0u8]).is_err());
    }

    #[test]
    fn truncated_input_returns_error() {
        // Encode a valid patch, then trim it to <3 bytes (the guard threshold).
        // All inputs shorter than 3 bytes must return Err without panicking.
        let mut patch = Patch::new();
        patch.ops.push(Op::NewStr { id: t(1) });
        let bytes = encode(&patch);
        assert!(bytes.len() >= 3, "encoded patch must be at least 3 bytes");
        for len in 0..3usize.min(bytes.len()) {
            let result = decode(&bytes[..len]);
            assert!(result.is_err(), "short input of len {len} must return Err");
        }
    }
}
