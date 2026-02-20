//! WebSocket frame decoder (RFC 6455).
//!
//! Upstream reference: `json-pack/src/ws/WsFrameDecoder.ts`

use json_joy_buffers::StreamingOctetReader;

use super::constants::WsFrameOpcode;
use super::frames::{WsCloseFrame, WsFrame, WsFrameHeader, WsPingFrame, WsPongFrame};

/// Error type for WebSocket frame decoding failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WsFrameDecodingError {
    #[error("invalid WebSocket frame")]
    InvalidFrame,
}

/// Streaming WebSocket frame decoder.
///
/// Feed bytes via [`push`] and call [`read_frame_header`] to receive parsed
/// frame headers. Data frames are returned as [`WsFrame::Data`]; control
/// frames (Ping/Pong/Close) include their payloads.
pub struct WsFrameDecoder {
    pub reader: StreamingOctetReader,
}

impl Default for WsFrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl WsFrameDecoder {
    pub fn new() -> Self {
        Self {
            reader: StreamingOctetReader::new(),
        }
    }

    /// Pushes a chunk of bytes into the internal buffer.
    pub fn push(&mut self, data: Vec<u8>) {
        self.reader.push(data);
    }

    /// Attempts to read a complete WebSocket frame header from the buffer.
    ///
    /// Returns `None` if not enough data is available yet.
    /// Returns `Err` if the frame is malformed.
    ///
    /// For data frames (`Text`/`Binary`/`Continue`) the frame payload is NOT
    /// consumed here — use [`read_frame_data`] or [`copy_frame_data`] next.
    ///
    /// For control frames (`Ping`/`Pong`/`Close`) the payload is read
    /// immediately and included in the returned frame.
    pub fn read_frame_header(&mut self) -> Result<Option<WsFrame>, WsFrameDecodingError> {
        if self.reader.size() < 2 {
            return Ok(None);
        }
        // Attempt to parse; catch under-flow (size check returns Ok(None)).
        match self.try_read_frame_header() {
            Ok(v) => Ok(v),
            Err(WsFrameDecodingError::InvalidFrame) => Err(WsFrameDecodingError::InvalidFrame),
        }
    }

    fn try_read_frame_header(&mut self) -> Result<Option<WsFrame>, WsFrameDecodingError> {
        let b0 = self.reader.u8();
        let b1 = self.reader.u8();

        let fin = (b0 >> 7) != 0;
        let opcode = b0 & 0x0f;
        let mask_bit = b1 >> 7;
        let mut length = (b1 & 0x7f) as usize;

        if length == 126 {
            if self.reader.size() < 2 {
                return Ok(None);
            }
            let hi = self.reader.u8() as usize;
            let lo = self.reader.u8() as usize;
            length = (hi << 8) | lo;
        } else if length == 127 {
            if self.reader.size() < 8 {
                return Ok(None);
            }
            // Skip upper 4 bytes (always 0 for reasonable frame sizes)
            self.reader.skip(4);
            length = self.reader.u32() as usize;
        }

        let mask: Option<[u8; 4]> = if mask_bit != 0 {
            if self.reader.size() < 4 {
                return Ok(None);
            }
            Some([
                self.reader.u8(),
                self.reader.u8(),
                self.reader.u8(),
                self.reader.u8(),
            ])
        } else {
            None
        };

        let header = WsFrameHeader::new(fin, opcode, length, mask);

        if opcode >= WsFrameOpcode::MIN_CONTROL_OPCODE {
            match opcode {
                8 /* CLOSE */ => {
                    return Ok(Some(WsFrame::Close(WsCloseFrame {
                        header,
                        code: 0,
                        reason: String::new(),
                    })));
                }
                9 /* PING */ => {
                    if length > 125 {
                        return Err(WsFrameDecodingError::InvalidFrame);
                    }
                    if self.reader.size() < length {
                        return Ok(None);
                    }
                    let data = match mask {
                        Some(m) => self.reader.buf_xor(length, m, 0),
                        None => self.reader.buf(length),
                    };
                    return Ok(Some(WsFrame::Ping(WsPingFrame { header, data })));
                }
                10 /* PONG */ => {
                    if length > 125 {
                        return Err(WsFrameDecodingError::InvalidFrame);
                    }
                    if self.reader.size() < length {
                        return Ok(None);
                    }
                    let data = match mask {
                        Some(m) => self.reader.buf_xor(length, m, 0),
                        None => self.reader.buf(length),
                    };
                    return Ok(Some(WsFrame::Pong(WsPongFrame { header, data })));
                }
                _ => return Err(WsFrameDecodingError::InvalidFrame),
            }
        }

        Ok(Some(WsFrame::Data(header)))
    }

    /// Reads up to `remaining` bytes of data frame payload into `dst[pos..]`.
    ///
    /// Returns the number of bytes still remaining to be read.
    pub fn read_frame_data(
        &mut self,
        frame: &WsFrameHeader,
        remaining: usize,
        dst: &mut [u8],
        pos: usize,
    ) -> usize {
        let read_size = self.reader.size().min(remaining);
        let already_read = frame.length - remaining;
        match frame.mask {
            None => self.reader.copy_to(read_size, dst, pos),
            Some(mask) => self
                .reader
                .copy_xor(read_size, dst, pos, mask, already_read),
        }
        remaining - read_size
    }

    /// Reads all remaining payload bytes of a data frame into `dst[pos..]`.
    pub fn copy_frame_data(&mut self, frame: &WsFrameHeader, dst: &mut [u8], pos: usize) {
        let read_size = frame.length;
        match frame.mask {
            None => self.reader.copy_to(read_size, dst, pos),
            Some(mask) => self.reader.copy_xor(read_size, dst, pos, mask, 0),
        }
    }

    /// Reads and populates the payload of a Close frame.
    ///
    /// Updates `frame.code` and `frame.reason` in place.
    pub fn read_close_frame_data(
        &mut self,
        frame: &mut WsCloseFrame,
    ) -> Result<(), WsFrameDecodingError> {
        let length = frame.header.length;
        if length > 125 {
            return Err(WsFrameDecodingError::InvalidFrame);
        }
        if length == 0 {
            return Ok(());
        }
        if length < 2 {
            return Err(WsFrameDecodingError::InvalidFrame);
        }
        let mask = frame.header.mask;
        let b0 = self.reader.u8() ^ mask.map(|m| m[0]).unwrap_or(0);
        let b1 = self.reader.u8() ^ mask.map(|m| m[1]).unwrap_or(0);
        let code = ((b0 as u16) << 8) | b1 as u16;
        frame.code = code;
        let reason_len = length - 2;
        if reason_len > 0 {
            frame.reason = match mask {
                Some(m) => self.reader.utf8_masked(reason_len, m, 2),
                None => {
                    let bytes = self.reader.buf(reason_len);
                    String::from_utf8(bytes).unwrap_or_default()
                }
            };
        }
        Ok(())
    }
}
