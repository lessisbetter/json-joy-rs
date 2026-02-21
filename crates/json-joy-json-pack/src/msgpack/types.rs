//! MessagePack type aliases and compatibility traits.
//!
//! Upstream reference: `json-pack/src/msgpack/types.ts`

use crate::PackValue;

/// Binary MessagePack payload alias.
pub type MsgPack = Vec<u8>;

/// Legacy encoder trait mapped from upstream `IMessagePackEncoder`.
///
/// This is intentionally lightweight and exists as an API-surface bridge.
pub trait IMessagePackEncoder {
    fn encode_any(&mut self, value: &PackValue);
    fn encode_number(&mut self, num: f64);
    fn encode_string(&mut self, value: &str);
    fn encode_array(&mut self, values: &[PackValue]);
    fn encode_array_header(&mut self, length: usize);
    fn encode_object(&mut self, values: &[(String, PackValue)]);
    fn encode_object_header(&mut self, length: usize);
}
