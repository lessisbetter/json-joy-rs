//! Record Marshalling (RM) frame encoder.
//!
//! Upstream reference: `json-pack/src/rm/RmRecordEncoder.ts`

use json_joy_buffers::Writer;

const MAX_SINGLE_FRAME_SIZE: u32 = 0x7fff_ffff;

/// Record Marshalling frame encoder.
///
/// RM is a simple framing format: 4-byte header (1 fin bit + 31-bit payload length)
/// followed by the payload bytes. Large records are fragmented automatically.
pub struct RmRecordEncoder {
    pub writer: Writer,
}

impl Default for RmRecordEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RmRecordEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    /// Encodes just a frame header and returns the bytes.
    pub fn encode_hdr(&mut self, fin: bool, length: u32) -> Vec<u8> {
        self.write_hdr(fin, length);
        self.writer.flush()
    }

    /// Encodes a complete record (header + payload) and returns the bytes.
    pub fn encode_record(&mut self, record: &[u8]) -> Vec<u8> {
        self.write_record(record);
        self.writer.flush()
    }

    /// Writes a frame header into the internal writer.
    pub fn write_hdr(&mut self, fin: bool, length: u32) {
        let header: u32 = if fin { 0x8000_0000 | length } else { length };
        self.writer.u32(header);
    }

    /// Writes a complete record into the internal writer.
    ///
    /// If the record fits in a single frame it is written as-is. Otherwise it
    /// is split into MAX_SINGLE_FRAME_SIZE chunks, the last having fin=1.
    pub fn write_record(&mut self, record: &[u8]) {
        let length = record.len();
        if length <= MAX_SINGLE_FRAME_SIZE as usize {
            self.write_hdr(true, length as u32);
            self.writer.buf(record);
            return;
        }
        let mut offset = 0;
        while offset < length {
            let fragment_len = ((length - offset) as u32).min(MAX_SINGLE_FRAME_SIZE) as usize;
            let fin = offset + fragment_len >= length;
            self.write_fragment(record, offset, fragment_len, fin);
            offset += fragment_len;
        }
    }

    /// Writes a single fragment of a record.
    pub fn write_fragment(&mut self, record: &[u8], offset: usize, length: usize, fin: bool) {
        self.write_hdr(fin, length as u32);
        self.writer.buf(&record[offset..offset + length]);
    }

    /// Reserves space for an RM header and returns the position to be passed
    /// to [`end_record`].
    ///
    /// Use this to write a record in one pass when the payload length is not
    /// yet known.
    pub fn start_record(&mut self) -> usize {
        let pos = self.writer.x;
        self.writer.move_cursor(4);
        pos
    }

    /// Finalises the RM header reserved by [`start_record`].
    ///
    /// If the data written after `start_record` fits in a single frame the
    /// header is written in place. Otherwise the data is moved and written as
    /// multiple RM frames.
    pub fn end_record(&mut self, rm_header_position: usize) {
        let total_size = self.writer.x - rm_header_position - 4;
        if total_size <= MAX_SINGLE_FRAME_SIZE as usize {
            let current_x = self.writer.x;
            self.writer.x = rm_header_position;
            self.write_hdr(true, total_size as u32);
            self.writer.x = current_x;
        } else {
            let current_x = self.writer.x;
            let data_start = rm_header_position + 4;
            let data: Vec<u8> = self.writer.uint8[data_start..current_x].to_vec();
            self.writer.x = rm_header_position;
            self.writer.x0 = rm_header_position;
            self.write_record(&data);
        }
    }
}
