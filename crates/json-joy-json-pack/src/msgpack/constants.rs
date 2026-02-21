//! MessagePack marker constants.
//!
//! Upstream reference: `json-pack/src/msgpack/constants.ts`

/// Core one-byte MessagePack markers surfaced by upstream constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MsgPackMarker {
    Null = 0xc0,
    Undefined = 0xc1,
    False = 0xc2,
    True = 0xc3,
}
