use json_joy_json_pack::write_cbor_value_like_json_pack;

use ciborium::value::Value as CborValue;

use crate::crdt_binary::write_b1vu56;
use crate::model_runtime::types::{ConCell, Id, RuntimeNode};
use crate::model_runtime::RuntimeModel;
use crate::patch_clock_codec;

use super::types::{
    cbor_from_json, group_arr_chunks, group_bin_chunks, group_str_chunks, write_type_len,
    ClockEncCtx, SidecarBinaryCodecError,
};

pub fn encode_model_binary_to_sidecar(
    model_binary: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), SidecarBinaryCodecError> {
    let runtime = RuntimeModel::from_model_binary(model_binary)?;
    if runtime.server_clock_time.is_some() || runtime.clock_table.is_empty() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let mut ctx = ClockEncCtx::new(&runtime.clock_table)?;
    let mut meta = vec![0, 0, 0, 0];
    let view = match runtime.root {
        Some(root) if root.sid != 0 => {
            let view_cbor = encode_node_view(root, &runtime, &mut ctx, &mut meta)?;
            let mut encoded = Vec::new();
            write_cbor_value_like_json_pack(&mut encoded, &view_cbor)
                .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
            encoded
        }
        _ => {
            meta.push(0);
            Vec::new()
        }
    };

    let table_offset = (meta.len() - 4) as u32;
    meta[0] = ((table_offset >> 24) & 0xff) as u8;
    meta[1] = ((table_offset >> 16) & 0xff) as u8;
    meta[2] = ((table_offset >> 8) & 0xff) as u8;
    meta[3] = (table_offset & 0xff) as u8;

    meta.extend_from_slice(&patch_clock_codec::encode_clock_table(&ctx.table));

    Ok((view, meta))
}

fn encode_node_view(
    id: Id,
    runtime: &RuntimeModel,
    clock: &mut ClockEncCtx,
    meta: &mut Vec<u8>,
) -> Result<CborValue, SidecarBinaryCodecError> {
    let node = runtime
        .nodes
        .get(&id)
        .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
    clock.append(id, meta)?;
    Ok(match node {
        RuntimeNode::Con(ConCell::Json(v)) => {
            meta.push(0);
            cbor_from_json(v)
        }
        RuntimeNode::Con(ConCell::Ref(ref_id)) => {
            meta.push(1);
            clock.append(*ref_id, meta)?;
            CborValue::Null
        }
        RuntimeNode::Con(ConCell::Undef) => {
            meta.push(0);
            CborValue::Null
        }
        RuntimeNode::Val(child) => {
            meta.push(0b0010_0000);
            encode_node_view(*child, runtime, clock, meta)?
        }
        RuntimeNode::Obj(entries) => {
            let mut sorted: Vec<(&str, Id)> =
                entries.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            write_type_len(meta, 2, sorted.len() as u64);
            let mut map = Vec::with_capacity(sorted.len());
            for (k, child) in sorted {
                let child_view = encode_node_view(child, runtime, clock, meta)?;
                map.push((CborValue::Text(k.to_string()), child_view));
            }
            CborValue::Map(map)
        }
        RuntimeNode::Vec(elements) => {
            let len = elements.keys().max().map(|v| v + 1).unwrap_or(0);
            write_type_len(meta, 3, len);
            let mut arr = Vec::with_capacity(len as usize);
            for i in 0..len {
                if let Some(child) = elements.get(&i) {
                    arr.push(encode_node_view(*child, runtime, clock, meta)?);
                } else {
                    // Missing vec slots are represented as undefined in upstream.
                    // For JSON-transport side this serializes as null.
                    arr.push(CborValue::Null);
                }
            }
            CborValue::Array(arr)
        }
        RuntimeNode::Str(atoms) => {
            let chunks = group_str_chunks(atoms);
            write_type_len(meta, 4, chunks.len() as u64);
            for ch in chunks {
                clock.append(ch.id, meta)?;
                write_b1vu56(meta, if ch.text.is_some() { 0 } else { 1 }, ch.span);
            }
            let mut s = String::new();
            for atom in atoms {
                if let Some(ch) = atom.ch {
                    s.push(ch);
                }
            }
            CborValue::Text(s)
        }
        RuntimeNode::Bin(atoms) => {
            let chunks = group_bin_chunks(atoms);
            write_type_len(meta, 5, chunks.len() as u64);
            for ch in chunks {
                clock.append(ch.id, meta)?;
                write_b1vu56(meta, if ch.bytes.is_some() { 0 } else { 1 }, ch.span);
            }
            let mut b = Vec::new();
            for atom in atoms {
                if let Some(x) = atom.byte {
                    b.push(x);
                }
            }
            CborValue::Bytes(b)
        }
        RuntimeNode::Arr(atoms) => {
            let chunks = group_arr_chunks(atoms);
            write_type_len(meta, 6, chunks.len() as u64);
            let mut values = Vec::new();
            for ch in chunks {
                clock.append(ch.id, meta)?;
                match ch.values {
                    Some(ids) => {
                        write_b1vu56(meta, 0, ch.span);
                        for child in ids {
                            values.push(encode_node_view(child, runtime, clock, meta)?);
                        }
                    }
                    None => {
                        write_b1vu56(meta, 1, ch.span);
                    }
                }
            }
            CborValue::Array(values)
        }
    })
}
