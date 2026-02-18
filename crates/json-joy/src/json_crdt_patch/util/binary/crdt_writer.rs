//! [`CrdtWriter`] — extends [`Writer`] with CRDT timestamp encoding.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/util/binary/CrdtWriter.ts`.
//!
//! Encoding formats:
//!
//! ## `vu57` — variable-length unsigned 57-bit integer (up to 8 bytes)
//!
//! Each byte's high bit is a continuation flag; the remaining 7 bits carry
//! payload, little-endian order (LSB first).
//!
//! ## `b1vu56` — 1-bit flag + variable-length unsigned 56-bit integer (up to 8 bytes)
//!
//! Like `vu57` but the first byte format is `|f?zzzzzz|` — the top bit is the
//! user-supplied flag, the second bit is the continuation flag, and the
//! remaining 6 bits are the lowest 6 bits of the payload.
//!
//! ## `id(x, y)` — compact CRDT relative ID
//!
//! If `x <= 7` and `y <= 15`, encodes as a single byte `|0xxxyyyy|`.
//! Otherwise encodes `x` via `b1vu56(1, x)` then `y` via `vu57`.

use json_joy_buffers::Writer;

/// A writer that extends [`Writer`] with CRDT-specific variable-length encodings.
pub struct CrdtWriter {
    pub inner: Writer,
}

impl Default for CrdtWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CrdtWriter {
    /// Creates a new `CrdtWriter` with the default 64 KB allocation.
    pub fn new() -> Self {
        Self { inner: Writer::new() }
    }

    /// Creates a new `CrdtWriter` with a custom initial allocation size.
    pub fn with_alloc_size(size: usize) -> Self {
        Self { inner: Writer::with_alloc_size(size) }
    }

    // ── Delegation to inner Writer ─────────────────────────────────────────

    #[inline] pub fn reset(&mut self) { self.inner.reset(); }
    #[inline] pub fn flush(&mut self) -> Vec<u8> { self.inner.flush() }
    #[inline] pub fn u8(&mut self, val: u8) { self.inner.u8(val); }
    #[inline] pub fn ensure_capacity(&mut self, n: usize) { self.inner.ensure_capacity(n); }

    /// Writes raw bytes.
    #[inline]
    pub fn buf(&mut self, data: &[u8]) {
        self.inner.ensure_capacity(data.len());
        let x = self.inner.x;
        self.inner.uint8[x..x + data.len()].copy_from_slice(data);
        self.inner.x += data.len();
    }

    /// Writes a UTF-8 string and returns the byte length written.
    #[inline]
    pub fn utf8(&mut self, s: &str) -> usize {
        self.inner.utf8(s)
    }

    // ── CRDT-specific encodings ────────────────────────────────────────────

    /// Encodes a compact CRDT relative ID.
    ///
    /// - If `x <= 7` and `y <= 15`: encodes as a single byte `|0xxxyyyy|`.
    /// - Otherwise: `b1vu56(1, x)` followed by `vu57(y)`.
    pub fn id(&mut self, x: u64, y: u64) {
        if x <= 0b111 && y <= 0b1111 {
            self.inner.u8((x as u8) << 4 | (y as u8));
        } else {
            self.b1vu56(1, x);
            self.vu57(y);
        }
    }

