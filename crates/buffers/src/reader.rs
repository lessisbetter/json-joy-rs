//! Binary buffer reader with cursor tracking.

use std::str;

use crate::BufferError;

/// A binary buffer reader that reads data from a byte slice.
///
/// The reader maintains a cursor position and provides methods for reading
/// various integer types and strings.
///
/// # Example
///
/// ```
/// use json_joy_buffers::Reader;
///
/// let data = [0x01, 0x02, 0x03, 0x04];
/// let mut reader = Reader::new(&data);
///
/// assert_eq!(reader.u8(), 0x01);
/// assert_eq!(reader.u16(), 0x0203);
/// ```
pub struct Reader<'a> {
    /// The underlying byte slice.
    pub uint8: &'a [u8],
    /// Current cursor position.
    pub x: usize,
    /// End position (exclusive).
    pub end: usize,
}

impl<'a> Reader<'a> {
    /// Creates a new reader for the given byte slice.
    pub fn new(uint8: &'a [u8]) -> Self {
        let end = uint8.len();
        Self { uint8, x: 0, end }
    }

    /// Creates a reader from a slice with custom start and end positions.
    pub fn from_slice(uint8: &'a [u8], x: usize, end: usize) -> Self {
        Self { uint8, x, end }
    }

    /// Resets the reader with a new byte slice.
    pub fn reset(&mut self, uint8: &'a [u8]) {
        self.x = 0;
        self.end = uint8.len();
        self.uint8 = uint8;
    }

    /// Returns the number of remaining bytes.
    pub fn size(&self) -> usize {
        self.end - self.x
    }

    /// Peeks at the current byte without advancing the cursor.
    pub fn peek(&self) -> u8 {
        self.uint8[self.x]
    }

    /// @deprecated Use peek() instead.
    pub fn peak(&self) -> u8 {
        self.peek()
    }

    /// Advances the cursor by the given number of bytes.
    pub fn skip(&mut self, length: usize) {
        self.x += length;
    }

