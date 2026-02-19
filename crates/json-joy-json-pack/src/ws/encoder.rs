//! WebSocket frame encoder (RFC 6455).
//!
//! Upstream reference: `json-pack/src/ws/WsFrameEncoder.ts`

use json_joy_buffers::Writer;

use super::constants::WsFrameOpcode;

/// WebSocket frame encoder.
///
/// Writes RFC 6455 frame headers and payloads into an internal [`Writer`].
/// Call `encode_*` methods to get bytes, or `write_*` methods to accumulate
/// data into the writer and flush manually.
pub struct WsFrameEncoder {
    pub writer: Writer,
}

impl Default for WsFrameEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl WsFrameEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    /// Encodes a Ping frame.
    pub fn encode_ping(&mut self, data: Option<&[u8]>) -> Vec<u8> {
        self.write_ping(data);
        self.writer.flush()
    }

    /// Encodes a Pong frame.
    pub fn encode_pong(&mut self, data: Option<&[u8]>) -> Vec<u8> {
        self.write_pong(data);
        self.writer.flush()
    }

    /// Encodes a Close frame.
    pub fn encode_close(&mut self, reason: &str, code: u16) -> Vec<u8> {
        self.write_close(reason, code);
        self.writer.flush()
    }

    /// Encodes just a frame header.
    pub fn encode_hdr(
        &mut self,
        fin: bool,
        opcode: WsFrameOpcode,
        length: usize,
        mask: u32,
    ) -> Vec<u8> {
        self.write_hdr(fin, opcode, length, mask);
        self.writer.flush()
    }

    /// Encodes a fast (unmasked binary) data message header.
    pub fn encode_data_msg_hdr_fast(&mut self, length: usize) -> Vec<u8> {
        self.write_data_msg_hdr_fast(length);
        self.writer.flush()
    }

    /// Writes a Ping frame into the internal writer.
    pub fn write_ping(&mut self, data: Option<&[u8]>) {
        match data {
            Some(d) if !d.is_empty() => {
                self.write_hdr(true, WsFrameOpcode::Ping, d.len(), 0);
                self.writer.buf(d);
            }
            _ => {
                self.write_hdr(true, WsFrameOpcode::Ping, 0, 0);
            }
        }
    }

    /// Writes a Pong frame into the internal writer.
    pub fn write_pong(&mut self, data: Option<&[u8]>) {
        match data {
            Some(d) if !d.is_empty() => {
                self.write_hdr(true, WsFrameOpcode::Pong, d.len(), 0);
                self.writer.buf(d);
            }
            _ => {
                self.write_hdr(true, WsFrameOpcode::Pong, 0, 0);
            }
        }
    }

    /// Writes a Close frame into the internal writer.
    ///
    /// If `code == 0` and `reason` is empty, writes a minimal close frame with
    /// no payload.
    pub fn write_close(&mut self, reason: &str, code: u16) {
        if code == 0 && reason.is_empty() {
            self.write_hdr(true, WsFrameOpcode::Close, 0, 0);
            return;
        }
        // Estimate: 2 bytes for code + up to 4 bytes per UTF-8 char
        let reason_bytes = reason.as_bytes();
        let utf8_len = reason_bytes.len();
        let payload_len = 2 + utf8_len;
        // Capture the position of the second header byte to patch length later
        // if UTF-8 len differs from char count (ASCII-only in practice).
        let writer = &mut self.writer;
        let length_byte_x = writer.x + 1;
        self.write_hdr(true, WsFrameOpcode::Close, payload_len, 0);
        self.writer.u16(code);
        if utf8_len > 0 {
            self.writer.buf(reason_bytes);
            // If actual UTF-8 length differs from estimated, patch the length byte.
            // RFC 6455 close frames must not exceed 125 bytes payload.
            let actual_len = 2 + utf8_len;
            if actual_len != payload_len {
                let mask_bit = self.writer.uint8[length_byte_x] & 0x80;
                self.writer.uint8[length_byte_x] = mask_bit | (actual_len as u8 & 0x7f);
            }
        }
    }

    /// Writes a WebSocket frame header into the internal writer.
    ///
    /// - `length < 126`: 2-byte header
    /// - `length < 65536`: 4-byte header with 16-bit extended length
    /// - otherwise: 10-byte header with 64-bit extended length (upper 32 bits = 0)
    ///
    /// If `mask != 0`, appends the 4-byte masking key.
    pub fn write_hdr(&mut self, fin: bool, opcode: WsFrameOpcode, length: usize, mask: u32) {
        let octet1 = ((fin as u8) << 7) | (opcode as u8);
        let mask_bit: u8 = if mask != 0 { 0x80 } else { 0x00 };
        let writer = &mut self.writer;
        if length < 126 {
            let octet2 = mask_bit | length as u8;
            writer.u16(((octet1 as u16) << 8) | octet2 as u16);
        } else if length < 0x1_0000 {
            let octet2 = mask_bit | 126;
            // Write 2-byte header + 2-byte extended length as a single u32
            let val = (((octet1 as u32) << 8 | octet2 as u32) << 16) | length as u32;
            writer.u32(val);
        } else {
            let octet2 = mask_bit | 127;
            writer.u16(((octet1 as u16) << 8) | octet2 as u16);
            writer.u32(0); // upper 32 bits of 64-bit length (always 0 in practice)
            writer.u32(length as u32);
        }
        if mask != 0 {
            writer.u32(mask);
        }
    }

    /// Writes a fast (fin=1, opcode=BINARY, no mask) data frame header.
    pub fn write_data_msg_hdr_fast(&mut self, length: usize) {
        let writer = &mut self.writer;
        if length < 126 {
            // 0b10000010_00000000 | length
            writer.u16(0b10000010_0000_0000 | length as u16);
        } else if length < 0x1_0000 {
            // 0b10000010_01111110_<length_hi>_<length_lo>
            writer.u32(0b10000010_0111_1110_0000_0000_0000_0000 | length as u32);
        } else {
            writer.u16(0b10000010_0111_1111);
            writer.u32(0);
            writer.u32(length as u32);
        }
    }

    /// Writes `buf` XOR-masked with `mask` (big-endian 32-bit masking key).
    pub fn write_buf_xor(&mut self, buf: &[u8], mask: u32) {
        let mask_bytes = mask.to_be_bytes();
        let writer = &mut self.writer;
        writer.ensure_capacity(buf.len());
        for (i, &b) in buf.iter().enumerate() {
            writer.uint8[writer.x + i] = b ^ mask_bytes[i & 3];
        }
        writer.x += buf.len();
    }
}
