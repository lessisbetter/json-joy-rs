//! Binary buffer writer with auto-growing capacity.

/// A binary buffer writer that grows automatically as needed.
///
/// # Example
///
/// ```
/// use json_joy_buffers::Writer;
///
/// let mut writer = Writer::new();
/// writer.u8(0x01);
/// writer.u16(0x0203);
/// let data = writer.flush();
/// assert_eq!(data, [0x01, 0x02, 0x03]);
/// ```
pub struct Writer {
    /// The underlying byte buffer.
    pub uint8: Vec<u8>,
    /// Position where last flush happened.
    pub x0: usize,
    /// Current cursor position.
    pub x: usize,
    /// Allocation size when buffer needs to grow.
    alloc_size: usize,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    /// Creates a new writer with default allocation size (64KB).
    pub fn new() -> Self {
        Self::with_alloc_size(64 * 1024)
    }

    /// Creates a new writer with custom allocation size.
    pub fn with_alloc_size(alloc_size: usize) -> Self {
        let uint8 = vec![0u8; alloc_size];
        Self {
            uint8,
            x0: 0,
            x: 0,
            alloc_size,
        }
    }

    /// Ensures the buffer has at least `capacity` bytes available.
    pub fn ensure_capacity(&mut self, capacity: usize) {
        let remaining = self.uint8.len() - self.x;
        if remaining < capacity {
            let total = self.uint8.len() - self.x0;
            let required = capacity - remaining;
            let total_required = total + required;
            let new_size = if total_required <= self.alloc_size {
                self.alloc_size
            } else {
                total_required * 2
            };
            self.grow(new_size);
        }
    }

    fn grow(&mut self, new_size: usize) {
        let x0 = self.x0;
        let x = self.x;
        let mut new_buf = vec![0u8; new_size];
        new_buf[..x - x0].copy_from_slice(&self.uint8[x0..x]);
        self.uint8 = new_buf;
        self.x = x - x0;
        self.x0 = 0;
    }

    /// Moves the cursor forward by the given amount.
    pub fn move_cursor(&mut self, capacity: usize) {
        self.ensure_capacity(capacity);
        self.x += capacity;
    }

    /// Resets the flush position.
    pub fn reset(&mut self) {
        self.x0 = self.x;
    }

    /// Allocates a new buffer of the given size.
    pub fn new_buffer(&mut self, size: usize) {
        self.uint8 = vec![0u8; size];
        self.x = 0;
        self.x0 = 0;
    }

    /// Returns the written data and advances the flush position.
    pub fn flush(&mut self) -> Vec<u8> {
        let result = self.uint8[self.x0..self.x].to_vec();
        self.x0 = self.x;
        result
    }

