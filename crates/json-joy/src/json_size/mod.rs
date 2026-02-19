//! json-size — approximate MessagePack encoding size estimation.
//!
//! Mirrors `packages/json-joy/src/json-size/msgpackSizeFast.ts`.

use json_joy_json_pack::PackValue;

/// Approximate the byte size of a [`PackValue`] when encoded as MessagePack.
///
/// Same heuristic as the upstream `msgpackSizeFast`:
/// - null / undefined → 1 byte
/// - bool → 1 byte
/// - number → 9 bytes (worst-case: 1 header + 8-byte float64)
/// - string → 4 + byte length (1..4 header bytes)
/// - bytes (`Uint8Array`) → 5 + byte length
/// - array → 2 + sum of element sizes
/// - object → 2 + sum of (2 + key bytes + value size) per entry
/// - pre-encoded blob → raw byte length as-is
/// - extension → 6 + recursive size of contained value
pub fn msgpack_size_fast(value: &PackValue) -> usize {
    match value {
        PackValue::Null | PackValue::Undefined => 1,
        PackValue::Bool(_) => 1,
        PackValue::Integer(_)
        | PackValue::UInteger(_)
        | PackValue::Float(_)
        | PackValue::BigInt(_) => 9,
        PackValue::Str(s) => 4 + s.len(),
        PackValue::Bytes(b) => 5 + b.len(),
        PackValue::Array(arr) => {
            let mut size: usize = 2;
            for item in arr {
                size += msgpack_size_fast(item);
            }
            size
        }
        PackValue::Object(obj) => {
            let mut size: usize = 2;
            for (key, val) in obj {
                size += 2 + key.len() + msgpack_size_fast(val);
            }
            size
        }
        PackValue::Blob(blob) => blob.val.len(),
        // Note: upstream JsonPackExtension.val is a raw Uint8Array (byte count = 6 + val.length).
        // In the Rust port JsonPackExtension.val is Box<PackValue>, so we recurse to estimate its
        // encoded size. This is an intentional approximation; the result differs from upstream
        // when the inner value is not a flat byte blob.
        PackValue::Extension(ext) => 6 + msgpack_size_fast(&ext.val),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use json_joy_json_pack::{JsonPackExtension, JsonPackValue};

    #[test]
    fn null_is_one_byte() {
        assert_eq!(msgpack_size_fast(&PackValue::Null), 1);
        assert_eq!(msgpack_size_fast(&PackValue::Undefined), 1);
    }

    #[test]
    fn bool_is_one_byte() {
        assert_eq!(msgpack_size_fast(&PackValue::Bool(true)), 1);
        assert_eq!(msgpack_size_fast(&PackValue::Bool(false)), 1);
    }

    #[test]
    fn numbers_are_nine_bytes() {
        assert_eq!(msgpack_size_fast(&PackValue::Integer(-42)), 9);
        assert_eq!(msgpack_size_fast(&PackValue::UInteger(u64::MAX)), 9);
        assert_eq!(msgpack_size_fast(&PackValue::Float(3.14)), 9);
        assert_eq!(msgpack_size_fast(&PackValue::BigInt(i128::MAX)), 9);
    }

    #[test]
    fn string_size() {
        assert_eq!(msgpack_size_fast(&PackValue::Str("".to_owned())), 4);
        assert_eq!(msgpack_size_fast(&PackValue::Str("hello".to_owned())), 9); // 4 + 5
    }

    #[test]
    fn bytes_size() {
        assert_eq!(msgpack_size_fast(&PackValue::Bytes(vec![1, 2, 3])), 8); // 5 + 3
    }

    #[test]
    fn empty_array() {
        assert_eq!(msgpack_size_fast(&PackValue::Array(vec![])), 2);
    }

    #[test]
    fn array_with_items() {
        let arr = PackValue::Array(vec![
            PackValue::Null,
            PackValue::Bool(true),
            PackValue::Integer(42),
        ]);
        // 2 + 1 + 1 + 9 = 13
        assert_eq!(msgpack_size_fast(&arr), 13);
    }

    #[test]
    fn object_size() {
        let obj = PackValue::Object(vec![("key".to_owned(), PackValue::Integer(1))]);
        // 2 + (2 + 3 + 9) = 16
        assert_eq!(msgpack_size_fast(&obj), 16);
    }

    #[test]
    fn blob_size() {
        let blob = PackValue::Blob(JsonPackValue::new(vec![0xAA, 0xBB, 0xCC]));
        assert_eq!(msgpack_size_fast(&blob), 3);
    }

    #[test]
    fn extension_size() {
        let ext = PackValue::Extension(Box::new(JsonPackExtension::new(
            1,
            PackValue::Str("hi".to_owned()),
        )));
        // 6 + (4 + 2) = 12
        assert_eq!(msgpack_size_fast(&ext), 12);
    }
}
