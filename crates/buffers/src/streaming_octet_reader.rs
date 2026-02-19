//! Streaming octet reader for reading across chunk boundaries.

/// A streaming reader that manages multiple chunks of byte slices.
///
/// For performance, it does not merge chunks into a single buffer.
/// Instead, it tracks chunks and reads across boundaries as needed.
pub struct StreamingOctetReader {
    chunks: Vec<Vec<u8>>,
    /// Current position within the current chunk.
    x: usize,
    /// Total size of all chunks.
    chunk_size: usize,
}

impl Default for StreamingOctetReader {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingOctetReader {
    /// Creates a new streaming reader.
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            x: 0,
            chunk_size: 0,
        }
    }

    /// Returns the number of bytes remaining to be read.
    pub fn size(&self) -> usize {
        self.chunk_size - self.x
    }

    /// Adds a chunk of data to be read.
    pub fn push(&mut self, chunk: Vec<u8>) {
        self.chunk_size += chunk.len();
        self.chunks.push(chunk);
    }

    fn assert_size(&self, size: usize) {
        if size > self.size() {
            panic!("OUT_OF_BOUNDS");
        }
    }

    /// Reads a single unsigned byte.
    pub fn u8(&mut self) -> u8 {
        self.assert_size(1);
        let chunk = &mut self.chunks[0];
        let octet = chunk[self.x];
        self.x += 1;
        if self.x >= chunk.len() {
            self.chunk_size -= chunk.len();
            self.chunks.remove(0);
            self.x = 0;
        }
        octet
    }

    /// Reads an unsigned 32-bit integer (big-endian).
    pub fn u32(&mut self) -> u32 {
        let octet0 = self.u8() as u32;
        let octet1 = self.u8() as u32;
        let octet2 = self.u8() as u32;
        let octet3 = self.u8() as u32;
        (octet0 << 24) | (octet1 << 16) | (octet2 << 8) | octet3
    }

    /// Copies bytes to a destination buffer.
    pub fn copy_to(&mut self, size: usize, dst: &mut [u8], pos: usize) {
        if size == 0 {
            return;
        }
        self.assert_size(size);
        let mut remaining = size;
        let mut dst_pos = pos;
        let mut chunk_idx = 0;
        let mut local_x = self.x;

        while remaining > 0 {
            let chunk = &self.chunks[chunk_idx];
            let available = chunk.len() - local_x;
            let to_copy = available.min(remaining);
            dst[dst_pos..dst_pos + to_copy].copy_from_slice(&chunk[local_x..local_x + to_copy]);
            dst_pos += to_copy;
            remaining -= to_copy;

            if to_copy == available {
                // Move to next chunk
                chunk_idx += 1;
                local_x = 0;
            } else {
                local_x += to_copy;
            }
        }

        self.skip_unsafe(size);
    }

    /// Reads bytes into a new vector.
    pub fn buf(&mut self, size: usize) -> Vec<u8> {
        self.assert_size(size);
        let mut result = vec![0u8; size];
        self.copy_to(size, &mut result, 0);
        result
    }

    /// Skips bytes without reading them.
    pub fn skip(&mut self, n: usize) {
        self.assert_size(n);
        self.skip_unsafe(n);
    }

    fn skip_unsafe(&mut self, mut n: usize) {
        if n == 0 {
            return;
        }
        while n > 0 && !self.chunks.is_empty() {
            let chunk = &self.chunks[0];
            let remaining = chunk.len() - self.x;
            if remaining > n {
                self.x += n;
                return;
            }
            n -= remaining;
            self.chunk_size -= chunk.len();
            self.chunks.remove(0);
            self.x = 0;
        }
    }

    /// Peeks at the next byte without advancing.
    pub fn peek(&self) -> u8 {
        self.assert_size(1);
        self.chunks[0][self.x]
    }

    /// Reads `size` bytes, XOR-masking each byte with `mask[(offset + i) % 4]`.
    ///
    /// Used for WebSocket frame payload unmasking (RFC 6455 §5.3).
    pub fn buf_xor(&mut self, size: usize, mask: [u8; 4], offset: usize) -> Vec<u8> {
        let raw = self.buf(size);
        raw.into_iter()
            .enumerate()
            .map(|(i, b)| b ^ mask[(offset + i) % 4])
            .collect()
    }

    /// Copies `size` bytes to `dst[pos..]`, XOR-masking with `mask[(already_read + i) % 4]`.
    ///
    /// Used for streaming WebSocket frame data reads.
    pub fn copy_xor(
        &mut self,
        size: usize,
        dst: &mut [u8],
        pos: usize,
        mask: [u8; 4],
        already_read: usize,
    ) {
        let raw = self.buf(size);
        for (i, b) in raw.into_iter().enumerate() {
            dst[pos + i] = b ^ mask[(already_read + i) % 4];
        }
    }

    /// Reads `size` UTF-8 bytes, XOR-masking with `mask[(offset + i) % 4]`, and
    /// decodes to a `String`.
    pub fn utf8_masked(&mut self, size: usize, mask: [u8; 4], offset: usize) -> String {
        let raw = self.buf_xor(size, mask, offset);
        String::from_utf8(raw).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![1, 2, 3]);
        assert_eq!(reader.u8(), 1);
        assert_eq!(reader.u8(), 2);
        assert_eq!(reader.u8(), 3);
    }

    #[test]
    fn test_u8_across_chunks() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![1, 2]);
        reader.push(vec![3, 4]);
        assert_eq!(reader.u8(), 1);
        assert_eq!(reader.u8(), 2);
        assert_eq!(reader.u8(), 3);
        assert_eq!(reader.u8(), 4);
    }

    #[test]
    fn test_u32() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![0x01, 0x02, 0x03, 0x04]);
        assert_eq!(reader.u32(), 0x01020304);
    }

    #[test]
    fn test_peek() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![42, 43]);
        assert_eq!(reader.peek(), 42);
        assert_eq!(reader.u8(), 42);
        assert_eq!(reader.peek(), 43);
    }

    #[test]
    fn test_skip() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![1, 2, 3, 4, 5]);
        reader.skip(2);
        assert_eq!(reader.u8(), 3);
    }

    #[test]
    fn test_buf() {
        let mut reader = StreamingOctetReader::new();
        reader.push(vec![1, 2, 3, 4, 5]);
        let buf = reader.buf(3);
        assert_eq!(buf, vec![1, 2, 3]);
        assert_eq!(reader.u8(), 4);
    }
}
