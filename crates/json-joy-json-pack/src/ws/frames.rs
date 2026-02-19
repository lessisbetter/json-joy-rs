//! WebSocket frame header and control frame structs.
//!
//! Upstream reference: `json-pack/src/ws/frames.ts`

/// WebSocket frame header (RFC 6455 §5.2).
#[derive(Debug, Clone)]
pub struct WsFrameHeader {
    pub fin: bool,
    pub opcode: u8,
    pub length: usize,
    /// Masking key, if the mask bit was set.
    pub mask: Option<[u8; 4]>,
}

impl WsFrameHeader {
    pub fn new(fin: bool, opcode: u8, length: usize, mask: Option<[u8; 4]>) -> Self {
        Self {
            fin,
            opcode,
            length,
            mask,
        }
    }
}

/// Parsed WebSocket Ping control frame (includes payload data).
#[derive(Debug, Clone)]
pub struct WsPingFrame {
    pub header: WsFrameHeader,
    pub data: Vec<u8>,
}

/// Parsed WebSocket Pong control frame (includes payload data).
#[derive(Debug, Clone)]
pub struct WsPongFrame {
    pub header: WsFrameHeader,
    pub data: Vec<u8>,
}

/// Parsed WebSocket Close control frame (includes close code and reason).
#[derive(Debug, Clone)]
pub struct WsCloseFrame {
    pub header: WsFrameHeader,
    pub code: u16,
    pub reason: String,
}

/// A fully parsed WebSocket frame.
#[derive(Debug, Clone)]
pub enum WsFrame {
    Data(WsFrameHeader),
    Ping(WsPingFrame),
    Pong(WsPongFrame),
    Close(WsCloseFrame),
}

impl WsFrame {
    pub fn header(&self) -> &WsFrameHeader {
        match self {
            WsFrame::Data(h) => h,
            WsFrame::Ping(f) => &f.header,
            WsFrame::Pong(f) => &f.header,
            WsFrame::Close(f) => &f.header,
        }
    }
}
