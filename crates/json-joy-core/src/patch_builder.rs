//! Native patch construction helpers and canonical patch timeline validation.
//!
//! Encoding implementation note:
//! - Actual binary opcode/clock encoding lives in `patch/encode.rs`.
//! - This module owns builder ergonomics and shared error taxonomy.

use crate::patch::{DecodedOp, Patch, PatchError};

#[derive(Debug, thiserror::Error)]
pub enum PatchBuildError {
    #[error("operation id must match patch timeline at index {index}: expected ({expected_sid},{expected_time}) got ({actual_sid},{actual_time})")]
    NonCanonicalId {
        index: usize,
        expected_sid: u64,
        expected_time: u64,
        actual_sid: u64,
        actual_time: u64,
    },
    #[error("ins_vec index must fit in u8")]
    VecIndexOutOfRange,
    #[error("binary patch decode failed after encode: {0}")]
    EncodedPatchDecode(#[from] PatchError),
}

#[derive(Debug, Default)]
pub struct PatchBuilder {
    sid: u64,
    time: u64,
    ops: Vec<DecodedOp>,
}

impl PatchBuilder {
    pub fn new(sid: u64, time: u64) -> Self {
        Self {
            sid,
            time,
            ops: Vec::new(),
        }
    }

    pub fn sid(&self) -> u64 {
        self.sid
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn ops(&self) -> &[DecodedOp] {
        &self.ops
    }

    pub fn push_op(&mut self, op: DecodedOp) {
        self.ops.push(op);
    }

    pub fn into_bytes(self) -> Result<Vec<u8>, PatchBuildError> {
        encode_patch_from_ops(self.sid, self.time, &self.ops)
    }

    pub fn into_patch(self) -> Result<Patch, PatchBuildError> {
        let bytes = self.into_bytes()?;
        Ok(Patch::from_binary(&bytes)?)
    }
}

pub fn encode_patch_from_ops(
    sid: u64,
    time: u64,
    ops: &[DecodedOp],
) -> Result<Vec<u8>, PatchBuildError> {
    crate::patch::encode_patch_from_ops(sid, time, ops)
}