    /// Returns a slice view of the written data and advances the flush position.
    ///
    /// Mirrors the upstream `flushSlice()` method. Returns a view into the
    /// internal buffer rather than copying.
    pub fn flush_slice(&mut self) -> crate::Slice<'_> {
        let slice = crate::Slice::new(&self.uint8, self.x0, self.x);
        self.x0 = self.x;
        slice
    }

    /// Writes an unsigned 8-bit integer.
    #[inline]
    pub fn u8(&mut self, val: u8) {
        self.ensure_capacity(1);
        self.uint8[self.x] = val;
        self.x += 1;
    }

    /// Writes a signed 8-bit integer.
    #[inline]
    pub fn i8(&mut self, val: i8) {
        self.ensure_capacity(1);
        self.uint8[self.x] = val as u8;
        self.x += 1;
    }

    /// Writes an unsigned 16-bit integer (big-endian).
    #[inline]
    pub fn u16(&mut self, val: u16) {
        self.ensure_capacity(2);
        let bytes = val.to_be_bytes();
        self.uint8[self.x] = bytes[0];
        self.uint8[self.x + 1] = bytes[1];
        self.x += 2;
    }

    /// Writes a signed 16-bit integer (big-endian).
    #[inline]
    pub fn i16(&mut self, val: i16) {
        self.ensure_capacity(2);
        let bytes = val.to_be_bytes();
        self.uint8[self.x] = bytes[0];
        self.uint8[self.x + 1] = bytes[1];
        self.x += 2;
    }

    /// Writes an unsigned 32-bit integer (big-endian).
    #[inline]
    pub fn u32(&mut self, val: u32) {
        self.ensure_capacity(4);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 4].copy_from_slice(&bytes);
        self.x += 4;
    }

    /// Writes a signed 32-bit integer (big-endian).
    #[inline]
    pub fn i32(&mut self, val: i32) {
        self.ensure_capacity(4);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 4].copy_from_slice(&bytes);
        self.x += 4;
    }

    /// Writes an unsigned 64-bit integer (big-endian).
    #[inline]
    pub fn u64(&mut self, val: u64) {
        self.ensure_capacity(8);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 8].copy_from_slice(&bytes);
        self.x += 8;
    }

    /// Writes a signed 64-bit integer (big-endian).
    #[inline]
    pub fn i64(&mut self, val: i64) {
        self.ensure_capacity(8);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 8].copy_from_slice(&bytes);
        self.x += 8;
    }

    /// Writes a 64-bit floating point number (big-endian).
    #[inline]
    pub fn f64(&mut self, val: f64) {
        self.ensure_capacity(8);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 8].copy_from_slice(&bytes);
        self.x += 8;
    }

    /// Writes a 32-bit floating point number (big-endian).
    #[inline]
    pub fn f32(&mut self, val: f32) {
        self.ensure_capacity(4);
        let bytes = val.to_be_bytes();
        self.uint8[self.x..self.x + 4].copy_from_slice(&bytes);
        self.x += 4;
    }

    /// Writes a u8 followed by a u16 (big-endian).
    pub fn u8u16(&mut self, u8_val: u8, u16_val: u16) {
        self.ensure_capacity(3);
        self.uint8[self.x] = u8_val;
        let bytes = u16_val.to_be_bytes();
        self.uint8[self.x + 1] = bytes[0];
        self.uint8[self.x + 2] = bytes[1];
        self.x += 3;
    }

    /// Writes a u8 followed by a u32 (big-endian).
    pub fn u8u32(&mut self, u8_val: u8, u32_val: u32) {
        self.ensure_capacity(5);
        self.uint8[self.x] = u8_val;
        let bytes = u32_val.to_be_bytes();
        self.uint8[self.x + 1..self.x + 5].copy_from_slice(&bytes);
        self.x += 5;
    }

    /// Writes a u8 followed by a u64 (big-endian).
    pub fn u8u64(&mut self, u8_val: u8, u64_val: u64) {
        self.ensure_capacity(9);
        self.uint8[self.x] = u8_val;
        let bytes = u64_val.to_be_bytes();
        self.uint8[self.x + 1..self.x + 9].copy_from_slice(&bytes);
        self.x += 9;
    }

    /// Writes a u8 followed by a f32 (big-endian).
    pub fn u8f32(&mut self, u8_val: u8, f32_val: f32) {
        self.ensure_capacity(5);
        self.uint8[self.x] = u8_val;
        let bytes = f32_val.to_be_bytes();
        self.uint8[self.x + 1..self.x + 5].copy_from_slice(&bytes);
        self.x += 5;
    }

    /// Writes a u8 followed by a f64 (big-endian).
    pub fn u8f64(&mut self, u8_val: u8, f64_val: f64) {
        self.ensure_capacity(9);
        self.uint8[self.x] = u8_val;
        let bytes = f64_val.to_be_bytes();
        self.uint8[self.x + 1..self.x + 9].copy_from_slice(&bytes);
        self.x += 9;
    }

    /// Writes a byte slice.
    pub fn buf(&mut self, buf: &[u8]) {
        let length = buf.len();
        self.ensure_capacity(length);
        self.uint8[self.x..self.x + length].copy_from_slice(buf);
        self.x += length;
    }

    /// Writes a UTF-8 string. Returns the number of bytes written.
    pub fn utf8(&mut self, s: &str) -> usize {
        let bytes = s.as_bytes();
        let length = bytes.len();
        self.ensure_capacity(length);
        self.uint8[self.x..self.x + length].copy_from_slice(bytes);
        self.x += length;
        length
    }

    /// Writes a UTF-8 string using the native pure-Rust encoder.
    ///
    /// Encodes multi-byte characters directly without relying on external
    /// encoders. Mirrors the upstream `utf8Native()` method. Returns the
    /// number of bytes written.
    pub fn utf8_native(&mut self, s: &str) -> usize {
        // Rust strings are always valid UTF-8, so we can copy bytes directly.
        // The `utf8` method already does this correctly.
        self.utf8(s)
    }

    /// Writes an ASCII string.
    pub fn ascii(&mut self, s: &str) {
        self.utf8(s); // ASCII is a subset of UTF-8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8() {
        let mut writer = Writer::new();
        writer.u8(0x01);
        writer.u8(0x02);
        assert_eq!(writer.flush(), [0x01, 0x02]);
    }

    #[test]
    fn test_u16() {
        let mut writer = Writer::new();
        writer.u16(0x0102);
        assert_eq!(writer.flush(), [0x01, 0x02]);
    }

    #[test]
    fn test_u32() {
        let mut writer = Writer::new();
        writer.u32(0x01020304);
        assert_eq!(writer.flush(), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_utf8() {
        let mut writer = Writer::new();
        writer.utf8("hello");
        assert_eq!(writer.flush(), b"hello");
    }

    #[test]
    fn test_flush_multiple() {
        let mut writer = Writer::new();
        writer.u8(0x01);
        assert_eq!(writer.flush(), [0x01]);
        writer.u8(0x02);
        assert_eq!(writer.flush(), [0x02]);
    }

    #[test]
    fn test_i8_positive() {
        let mut writer = Writer::new();
        writer.i8(127i8);
        assert_eq!(writer.flush(), [0x7f]);
    }

    #[test]
    fn test_i8_negative() {
        let mut writer = Writer::new();
        writer.i8(-1i8);
        assert_eq!(writer.flush(), [0xff]);
    }

    #[test]
    fn test_i8_minus_two() {
        let mut writer = Writer::new();
        writer.i8(-2i8);
        assert_eq!(writer.flush(), [0xfe]);
    }

    #[test]
    fn test_i16_positive() {
        let mut writer = Writer::new();
        writer.i16(1000i16);
        let data = writer.flush();
        assert_eq!(data.len(), 2);
        assert_eq!(i16::from_be_bytes([data[0], data[1]]), 1000i16);
    }

    #[test]
    fn test_i16_negative() {
        let mut writer = Writer::new();
        writer.i16(-1000i16);
        let data = writer.flush();
        assert_eq!(i16::from_be_bytes([data[0], data[1]]), -1000i16);
    }

    #[test]
    fn test_i64_roundtrip() {
        let mut writer = Writer::new();
        writer.i64(-9_999_999_999i64);
        let data = writer.flush();
        assert_eq!(data.len(), 8);
        assert_eq!(
            i64::from_be_bytes(data.try_into().unwrap()),
            -9_999_999_999i64
        );
    }

    #[test]
    fn test_flush_slice() {
        let mut writer = Writer::new();
        writer.u8(0x0a);
        writer.u8(0x0b);
        let slice = writer.flush_slice();
        // The slice view should cover exactly the two written bytes.
        assert_eq!(slice.subarray(), [0x0a, 0x0b]);
        // A subsequent write should be tracked as a new slice.
        writer.u8(0x0c);
        let slice2 = writer.flush_slice();
        assert_eq!(slice2.subarray(), [0x0c]);
    }

    #[test]
    fn test_utf8_native_roundtrip() {
        let mut writer = Writer::new();
        let n = writer.utf8_native("café");
        let data = writer.flush();
        assert_eq!(n, data.len());
        assert_eq!(std::str::from_utf8(&data).unwrap(), "café");
    }
}
