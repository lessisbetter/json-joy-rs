//! Upstream parity port of `json-crdt-patch/compaction.ts`.

use crate::patch::{DecodedOp, Patch, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};

#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    #[error("patch sid mismatch")]
    SidMismatch,
    #[error("patch timestamp conflict")]
    TimestampConflict,
    #[error("failed to encode compacted patch: {0}")]
    Build(#[from] PatchBuildError),
    #[error("failed to decode compacted patch: {0}")]
    Decode(#[from] crate::patch::PatchError),
}

fn patch_from_ops(sid: u64, time: u64, ops: &[DecodedOp]) -> Result<Patch, CompactionError> {
    let bytes = encode_patch_from_ops(sid, time, ops)?;
    Ok(Patch::from_binary(&bytes)?)
}

/// Combines patches with upstream `combine()` semantics.
pub fn combine_patches(patches: &[Patch]) -> Result<Patch, CompactionError> {
    if patches.is_empty() {
        return patch_from_ops(0, 0, &[]);
    }
    let first = &patches[0];
    let first_id = first.id();
    let mut ops = first.decoded_ops().to_vec();
    let mut sid_time = first_id;

    for current in patches.iter().skip(1) {
        let current_id = current.id();
        match (sid_time, current_id) {
            (None, None) => return Ok(first.clone()),
            (None, Some(_)) => {
                ops.extend_from_slice(current.decoded_ops());
                return Ok(current.clone());
            }
            (Some((sid, time)), None) => return patch_from_ops(sid, time, &ops),
            (Some((sid_a, time_a)), Some((sid_b, time_b))) => {
                if sid_a != sid_b {
                    return Err(CompactionError::SidMismatch);
                }
                let next_tick = time_a.saturating_add(ops.iter().map(DecodedOp::span).sum::<u64>());
                if time_b < next_tick {
                    return Err(CompactionError::TimestampConflict);
                }
                if time_b > next_tick {
                    ops.push(DecodedOp::Nop {
                        id: Timestamp {
                            sid: sid_a,
                            time: next_tick,
                        },
                        len: time_b - next_tick,
                    });
                }
                ops.extend_from_slice(current.decoded_ops());
                sid_time = Some((sid_a, time_a));
            }
        }
    }

    match first_id {
        Some((sid, time)) => patch_from_ops(sid, time, &ops),
        None => {
            if let Some((sid, time)) = sid_time {
                patch_from_ops(sid, time, &ops)
            } else if let Some((sid, time)) = patches.iter().find_map(Patch::id) {
                patch_from_ops(sid, time, &ops)
            } else {
                patch_from_ops(0, 0, &[])
            }
        }
    }
}

/// Compacts a patch with upstream `compact()` semantics.
pub fn compact_patch(patch: &Patch) -> Result<Patch, CompactionError> {
    let ops = patch.decoded_ops();
    if ops.is_empty() {
        return Ok(patch.clone());
    }
    let mut new_ops = Vec::with_capacity(ops.len());
    new_ops.push(ops[0].clone());

    for op in ops.iter().skip(1) {
        let mut merged = false;
        if let Some(last) = new_ops.last_mut() {
            if let (
                DecodedOp::InsStr {
                    id: last_id,
                    obj: last_obj,
                    reference: last_ref,
                    data: last_data,
                },
                DecodedOp::InsStr {
                    id,
                    obj,
                    reference,
                    data,
                },
            ) = (last, op)
            {
                // Upstream JS uses string `.length` (UTF-16 code units) for op
                // span progression, not Unicode scalar count.
                let last_next_tick = last_id
                    .time
                    .saturating_add(last_data.encode_utf16().count() as u64);
                let is_time_consecutive = last_next_tick == id.time;
                let same_obj = *last_obj == *obj;
                let is_append = last_next_tick == reference.time.saturating_add(1)
                    && last_ref.sid == reference.sid;
                if is_time_consecutive && same_obj && is_append {
                    last_data.push_str(data);
                    merged = true;
                }
            }
        }
        if !merged {
            new_ops.push(op.clone());
        }
    }

    let (sid, time) = patch.id().unwrap_or((0, 0));
    patch_from_ops(sid, time, &new_ops)
}