    /// Encodes a 57-bit variable-length unsigned integer.
    ///
    /// Uses 1–8 bytes. Each byte's MSB is a continuation flag; payload bits
    /// are in little-endian 7-bit groups.
    pub fn vu57(&mut self, num: u64) {
        if num <= 0x7F {
            self.inner.u8(num as u8);
        } else if num <= 0x3FFF {
            self.inner.ensure_capacity(2);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = (num >> 7) as u8;
            self.inner.x += 2;
        } else if num <= 0x1F_FFFF {
            self.inner.ensure_capacity(3);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = (num >> 14) as u8;
            self.inner.x += 3;
        } else if num <= 0xFFF_FFFF {
            self.inner.ensure_capacity(4);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = 0x80 | ((num >> 14) & 0x7F) as u8;
            self.inner.uint8[x + 3] = (num >> 21) as u8;
            self.inner.x += 4;
        } else if num <= 0x7_FFFF_FFFF {
            self.inner.ensure_capacity(5);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = 0x80 | ((num >> 14) & 0x7F) as u8;
            self.inner.uint8[x + 3] = 0x80 | ((num >> 21) & 0x7F) as u8;
            self.inner.uint8[x + 4] = (num >> 28) as u8;
            self.inner.x += 5;
        } else if num <= 0x3FF_FFFF_FFFF {
            self.inner.ensure_capacity(6);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = 0x80 | ((num >> 14) & 0x7F) as u8;
            self.inner.uint8[x + 3] = 0x80 | ((num >> 21) & 0x7F) as u8;
            self.inner.uint8[x + 4] = 0x80 | ((num >> 28) & 0x7F) as u8;
            self.inner.uint8[x + 5] = (num >> 35) as u8;
            self.inner.x += 6;
        } else if num <= 0x1_FFFF_FFFF_FFFF {
            self.inner.ensure_capacity(7);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = 0x80 | ((num >> 14) & 0x7F) as u8;
            self.inner.uint8[x + 3] = 0x80 | ((num >> 21) & 0x7F) as u8;
            self.inner.uint8[x + 4] = 0x80 | ((num >> 28) & 0x7F) as u8;
            self.inner.uint8[x + 5] = 0x80 | ((num >> 35) & 0x7F) as u8;
            self.inner.uint8[x + 6] = (num >> 42) as u8;
            self.inner.x += 7;
        } else {
            // Up to 57 bits (8 bytes)
            self.inner.ensure_capacity(8);
            let x = self.inner.x;
            self.inner.uint8[x]     = 0x80 | (num & 0x7F) as u8;
            self.inner.uint8[x + 1] = 0x80 | ((num >> 7) & 0x7F) as u8;
            self.inner.uint8[x + 2] = 0x80 | ((num >> 14) & 0x7F) as u8;
            self.inner.uint8[x + 3] = 0x80 | ((num >> 21) & 0x7F) as u8;
            self.inner.uint8[x + 4] = 0x80 | ((num >> 28) & 0x7F) as u8;
            self.inner.uint8[x + 5] = 0x80 | ((num >> 35) & 0x7F) as u8;
            self.inner.uint8[x + 6] = 0x80 | ((num >> 42) & 0x7F) as u8;
            self.inner.uint8[x + 7] = (num >> 49) as u8;
            self.inner.x += 8;
        }
    }

