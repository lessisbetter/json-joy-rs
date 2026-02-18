//! [`CrdtReader`] — extends a byte-slice reader with CRDT timestamp decoding.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/util/binary/CrdtReader.ts`.

/// A stateful byte-slice reader with CRDT-specific decoding helpers.
pub struct CrdtReader<'a> {
    pub data: &'a [u8],
    pub x: usize,
}

impl<'a> CrdtReader<'a> {
    /// Creates a new reader over the given byte slice.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, x: 0 }
    }

    /// Resets the reader to a new byte slice.
    pub fn reset<'b>(data: &'b [u8]) -> CrdtReader<'b> {
        CrdtReader { data, x: 0 }
    }

    /// Reads an unsigned 8-bit integer.
    #[inline]
    pub fn u8(&mut self) -> u8 {
        let v = self.data[self.x];
        self.x += 1;
        v
    }

    /// Returns a slice of `len` bytes and advances the cursor.
    #[inline]
    pub fn buf(&mut self, len: usize) -> &'a [u8] {
        let start = self.x;
        self.x += len;
        &self.data[start..self.x]
    }

    /// Reads `len` bytes as a UTF-8 string.
    #[inline]
    pub fn utf8(&mut self, len: usize) -> &'a str {
        let start = self.x;
        self.x += len;
        std::str::from_utf8(&self.data[start..self.x]).unwrap_or("")
    }

    /// Decodes a compact CRDT relative ID, returning `(x, y)`.
    ///
    /// If the high bit of the first byte is 0, the entire ID fits in one byte:
    /// `|0xxxyyyy|` → `(x, y)`.
    /// Otherwise decodes `x` from `b1vu56` and `y` from `vu57`.
    pub fn id(&mut self) -> (u64, u64) {
        let byte = self.u8();
        if byte <= 0b0111_1111 {
            ((byte >> 4) as u64, (byte & 0x0F) as u64)
        } else {
            self.x -= 1;
            let (_, x) = self.b1vu56();
            let y = self.vu57();
            (x, y)
        }
    }

    /// Skips a compact CRDT relative ID without decoding it.
    pub fn id_skip(&mut self) {
        let byte = self.u8();
        if byte <= 0b0111_1111 {
            return;
        }
        self.x -= 1;
        self.b1vu56();
        self.vu57_skip();
    }

    /// Decodes a 57-bit variable-length unsigned integer.
    pub fn vu57(&mut self) -> u64 {
        let o1 = self.u8() as u64;
        if o1 <= 0x7F { return o1; }
        let o2 = self.u8() as u64;
        if o2 <= 0x7F { return (o2 << 7) | (o1 & 0x7F); }
        let o3 = self.u8() as u64;
        if o3 <= 0x7F { return (o3 << 14) | ((o2 & 0x7F) << 7) | (o1 & 0x7F); }
        let o4 = self.u8() as u64;
        if o4 <= 0x7F { return (o4 << 21) | ((o3 & 0x7F) << 14) | ((o2 & 0x7F) << 7) | (o1 & 0x7F); }
        let o5 = self.u8() as u64;
        if o5 <= 0x7F {
            return (o5 << 28)
                | ((o4 & 0x7F) << 21)
                | ((o3 & 0x7F) << 14)
                | ((o2 & 0x7F) << 7)
                | (o1 & 0x7F);
        }
        let o6 = self.u8() as u64;
        if o6 <= 0x7F {
            return (o6 << 35)
                | ((o5 & 0x7F) << 28)
                | ((o4 & 0x7F) << 21)
                | ((o3 & 0x7F) << 14)
                | ((o2 & 0x7F) << 7)
                | (o1 & 0x7F);
        }
        let o7 = self.u8() as u64;
        if o7 <= 0x7F {
            return (o7 << 42)
                | ((o6 & 0x7F) << 35)
                | ((o5 & 0x7F) << 28)
                | ((o4 & 0x7F) << 21)
                | ((o3 & 0x7F) << 14)
                | ((o2 & 0x7F) << 7)
                | (o1 & 0x7F);
        }
        let o8 = self.u8() as u64;
        (o8 << 49)
            | ((o7 & 0x7F) << 42)
            | ((o6 & 0x7F) << 35)
            | ((o5 & 0x7F) << 28)
            | ((o4 & 0x7F) << 21)
            | ((o3 & 0x7F) << 14)
            | ((o2 & 0x7F) << 7)
            | (o1 & 0x7F)
    }

    /// Skips a `vu57` value without decoding it.
    pub fn vu57_skip(&mut self) {
        loop {
            let b = self.u8();
            if b <= 0x7F { return; }
        }
    }

    /// Decodes a 1-bit flag + 56-bit variable-length unsigned integer.
    ///
    /// Returns `(flag, value)`.
    pub fn b1vu56(&mut self) -> (u8, u64) {
        let byte = self.u8() as u64;
        let flag = ((byte >> 7) & 1) as u8;
        let o1 = byte & 0x7F; // strip the top bit (flag)
        // continuation bit is bit 6 of the masked byte (bit 6 of 0b0_?_zzzzzz)
        if o1 <= 0x3F {
            // no continuation: 6 payload bits
            return (flag, o1);
        }
        let o2 = self.u8() as u64;
        if o2 <= 0x7F { return (flag, (o2 << 6) | (o1 & 0x3F)); }
        let o3 = self.u8() as u64;
        if o3 <= 0x7F { return (flag, (o3 << 13) | ((o2 & 0x7F) << 6) | (o1 & 0x3F)); }
        let o4 = self.u8() as u64;
        if o4 <= 0x7F {
            return (flag, (o4 << 20) | ((o3 & 0x7F) << 13) | ((o2 & 0x7F) << 6) | (o1 & 0x3F));
        }
        let o5 = self.u8() as u64;
        if o5 <= 0x7F {
            return (
                flag,
                (o5 << 27)
                    | ((o4 & 0x7F) << 20)
                    | ((o3 & 0x7F) << 13)
                    | ((o2 & 0x7F) << 6)
                    | (o1 & 0x3F),
            );
        }
        let o6 = self.u8() as u64;
        if o6 <= 0x7F {
            return (
                flag,
                (o6 << 34)
                    | ((o5 & 0x7F) << 27)
                    | ((o4 & 0x7F) << 20)
                    | ((o3 & 0x7F) << 13)
                    | ((o2 & 0x7F) << 6)
                    | (o1 & 0x3F),
            );
        }
        let o7 = self.u8() as u64;
        if o7 <= 0x7F {
            return (
                flag,
                (o7 << 41)
                    | ((o6 & 0x7F) << 34)
                    | ((o5 & 0x7F) << 27)
                    | ((o4 & 0x7F) << 20)
                    | ((o3 & 0x7F) << 13)
                    | ((o2 & 0x7F) << 6)
                    | (o1 & 0x3F),
            );
        }
        let o8 = self.u8() as u64;
        (
            flag,
            (o8 << 48)
                | ((o7 & 0x7F) << 41)
                | ((o6 & 0x7F) << 34)
                | ((o5 & 0x7F) << 27)
                | ((o4 & 0x7F) << 20)
                | ((o3 & 0x7F) << 13)
                | ((o2 & 0x7F) << 6)
                | (o1 & 0x3F),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_vu57(n: u64) -> Vec<u8> {
        use crate::json_crdt_patch::util::binary::CrdtWriter;
        let mut w = CrdtWriter::new();
        w.vu57(n);
        w.flush()
    }

    fn encode_b1vu56(flag: u8, n: u64) -> Vec<u8> {
        use crate::json_crdt_patch::util::binary::CrdtWriter;
        let mut w = CrdtWriter::new();
        w.b1vu56(flag, n);
        w.flush()
    }

    #[test]
    fn vu57_zero() {
        let data = encode_vu57(0);
        let mut r = CrdtReader::new(&data);
        assert_eq!(r.vu57(), 0);
    }

    #[test]
    fn vu57_127() {
        let data = encode_vu57(127);
        let mut r = CrdtReader::new(&data);
        assert_eq!(r.vu57(), 127);
    }

    #[test]
    fn vu57_128() {
        let data = encode_vu57(128);
        let mut r = CrdtReader::new(&data);
        assert_eq!(r.vu57(), 128);
    }

    #[test]
    fn vu57_session_max() {
        let n = 9_007_199_254_740_991u64;
        let data = encode_vu57(n);
        let mut r = CrdtReader::new(&data);
        assert_eq!(r.vu57(), n);
    }

    #[test]
    fn b1vu56_small_flag0() {
        let data = encode_b1vu56(0, 10);
        let mut r = CrdtReader::new(&data);
        let (f, v) = r.b1vu56();
        assert_eq!(f, 0);
        assert_eq!(v, 10);
    }

    #[test]
    fn b1vu56_small_flag1() {
        let data = encode_b1vu56(1, 10);
        let mut r = CrdtReader::new(&data);
        let (f, v) = r.b1vu56();
        assert_eq!(f, 1);
        assert_eq!(v, 10);
    }

    #[test]
    fn b1vu56_large() {
        let n = 9_007_199_254_740_991u64;
        let data = encode_b1vu56(0, n);
        let mut r = CrdtReader::new(&data);
        let (f, v) = r.b1vu56();
        assert_eq!(f, 0);
        assert_eq!(v, n);
    }

    #[test]
    fn id_single_byte() {
        use crate::json_crdt_patch::util::binary::CrdtWriter;
        let mut w = CrdtWriter::new();
        w.id(3, 7);  // x=3 (fits in 3 bits), y=7 (fits in 4 bits)
        let data = w.flush();
        assert_eq!(data.len(), 1);
        let mut r = CrdtReader::new(&data);
        let (x, y) = r.id();
        assert_eq!(x, 3);
        assert_eq!(y, 7);
    }

    #[test]
    fn id_multi_byte() {
        use crate::json_crdt_patch::util::binary::CrdtWriter;
        let mut w = CrdtWriter::new();
        w.id(10, 100);  // x > 7, needs multi-byte
        let data = w.flush();
        assert!(data.len() > 1);
        let mut r = CrdtReader::new(&data);
        let (x, y) = r.id();
        assert_eq!(x, 10);
        assert_eq!(y, 100);
    }
}
