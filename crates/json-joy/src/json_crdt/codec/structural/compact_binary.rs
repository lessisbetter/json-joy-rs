//! Structural compact-binary codec.
//!
//! Mirrors:
//! - `structural/compact-binary/Encoder.ts`
//! - `structural/compact-binary/Decoder.ts`
//!
//! The format is simply the compact JSON format serialized with MessagePack.

use crate::json_crdt::codec::structural::compact;
use crate::json_crdt::model::Model;
use json_joy_json_pack::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
use json_joy_json_pack::PackValue;

/// Errors that can occur during compact-binary decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("msgpack decode error: {0}")]
    MsgPack(String),
    #[error("compact decode error: {0}")]
    Compact(#[from] compact::DecodeError),
}

/// Encode a [`Model`] to the compact-binary format (compact JSON → MessagePack).
pub fn encode(model: &Model) -> Vec<u8> {
    let json_val = compact::encode(model);
    let pack_val = PackValue::from(json_val);
    let mut enc = MsgPackEncoderFast::new();
    enc.encode(&pack_val)
}

/// Decode a compact-binary document back into a [`Model`].
pub fn decode(data: &[u8]) -> Result<Model, DecodeError> {
    let mut dec = MsgPackDecoderFast::new();
    let pack_val = dec
        .decode(data)
        .map_err(|e| DecodeError::MsgPack(format!("{:?}", e)))?;
    let json_val = serde_json::Value::from(pack_val);
    let model = compact::decode(&json_val)?;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 {
        111222
    }

    #[test]
    fn roundtrip_string() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: crate::json_crdt::constants::ORIGIN,
            data: "compact-binary".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let bytes = encode(&model);
        let decoded = decode(&bytes).expect("decode");
        assert_eq!(decoded.view(), view);
    }

    #[test]
    fn roundtrip_number() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(99)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let bytes = encode(&model);
        let decoded = decode(&bytes).expect("decode");
        assert_eq!(decoded.view(), view);
    }
}