    /// Encodes a 1-bit flag followed by a 56-bit variable-length unsigned integer.
    ///
    /// First byte format: `|f?zzzzzz|` where `f` is the flag, `?` is the
    /// continuation bit, and `z` are the 6 low bits of the payload.
    pub fn b1vu56(&mut self, flag: u8, num: u64) {
        let flag_bit = (flag as u64) << 7;
        if num <= 0x3F {
            self.inner.u8((flag_bit | num) as u8);
        } else {
            let first = flag_bit | 0x40 | (num & 0x3F);
            if num <= 0x1FFF {
                self.inner.ensure_capacity(2);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = (num >> 6) as u8;
                self.inner.x += 2;
            } else if num <= 0xF_FFFF {
                self.inner.ensure_capacity(3);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = (num >> 13) as u8;
                self.inner.x += 3;
            } else if num <= 0x7FF_FFFF {
                self.inner.ensure_capacity(4);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = 0x80 | ((num >> 13) & 0x7F) as u8;
                self.inner.uint8[x + 3] = (num >> 20) as u8;
                self.inner.x += 4;
            } else if num <= 0x3F_FFFF_FFFF {
                self.inner.ensure_capacity(5);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = 0x80 | ((num >> 13) & 0x7F) as u8;
                self.inner.uint8[x + 3] = 0x80 | ((num >> 20) & 0x7F) as u8;
                self.inner.uint8[x + 4] = (num >> 27) as u8;
                self.inner.x += 5;
            } else if num <= 0x1FF_FFFF_FFFF {
                self.inner.ensure_capacity(6);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = 0x80 | ((num >> 13) & 0x7F) as u8;
                self.inner.uint8[x + 3] = 0x80 | ((num >> 20) & 0x7F) as u8;
                self.inner.uint8[x + 4] = 0x80 | ((num >> 27) & 0x7F) as u8;
                self.inner.uint8[x + 5] = (num >> 34) as u8;
                self.inner.x += 6;
            } else if num <= 0xFFFF_FFFF_FFFF {
                self.inner.ensure_capacity(7);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = 0x80 | ((num >> 13) & 0x7F) as u8;
                self.inner.uint8[x + 3] = 0x80 | ((num >> 20) & 0x7F) as u8;
                self.inner.uint8[x + 4] = 0x80 | ((num >> 27) & 0x7F) as u8;
                self.inner.uint8[x + 5] = 0x80 | ((num >> 34) & 0x7F) as u8;
                self.inner.uint8[x + 6] = (num >> 41) as u8;
                self.inner.x += 7;
            } else {
                // 56-bit max: 8 bytes
                self.inner.ensure_capacity(8);
                let x = self.inner.x;
                self.inner.uint8[x]     = first as u8;
                self.inner.uint8[x + 1] = 0x80 | ((num >> 6) & 0x7F) as u8;
                self.inner.uint8[x + 2] = 0x80 | ((num >> 13) & 0x7F) as u8;
                self.inner.uint8[x + 3] = 0x80 | ((num >> 20) & 0x7F) as u8;
                self.inner.uint8[x + 4] = 0x80 | ((num >> 27) & 0x7F) as u8;
                self.inner.uint8[x + 5] = 0x80 | ((num >> 34) & 0x7F) as u8;
                self.inner.uint8[x + 6] = 0x80 | ((num >> 41) & 0x7F) as u8;
                self.inner.uint8[x + 7] = (num >> 48) as u8;
                self.inner.x += 8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::util::binary::CrdtReader;

    fn roundtrip_vu57(n: u64) -> u64 {
        let mut w = CrdtWriter::new();
        w.vu57(n);
        let data = w.flush();
        let mut r = CrdtReader::new(&data);
        r.vu57()
    }

    fn roundtrip_b1vu56(flag: u8, n: u64) -> (u8, u64) {
        let mut w = CrdtWriter::new();
        w.b1vu56(flag, n);
        let data = w.flush();
        let mut r = CrdtReader::new(&data);
        r.b1vu56()
    }

    #[test]
    fn vu57_small() {
        assert_eq!(roundtrip_vu57(0), 0);
        assert_eq!(roundtrip_vu57(127), 127);
    }

    #[test]
    fn vu57_medium() {
        assert_eq!(roundtrip_vu57(128), 128);
        assert_eq!(roundtrip_vu57(16383), 16383);
        assert_eq!(roundtrip_vu57(16384), 16384);
    }

    #[test]
    fn vu57_large() {
        assert_eq!(roundtrip_vu57(1_000_000), 1_000_000);
        assert_eq!(roundtrip_vu57(9_007_199_254_740_991), 9_007_199_254_740_991);
    }

    #[test]
    fn b1vu56_flag_0() {
        let (f, v) = roundtrip_b1vu56(0, 42);
        assert_eq!(f, 0);
        assert_eq!(v, 42);
    }

    #[test]
    fn b1vu56_flag_1() {
        let (f, v) = roundtrip_b1vu56(1, 10000);
        assert_eq!(f, 1);
        assert_eq!(v, 10000);
    }

    #[test]
    fn b1vu56_large() {
        let big = 9_007_199_254_740_991u64;
        let (f, v) = roundtrip_b1vu56(1, big);
        assert_eq!(f, 1);
        assert_eq!(v, big);
    }
}