    /// Returns a subarray of the given size and advances the cursor.
    pub fn buf(&mut self, size: usize) -> &'a [u8] {
        let x = self.x;
        let end = x + size;
        let bin = &self.uint8[x..end];
        self.x = end;
        bin
    }

    /// Returns a subarray without advancing the cursor.
    pub fn subarray(&self, start: usize, end: Option<usize>) -> &'a [u8] {
        let x = self.x;
        let actual_start = x + start;
        let actual_end = end.map(|e| x + e).unwrap_or(self.end);
        &self.uint8[actual_start..actual_end]
    }

    /// Creates a new Reader that references the same underlying memory.
    pub fn slice(&self, start: usize, end: Option<usize>) -> Reader<'a> {
        let x = self.x;
        let actual_start = x + start;
        let actual_end = end.map(|e| x + e).unwrap_or(self.end);
        Reader::from_slice(self.uint8, actual_start, actual_end)
    }

    /// Creates a new Reader from the current position and advances the cursor.
    pub fn cut(&mut self, size: usize) -> Reader<'a> {
        let slice = self.slice(0, Some(size));
        self.skip(size);
        slice
    }

    /// Reads an unsigned 8-bit integer.
    #[inline]
    pub fn u8(&mut self) -> u8 {
        let val = self.uint8[self.x];
        self.x += 1;
        val
    }

    /// Reads a signed 8-bit integer.
    #[inline]
    pub fn i8(&mut self) -> i8 {
        let val = self.uint8[self.x] as i8;
        self.x += 1;
        val
    }

    /// Reads an unsigned 16-bit integer (big-endian).
    #[inline]
    pub fn u16(&mut self) -> u16 {
        let x = self.x;
        let val = ((self.uint8[x] as u16) << 8) | (self.uint8[x + 1] as u16);
        self.x += 2;
        val
    }

    /// Reads a signed 16-bit integer (big-endian).
    #[inline]
    pub fn i16(&mut self) -> i16 {
        let val = i16::from_be_bytes([self.uint8[self.x], self.uint8[self.x + 1]]);
        self.x += 2;
        val
    }

    /// Reads an unsigned 32-bit integer (big-endian).
    #[inline]
    pub fn u32(&mut self) -> u32 {
        let val = u32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        val
    }

    /// Reads a signed 32-bit integer (big-endian).
    #[inline]
    pub fn i32(&mut self) -> i32 {
        let val = i32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        val
    }

    /// Reads an unsigned 64-bit integer (big-endian).
    #[inline]
    pub fn u64(&mut self) -> u64 {
        let val = u64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        val
    }

    /// Reads a signed 64-bit integer (big-endian).
    #[inline]
    pub fn i64(&mut self) -> i64 {
        let val = i64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        val
    }

    /// Reads a 32-bit floating point number (big-endian).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        let val = f32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        val
    }

    /// Reads a 64-bit floating point number (big-endian).
    #[inline]
    pub fn f64(&mut self) -> f64 {
        let val = f64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        val
    }

    /// Reads a UTF-8 string of the given size.
    pub fn utf8(&mut self, size: usize) -> &'a str {
        let start = self.x;
        self.x += size;
        str::from_utf8(&self.uint8[start..self.x]).unwrap_or("")
    }

    /// Reads an ASCII string of the given length.
    pub fn ascii(&mut self, length: usize) -> &'a str {
        let start = self.x;
        self.x += length;
        // ASCII is a subset of UTF-8, so this is safe
        str::from_utf8(&self.uint8[start..self.x]).unwrap_or("")
    }

    // -----------------------------------------------------------------------
    // Bounds-checked variants – return Result<T, BufferError::EndOfBuffer>
    // instead of panicking when reading past the end of the buffer.
    // -----------------------------------------------------------------------

    /// Checks that `n` more bytes are available from the current cursor.
    #[inline]
    fn check(&self, n: usize) -> Result<(), BufferError> {
        if self.x + n > self.uint8.len() {
            Err(BufferError::EndOfBuffer)
        } else {
            Ok(())
        }
    }

    /// Peeks at the current byte without advancing — returns an error when at
    /// or past the end of the buffer.
    pub fn try_peek(&self) -> Result<u8, BufferError> {
        self.check(1)?;
        Ok(self.uint8[self.x])
    }

    /// Reads an unsigned 8-bit integer, returning `Err` on out-of-bounds.
    #[inline]
    pub fn try_u8(&mut self) -> Result<u8, BufferError> {
        self.check(1)?;
        let val = self.uint8[self.x];
        self.x += 1;
        Ok(val)
    }

    /// Reads a signed 8-bit integer, returning `Err` on out-of-bounds.
    #[inline]
    pub fn try_i8(&mut self) -> Result<i8, BufferError> {
        self.check(1)?;
        let val = self.uint8[self.x] as i8;
        self.x += 1;
        Ok(val)
    }

    /// Reads an unsigned 16-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_u16(&mut self) -> Result<u16, BufferError> {
        self.check(2)?;
        let x = self.x;
        let val = ((self.uint8[x] as u16) << 8) | (self.uint8[x + 1] as u16);
        self.x += 2;
        Ok(val)
    }

    /// Reads a signed 16-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_i16(&mut self) -> Result<i16, BufferError> {
        self.check(2)?;
        let val = i16::from_be_bytes([self.uint8[self.x], self.uint8[self.x + 1]]);
        self.x += 2;
        Ok(val)
    }

    /// Reads an unsigned 32-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_u32(&mut self) -> Result<u32, BufferError> {
        self.check(4)?;
        let val = u32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        Ok(val)
    }

    /// Reads a signed 32-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_i32(&mut self) -> Result<i32, BufferError> {
        self.check(4)?;
        let val = i32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        Ok(val)
    }

    /// Reads an unsigned 64-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_u64(&mut self) -> Result<u64, BufferError> {
        self.check(8)?;
        let val = u64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        Ok(val)
    }

    /// Reads a signed 64-bit big-endian integer, returning `Err` on
    /// out-of-bounds.
    #[inline]
    pub fn try_i64(&mut self) -> Result<i64, BufferError> {
        self.check(8)?;
        let val = i64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        Ok(val)
    }

    /// Reads a 32-bit big-endian float, returning `Err` on out-of-bounds.
    #[inline]
    pub fn try_f32(&mut self) -> Result<f32, BufferError> {
        self.check(4)?;
        let val = f32::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
        ]);
        self.x += 4;
        Ok(val)
    }

    /// Reads a 64-bit big-endian float, returning `Err` on out-of-bounds.
    #[inline]
    pub fn try_f64(&mut self) -> Result<f64, BufferError> {
        self.check(8)?;
        let val = f64::from_be_bytes([
            self.uint8[self.x],
            self.uint8[self.x + 1],
            self.uint8[self.x + 2],
            self.uint8[self.x + 3],
            self.uint8[self.x + 4],
            self.uint8[self.x + 5],
            self.uint8[self.x + 6],
            self.uint8[self.x + 7],
        ]);
        self.x += 8;
        Ok(val)
    }

    /// Reads `size` raw bytes and advances the cursor, returning `Err` on
    /// out-of-bounds.
    pub fn try_buf(&mut self, size: usize) -> Result<&'a [u8], BufferError> {
        self.check(size)?;
        let x = self.x;
        let end = x + size;
        let bin = &self.uint8[x..end];
        self.x = end;
        Ok(bin)
    }

    /// Reads a UTF-8 string of `size` bytes, returning `Err` on out-of-bounds
    /// or invalid UTF-8.
    pub fn try_utf8(&mut self, size: usize) -> Result<&'a str, BufferError> {
        self.check(size)?;
        let start = self.x;
        self.x += size;
        str::from_utf8(&self.uint8[start..self.x]).map_err(|_| BufferError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8() {
        let data = [0x01, 0x02, 0x03];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.u8(), 0x01);
        assert_eq!(reader.u8(), 0x02);
        assert_eq!(reader.u8(), 0x03);
    }

    #[test]
    fn test_u16() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.u16(), 0x0102);
        assert_eq!(reader.u16(), 0x0304);
    }

    #[test]
    fn test_u32() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.u32(), 0x01020304);
    }

    #[test]
    fn test_skip() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut reader = Reader::new(&data);
        reader.skip(2);
        assert_eq!(reader.u8(), 0x03);
    }

    #[test]
    fn test_slice() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let reader = Reader::new(&data);
        let mut slice = reader.slice(1, Some(4));
        assert_eq!(slice.u8(), 0x02);
    }

    #[test]
    fn test_utf8() {
        let data = b"hello world";
        let mut reader = Reader::new(data);
        assert_eq!(reader.utf8(5), "hello");
        assert_eq!(reader.utf8(6), " world");
    }

    // ------------------------------------------------------------------
    // Bounds-checked try_* variants
    // ------------------------------------------------------------------

    #[test]
    fn test_try_u8_success() {
        let data = [0x42u8];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u8(), Ok(0x42));
        assert_eq!(reader.x, 1);
    }

    #[test]
    fn test_try_u8_end_of_buffer() {
        let data: [u8; 0] = [];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u8(), Err(BufferError::EndOfBuffer));
        // Cursor must not advance on error
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_i8_negative() {
        let data = [0xfeu8]; // -2 in two's complement
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_i8(), Ok(-2i8));
    }

    #[test]
    fn test_try_i8_end_of_buffer() {
        let data: [u8; 0] = [];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_i8(), Err(BufferError::EndOfBuffer));
    }

    #[test]
    fn test_try_u16_success() {
        let data = [0x01u8, 0x02];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u16(), Ok(0x0102u16));
        assert_eq!(reader.x, 2);
    }

    #[test]
    fn test_try_u16_partial() {
        let data = [0x01u8]; // only 1 byte — not enough for u16
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u16(), Err(BufferError::EndOfBuffer));
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_i16_negative() {
        // -1000 big-endian: 0xfc18
        let mut writer = crate::Writer::new();
        writer.i16(-1000i16);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_i16(), Ok(-1000i16));
    }

    #[test]
    fn test_try_u32_success() {
        let data = [0x01u8, 0x02, 0x03, 0x04];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u32(), Ok(0x01020304u32));
    }

    #[test]
    fn test_try_u32_end_of_buffer() {
        let data = [0x01u8, 0x02, 0x03]; // 3 bytes — not enough for u32
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u32(), Err(BufferError::EndOfBuffer));
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_i32_negative() {
        let mut writer = crate::Writer::new();
        writer.i32(-123456);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_i32(), Ok(-123456i32));
    }

    #[test]
    fn test_try_u64_success() {
        let mut writer = crate::Writer::new();
        writer.u64(0x0102030405060708u64);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u64(), Ok(0x0102030405060708u64));
    }

    #[test]
    fn test_try_u64_end_of_buffer() {
        let data = [0u8; 7]; // 7 bytes — not enough for u64
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_u64(), Err(BufferError::EndOfBuffer));
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_i64_negative() {
        let mut writer = crate::Writer::new();
        writer.i64(-9_999_999_999i64);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_i64(), Ok(-9_999_999_999i64));
    }

    #[test]
    fn test_try_f32_success() {
        let mut writer = crate::Writer::new();
        writer.f32(1.5f32);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        assert!((reader.try_f32().unwrap() - 1.5f32).abs() < 1e-6);
    }

    #[test]
    fn test_try_f32_end_of_buffer() {
        let data = [0u8; 3]; // 3 bytes — not enough for f32
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_f32(), Err(BufferError::EndOfBuffer));
    }

    #[test]
    fn test_try_f64_success() {
        let mut writer = crate::Writer::new();
        writer.f64(std::f64::consts::PI);
        let data = writer.flush();
        let mut reader = Reader::new(&data);
        let got = reader.try_f64().unwrap();
        assert!((got - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_try_f64_end_of_buffer() {
        let data = [0u8; 7]; // 7 bytes — not enough for f64
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_f64(), Err(BufferError::EndOfBuffer));
    }

    #[test]
    fn test_try_buf_success() {
        let data = [1u8, 2, 3, 4, 5];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_buf(3), Ok([1u8, 2, 3].as_ref()));
        assert_eq!(reader.x, 3);
    }

    #[test]
    fn test_try_buf_end_of_buffer() {
        let data = [1u8, 2];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_buf(5), Err(BufferError::EndOfBuffer));
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_utf8_success() {
        let data = b"hello";
        let mut reader = Reader::new(data);
        assert_eq!(reader.try_utf8(5), Ok("hello"));
    }

    #[test]
    fn test_try_utf8_end_of_buffer() {
        let data = b"hi";
        let mut reader = Reader::new(data);
        assert_eq!(reader.try_utf8(10), Err(BufferError::EndOfBuffer));
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_utf8_invalid() {
        // 0xff is not valid UTF-8
        let data = [0xffu8, 0xfe];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.try_utf8(2), Err(BufferError::InvalidUtf8));
    }

    #[test]
    fn test_try_peek_success() {
        let data = [0x55u8];
        let reader = Reader::new(&data);
        assert_eq!(reader.try_peek(), Ok(0x55));
        // cursor unchanged
        assert_eq!(reader.x, 0);
    }

    #[test]
    fn test_try_peek_end_of_buffer() {
        let data: [u8; 0] = [];
        let reader = Reader::new(&data);
        assert_eq!(reader.try_peek(), Err(BufferError::EndOfBuffer));
    }
}
