//! ONC RPC message decoder.
//!
//! Upstream reference: `json-pack/src/rpc/RpcMessageDecoder.ts`

use super::constants::{RpcAcceptStat, RpcAuthFlavor, RpcAuthStat, RpcRejectStat};
use super::messages::{
    RpcAcceptedReplyMessage, RpcCallMessage, RpcMessage, RpcMismatchInfo, RpcOpaqueAuth,
    RpcRejectedReplyMessage,
};

/// RPC decoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpcDecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("invalid message type: {0}")]
    InvalidMessageType(u32),
    #[error("invalid reply stat: {0}")]
    InvalidReplyStat(u32),
    #[error("invalid accept stat: {0}")]
    InvalidAcceptStat(u32),
    #[error("invalid reject stat: {0}")]
    InvalidRejectStat(u32),
    #[error("invalid auth flavor: {0}")]
    InvalidAuthFlavor(u32),
    #[error("invalid auth stat: {0}")]
    InvalidAuthStat(u32),
    #[error("auth body too large: {0} bytes")]
    AuthBodyTooLarge(u32),
}

/// ONC RPC message decoder.
///
/// Stateless — `decode_message` takes a data slice and creates internal state.
pub struct RpcMessageDecoder;

impl Default for RpcMessageDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcMessageDecoder {
    pub fn new() -> Self {
        Self
    }

    /// Decodes an RPC message from `data`.
    ///
    /// Returns `Ok(None)` if there is insufficient data (instead of an error).
    pub fn decode_message(&self, data: &[u8]) -> Result<Option<RpcMessage>, RpcDecodeError> {
        let mut r = SliceReader::new(data);
        match r.read_message() {
            Ok(msg) => Ok(Some(msg)),
            Err(RpcDecodeError::EndOfInput) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------- internal reader

struct SliceReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SliceReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn u32(&mut self) -> Result<u32, RpcDecodeError> {
        if self.pos + 4 > self.data.len() {
            return Err(RpcDecodeError::EndOfInput);
        }
        let b = &self.data[self.pos..self.pos + 4];
        let val = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        self.pos += 4;
        Ok(val)
    }

    fn buf(&mut self, n: usize) -> Result<Vec<u8>, RpcDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(RpcDecodeError::EndOfInput);
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }

    fn skip(&mut self, n: usize) -> Result<(), RpcDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(RpcDecodeError::EndOfInput);
        }
        self.pos += n;
        Ok(())
    }

    fn rest(&mut self) -> Vec<u8> {
        let bytes = self.data[self.pos..].to_vec();
        self.pos = self.data.len();
        bytes
    }

    fn read_opaque_auth(&mut self) -> Result<RpcOpaqueAuth, RpcDecodeError> {
        let flavor_u32 = self.u32()?;
        let flavor =
            RpcAuthFlavor::try_from(flavor_u32).map_err(RpcDecodeError::InvalidAuthFlavor)?;
        let len = self.u32()?;
        if len > 400 {
            return Err(RpcDecodeError::AuthBodyTooLarge(len));
        }
        let padded_len = (len + 3) & !3;
        let body = self.buf(len as usize)?;
        self.skip((padded_len - len) as usize)?;
        Ok(RpcOpaqueAuth { flavor, body })
    }

    fn read_mismatch_info(&mut self) -> Result<RpcMismatchInfo, RpcDecodeError> {
        Ok(RpcMismatchInfo {
            low: self.u32()?,
            high: self.u32()?,
        })
    }

    fn read_message(&mut self) -> Result<RpcMessage, RpcDecodeError> {
        let xid = self.u32()?;
        let msg_type = self.u32()?;
        match msg_type {
            0 => {
                // CALL
                let rpcvers = self.u32()?;
                let prog = self.u32()?;
                let vers = self.u32()?;
                let proc_ = self.u32()?;
                let cred = self.read_opaque_auth()?;
                let verf = self.read_opaque_auth()?;
                let params = if self.remaining() > 0 {
                    self.rest()
                } else {
                    Vec::new()
                };
                Ok(RpcMessage::Call(RpcCallMessage {
                    xid,
                    rpcvers,
                    prog,
                    vers,
                    proc_,
                    cred,
                    verf,
                    params,
                }))
            }
            1 => {
                // REPLY
                let reply_stat = self.u32()?;
                match reply_stat {
                    0 => {
                        // MSG_ACCEPTED
                        let verf = self.read_opaque_auth()?;
                        let stat_u32 = self.u32()?;
                        let stat = RpcAcceptStat::try_from(stat_u32)
                            .map_err(RpcDecodeError::InvalidAcceptStat)?;
                        let mismatch_info = if stat == RpcAcceptStat::ProgMismatch {
                            Some(self.read_mismatch_info()?)
                        } else {
                            None
                        };
                        let results = if self.remaining() > 0 {
                            Some(self.rest())
                        } else {
                            None
                        };
                        Ok(RpcMessage::AcceptedReply(RpcAcceptedReplyMessage {
                            xid,
                            verf,
                            stat,
                            mismatch_info,
                            results,
                        }))
                    }
                    1 => {
                        // MSG_DENIED
                        let stat_u32 = self.u32()?;
                        let stat = RpcRejectStat::try_from(stat_u32)
                            .map_err(RpcDecodeError::InvalidRejectStat)?;
                        let mismatch_info = if stat == RpcRejectStat::RpcMismatch {
                            Some(self.read_mismatch_info()?)
                        } else {
                            None
                        };
                        let auth_stat = if stat == RpcRejectStat::AuthError {
                            let s_u32 = self.u32()?;
                            let s = RpcAuthStat::try_from(s_u32)
                                .map_err(RpcDecodeError::InvalidAuthStat)?;
                            Some(s)
                        } else {
                            None
                        };
                        Ok(RpcMessage::RejectedReply(RpcRejectedReplyMessage {
                            xid,
                            stat,
                            mismatch_info,
                            auth_stat,
                        }))
                    }
                    other => Err(RpcDecodeError::InvalidReplyStat(other)),
                }
            }
            other => Err(RpcDecodeError::InvalidMessageType(other)),
        }
    }
}
