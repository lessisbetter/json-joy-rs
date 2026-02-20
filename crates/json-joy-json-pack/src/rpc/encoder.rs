//! ONC RPC message encoder.
//!
//! Upstream reference: `json-pack/src/rpc/RpcMessageEncoder.ts`

use json_joy_buffers::Writer;

use super::constants::{RpcMsgType, RpcReplyStat, RPC_VERSION};
use super::messages::{RpcMessage, RpcMismatchInfo, RpcOpaqueAuth};

/// RPC encoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpcEncodeError {
    #[error("auth body too large: {0} bytes (max 400)")]
    AuthBodyTooLarge(usize),
}

/// ONC RPC message encoder.
///
/// Encodes RPC messages to big-endian u32 wire format (RFC 1831).
pub struct RpcMessageEncoder {
    pub writer: Writer,
}

impl Default for RpcMessageEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcMessageEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    /// Encodes a Call message. Returns `Err` if an auth body exceeds 400 bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_call(
        &mut self,
        xid: u32,
        prog: u32,
        vers: u32,
        proc_: u32,
        cred: &RpcOpaqueAuth,
        verf: &RpcOpaqueAuth,
        params: &[u8],
    ) -> Result<Vec<u8>, RpcEncodeError> {
        self.write_call(xid, prog, vers, proc_, cred, verf, params)?;
        Ok(self.writer.flush())
    }

    /// Encodes an Accepted Reply. Returns `Err` if an auth body exceeds 400 bytes.
    ///
    /// `accept_stat` is a raw wire integer (SUCCESS = 0, PROG_UNAVAIL = 1, etc.).
    pub fn encode_accepted_reply(
        &mut self,
        xid: u32,
        verf: &RpcOpaqueAuth,
        accept_stat: u32,
        mismatch_info: Option<&RpcMismatchInfo>,
        results: &[u8],
    ) -> Result<Vec<u8>, RpcEncodeError> {
        self.write_accepted_reply(xid, verf, accept_stat, mismatch_info, results)?;
        Ok(self.writer.flush())
    }

    /// Encodes a Rejected Reply (cannot fail).
    ///
    /// `reject_stat` is a raw wire integer (RPC_MISMATCH = 0, AUTH_ERROR = 1).
    /// `auth_stat` is present only when `reject_stat == 1`.
    pub fn encode_rejected_reply(
        &mut self,
        xid: u32,
        reject_stat: u32,
        mismatch_info: Option<&RpcMismatchInfo>,
        auth_stat: Option<u32>,
    ) -> Vec<u8> {
        self.write_rejected_reply(xid, reject_stat, mismatch_info, auth_stat);
        self.writer.flush()
    }

    pub fn encode_message(&mut self, msg: &RpcMessage) -> Result<Vec<u8>, RpcEncodeError> {
        self.write_message(msg)?;
        Ok(self.writer.flush())
    }

    pub fn write_message(&mut self, msg: &RpcMessage) -> Result<(), RpcEncodeError> {
        match msg {
            RpcMessage::Call(m) => {
                self.write_call(m.xid, m.prog, m.vers, m.proc_, &m.cred, &m.verf, &m.params)?;
            }
            RpcMessage::AcceptedReply(m) => {
                let results = m.results.as_deref().unwrap_or(&[]);
                self.write_accepted_reply(
                    m.xid,
                    &m.verf,
                    m.stat as u32,
                    m.mismatch_info.as_ref(),
                    results,
                )?;
            }
            RpcMessage::RejectedReply(m) => {
                let auth_stat = m.auth_stat.map(|s| s as u32);
                self.write_rejected_reply(
                    m.xid,
                    m.stat as u32,
                    m.mismatch_info.as_ref(),
                    auth_stat,
                );
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn write_call(
        &mut self,
        xid: u32,
        prog: u32,
        vers: u32,
        proc_: u32,
        cred: &RpcOpaqueAuth,
        verf: &RpcOpaqueAuth,
        params: &[u8],
    ) -> Result<(), RpcEncodeError> {
        self.writer.u32(xid);
        self.writer.u32(RpcMsgType::Call as u32);
        self.writer.u32(RPC_VERSION);
        self.writer.u32(prog);
        self.writer.u32(vers);
        self.writer.u32(proc_);
        self.write_opaque_auth(cred)?;
        self.write_opaque_auth(verf)?;
        if !params.is_empty() {
            self.writer.buf(params);
        }
        Ok(())
    }

    fn write_accepted_reply(
        &mut self,
        xid: u32,
        verf: &RpcOpaqueAuth,
        accept_stat: u32,
        mismatch_info: Option<&RpcMismatchInfo>,
        results: &[u8],
    ) -> Result<(), RpcEncodeError> {
        self.writer.u32(xid);
        self.writer.u32(RpcMsgType::Reply as u32);
        self.writer.u32(RpcReplyStat::MsgAccepted as u32);
        self.write_opaque_auth(verf)?;
        self.writer.u32(accept_stat);
        if let Some(mi) = mismatch_info {
            self.writer.u32(mi.low);
            self.writer.u32(mi.high);
        }
        if !results.is_empty() {
            self.writer.buf(results);
        }
        Ok(())
    }

    fn write_rejected_reply(
        &mut self,
        xid: u32,
        reject_stat: u32,
        mismatch_info: Option<&RpcMismatchInfo>,
        auth_stat: Option<u32>,
    ) {
        self.writer.u32(xid);
        self.writer.u32(RpcMsgType::Reply as u32);
        self.writer.u32(RpcReplyStat::MsgDenied as u32);
        self.writer.u32(reject_stat);
        if reject_stat == 0 {
            // RPC_MISMATCH
            if let Some(mi) = mismatch_info {
                self.writer.u32(mi.low);
                self.writer.u32(mi.high);
            }
        } else if reject_stat == 1 {
            // AUTH_ERROR
            if let Some(s) = auth_stat {
                self.writer.u32(s);
            }
        }
    }

    fn write_opaque_auth(&mut self, auth: &RpcOpaqueAuth) -> Result<(), RpcEncodeError> {
        let len = auth.body.len();
        if len > 400 {
            return Err(RpcEncodeError::AuthBodyTooLarge(len));
        }
        let padded_len = (len + 3) & !3;
        let padding = padded_len - len;
        self.writer.u32(auth.flavor as u32);
        self.writer.u32(len as u32);
        self.writer.buf(&auth.body);
        for _ in 0..padding {
            self.writer.u8(0);
        }
        Ok(())
    }
}
