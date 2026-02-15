use crate::patch::{Patch, PatchError};
use thiserror::Error;

pub const PATCH_LOG_VERSION: u8 = 1;
pub const MAX_PATCH_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PatchLogError {
    #[error("unsupported patch log version: {0}")]
    UnsupportedVersion(u8),
    #[error("corrupt pending patches: truncated length header")]
    TruncatedLengthHeader,
    #[error("corrupt pending patches: patch size {0} exceeds max")]
    PatchTooLarge(usize),
    #[error("corrupt pending patches: truncated patch data")]
    TruncatedPatchData,
    #[error("patch decode failed: {0}")]
    PatchDecode(#[from] PatchError),
}

pub fn serialize_patches(patches: &[Patch]) -> Vec<u8> {
    if patches.is_empty() {
        return Vec::new();
    }

    let binaries: Vec<Vec<u8>> = patches.iter().map(|p| p.to_binary()).collect();
    let total_len = 1usize + binaries.iter().map(|b| 4 + b.len()).sum::<usize>();

    let mut out = Vec::with_capacity(total_len);
    out.push(PATCH_LOG_VERSION);

    for bin in binaries {
        out.extend_from_slice(&(bin.len() as u32).to_be_bytes());
        out.extend_from_slice(&bin);
    }

    out
}

pub fn deserialize_patches(data: &[u8]) -> Result<Vec<Patch>, PatchLogError> {
    if data.is_empty() {
        return Ok(vec![]);
    }

    let version = data[0];
    if version != PATCH_LOG_VERSION {
        return Err(PatchLogError::UnsupportedVersion(version));
    }

    let mut patches = Vec::new();
    let mut offset = 1usize;

    while offset < data.len() {
        if offset + 4 > data.len() {
            return Err(PatchLogError::TruncatedLengthHeader);
        }

        let len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if len > MAX_PATCH_SIZE {
            return Err(PatchLogError::PatchTooLarge(len));
        }
        if len > data.len().saturating_sub(offset) {
            return Err(PatchLogError::TruncatedPatchData);
        }

        let patch = Patch::from_binary(&data[offset..offset + len])?;
        patches.push(patch);
        offset += len;
    }

    Ok(patches)
}

pub fn append_patch(existing: &[u8], patch: &Patch) -> Vec<u8> {
    let patch_bin = patch.to_binary();

    if existing.is_empty() {
        let mut out = Vec::with_capacity(1 + 4 + patch_bin.len());
        out.push(PATCH_LOG_VERSION);
        out.extend_from_slice(&(patch_bin.len() as u32).to_be_bytes());
        out.extend_from_slice(&patch_bin);
        return out;
    }

    let mut out = Vec::with_capacity(existing.len() + 4 + patch_bin.len());
    out.extend_from_slice(existing);
    out.extend_from_slice(&(patch_bin.len() as u32).to_be_bytes());
    out.extend_from_slice(&patch_bin);
    out
}
