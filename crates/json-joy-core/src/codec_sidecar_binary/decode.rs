use std::collections::{BTreeMap, HashMap};

use ciborium::value::Value as CborValue;
use serde_json::Value;

use crate::crdt_binary::{read_b1vu56, LogicalClockBase};
use crate::model_runtime::types::{ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};
use crate::model_runtime::RuntimeModel;
use crate::patch_clock_codec;

use super::types::{decode_sidecar_id, json_from_cbor, MetaCursor, SidecarBinaryCodecError};

pub fn decode_sidecar_to_model_binary(
    view_binary: &[u8],
    meta_binary: &[u8],
) -> Result<Vec<u8>, SidecarBinaryCodecError> {
    if meta_binary.len() < 4 {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }
    let offset = u32::from_be_bytes([
        meta_binary[0],
        meta_binary[1],
        meta_binary[2],
        meta_binary[3],
    ]) as usize;
    if 4 + offset > meta_binary.len() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let table = patch_clock_codec::decode_clock_table(&meta_binary[4 + offset..])
        .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;

    let mut dec = MetaCursor::new(&meta_binary[4..4 + offset]);
    let mut nodes = HashMap::new();
    let root = if dec.peek().ok_or(SidecarBinaryCodecError::InvalidPayload)? == 0 {
        dec.u8()
            .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
        None
    } else {
        let view: CborValue = ciborium::de::from_reader(view_binary)
            .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
        Some(decode_node_from_sidecar(
            &view, &mut dec, &table, &mut nodes,
        )?)
    };
    if !dec.is_eof() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let runtime = RuntimeModel {
        nodes,
        root,
        clock: Default::default(),
        fallback_view: Value::Null,
        infer_empty_object_root: false,
        clock_table: table,
        server_clock_time: None,
    };
    runtime
        .to_model_binary_like()
        .map_err(SidecarBinaryCodecError::from)
}

fn decode_node_from_sidecar(
    view: &CborValue,
    meta: &mut MetaCursor<'_>,
    table: &[LogicalClockBase],
    nodes: &mut HashMap<Id, RuntimeNode>,
) -> Result<Id, SidecarBinaryCodecError> {
    let id = decode_sidecar_id(meta, table)?;
    let octet = meta
        .u8()
        .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
    let major = octet >> 5;
    let len = meta.read_len(octet & 0x1f)?;
    let node = match major {
        0 => {
            if len == 0 {
                RuntimeNode::Con(ConCell::Json(json_from_cbor(view)?))
            } else {
                let ref_id = decode_sidecar_id(meta, table)?;
                RuntimeNode::Con(ConCell::Ref(ref_id))
            }
        }
        1 => {
            let child = decode_node_from_sidecar(view, meta, table, nodes)?;
            RuntimeNode::Val(child)
        }
        2 => {
            let map = match view {
                CborValue::Map(m) => m,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            if map.len() != len as usize {
                return Err(SidecarBinaryCodecError::InvalidPayload);
            }
            let mut keys: Vec<String> = map
                .iter()
                .map(|(k, _)| match k {
                    CborValue::Text(s) => Ok(s.clone()),
                    _ => Err(SidecarBinaryCodecError::InvalidPayload),
                })
                .collect::<Result<_, _>>()?;
            keys.sort();
            let mut entries = Vec::with_capacity(keys.len());
            for k in keys {
                let child_view = map
                    .iter()
                    .find_map(|(kk, vv)| match kk {
                        CborValue::Text(s) if s == &k => Some(vv),
                        _ => None,
                    })
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                entries.push((k, child));
            }
            RuntimeNode::Obj(entries)
        }
        3 => {
            let arr = match view {
                CborValue::Array(a) => a,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            if arr.len() != len as usize {
                return Err(SidecarBinaryCodecError::InvalidPayload);
            }
            let mut elements = BTreeMap::new();
            for (idx, child_view) in arr.iter().enumerate() {
                let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                if child.sid != 0 {
                    elements.insert(idx as u64, child);
                }
            }
            RuntimeNode::Vec(elements)
        }
        4 => {
            let s = match view {
                CborValue::Text(s) => s,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut chars = s.chars();
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(StrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            ch: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let ch = chars
                            .next()
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        atoms.push(StrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            ch: Some(ch),
                        });
                    }
                }
            }
            RuntimeNode::Str(atoms)
        }
        5 => {
            let bytes = match view {
                CborValue::Bytes(b) => b,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut byte_pos = 0usize;
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            byte: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let b = *bytes
                            .get(byte_pos)
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        byte_pos += 1;
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            byte: Some(b),
                        });
                    }
                }
            }
            RuntimeNode::Bin(atoms)
        }
        6 => {
            let arr = match view {
                CborValue::Array(a) => a,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut view_idx = 0usize;
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let child_view = arr
                            .get(view_idx)
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        view_idx += 1;
                        let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: Some(child),
                        });
                    }
                }
            }
            RuntimeNode::Arr(atoms)
        }
        _ => return Err(SidecarBinaryCodecError::InvalidPayload),
    };

    nodes.insert(id, node);
    Ok(id)
}
