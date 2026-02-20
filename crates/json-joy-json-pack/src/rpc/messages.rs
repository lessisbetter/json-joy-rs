//! ONC RPC message structures.
//!
//! Upstream reference: `json-pack/src/rpc/messages.ts`

use super::constants::{RpcAcceptStat, RpcAuthFlavor, RpcAuthStat, RpcRejectStat};

/// Opaque authentication data (flavor + body bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcOpaqueAuth {
    pub flavor: RpcAuthFlavor,
    pub body: Vec<u8>,
}

impl RpcOpaqueAuth {
    /// Creates an `AUTH_NONE` credential (flavor = 0, empty body).
    pub fn none() -> Self {
        Self {
            flavor: RpcAuthFlavor::AuthNone,
            body: Vec::new(),
        }
    }
}

/// Mismatch version range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcMismatchInfo {
    pub low: u32,
    pub high: u32,
}

/// RPC Call message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcCallMessage {
    pub xid: u32,
    pub rpcvers: u32,
    pub prog: u32,
    pub vers: u32,
    pub proc_: u32,
    pub cred: RpcOpaqueAuth,
    pub verf: RpcOpaqueAuth,
    /// Raw RPC parameter bytes (empty if no params).
    pub params: Vec<u8>,
}

/// RPC Accepted Reply message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcAcceptedReplyMessage {
    pub xid: u32,
    pub verf: RpcOpaqueAuth,
    pub stat: RpcAcceptStat,
    pub mismatch_info: Option<RpcMismatchInfo>,
    /// Raw trailing reply bytes (if any), preserved regardless of accept status.
    pub results: Option<Vec<u8>>,
}

/// RPC Rejected Reply message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcRejectedReplyMessage {
    pub xid: u32,
    pub stat: RpcRejectStat,
    pub mismatch_info: Option<RpcMismatchInfo>,
    /// Present only when stat == AUTH_ERROR.
    pub auth_stat: Option<RpcAuthStat>,
}

/// RPC message — either a call or a reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcMessage {
    Call(RpcCallMessage),
    AcceptedReply(RpcAcceptedReplyMessage),
    RejectedReply(RpcRejectedReplyMessage),
}
