// Minimal constants used by JSON-focused CBOR encode/decode helpers.
pub const MAJOR_UNSIGNED: u8 = 0;
pub const MAJOR_NEGATIVE: u8 = 1;
pub const MAJOR_BYTES: u8 = 2;
pub const MAJOR_ARRAY: u8 = 4;
pub const MAJOR_MAP: u8 = 5;
pub const MAJOR_TAG: u8 = 6;

pub fn is_f32_roundtrip(value: f64) -> bool {
    (value as f32) as f64 == value
}
