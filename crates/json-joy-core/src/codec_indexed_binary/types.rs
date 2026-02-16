use std::collections::{BTreeMap, HashMap};

use ciborium::value::Value as CborValue;
use json_joy_json_pack::{
    write_cbor_text_like_json_pack, write_cbor_uint_major, write_json_like_json_pack,
};
use serde_json::Value;
use thiserror::Error;

use crate::crdt_binary::{read_b1vu56, read_vu57, write_b1vu56, write_vu57, LogicalClockBase};
use crate::model::ModelError;
use crate::model_runtime::types::{ArrAtom, BinAtom, Id, StrAtom};
use crate::patch_clock_codec;

pub type IndexedFields = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Error)]
pub enum IndexedBinaryCodecError {
    #[error("invalid indexed fields")]
    InvalidFields,
    #[error("invalid indexed node payload")]
    InvalidNode,
    #[error("model runtime failure: {0}")]
    Model(#[from] ModelError),
}

pub(super) fn encode_clock_table(clock_table: &[LogicalClockBase]) -> Vec<u8> {
    patch_clock_codec::encode_clock_table(clock_table)
}

pub(super) fn decode_clock_table(
    data: &[u8],
) -> Result<Vec<LogicalClockBase>, IndexedBinaryCodecError> {
    patch_clock_codec::decode_clock_table(data).map_err(|_| IndexedBinaryCodecError::InvalidFields)
}

pub(super) fn to_base36(v: u64) -> String {
    if v == 0 {
        return "0".to_string();
    }
    let mut n = v;
    let mut out = Vec::new();
    while n > 0 {
        let d = (n % 36) as u8;
        out.push(if d < 10 {
            (b'0' + d) as char
        } else {
            (b'a' + (d - 10)) as char
        });
        n /= 36;
    }
    out.iter().rev().collect()
}

pub(super) fn from_base36(s: &str) -> Option<u64> {
    let mut acc = 0u64;
    for b in s.bytes() {
        let v = match b {
            b'0'..=b'9' => (b - b'0') as u64,
            b'a'..=b'z' => 10 + (b - b'a') as u64,
            b'A'..=b'Z' => 10 + (b - b'A') as u64,
            _ => return None,
        };
        acc = acc.checked_mul(36)?.checked_add(v)?;
    }
    Some(acc)
}

pub(super) fn encode_indexed_id(
    id: Id,
    index_by_sid: &HashMap<u64, u64>,
) -> Result<Vec<u8>, IndexedBinaryCodecError> {
    let x = *index_by_sid
        .get(&id.sid)
        .ok_or(IndexedBinaryCodecError::InvalidFields)?;
    let y = id.time;
    let mut out = Vec::new();
    if x <= 0b111 && y <= 0b1111 {
        out.push(((x as u8) << 4) | (y as u8));
    } else {
        write_b1vu56(&mut out, 1, x);
        write_vu57(&mut out, y);
    }
    Ok(out)
}

pub(super) fn decode_indexed_id(
    data: &[u8],
    clock_table: &[LogicalClockBase],
) -> Result<Id, IndexedBinaryCodecError> {
    if data.is_empty() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }
    let first = data[0];
    let (session_index, time) = if first <= 0x7f {
        ((first >> 4) as u64, (first & 0x0f) as u64)
    } else {
        let mut pos = 0usize;
        let (flag, x) =
            read_b1vu56(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        if flag != 1 {
            return Err(IndexedBinaryCodecError::InvalidFields);
        }
        let y = read_vu57(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        if pos != data.len() {
            return Err(IndexedBinaryCodecError::InvalidFields);
        }
        (x, y)
    };

    let sid = clock_table
        .get(session_index as usize)
        .ok_or(IndexedBinaryCodecError::InvalidFields)?
        .sid;
    Ok(Id { sid, time })
}

pub(super) fn write_type_len(out: &mut Vec<u8>, major: u8, len: u64) {
    if len < 24 {
        out.push((major << 5) | (len as u8));
    } else if len <= 0xff {
        out.push((major << 5) | 24);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push((major << 5) | 25);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    } else {
        out.push((major << 5) | 26);
        out.push(((len >> 24) & 0xff) as u8);
        out.push(((len >> 16) & 0xff) as u8);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    }
}

pub(super) fn write_json_pack_any(out: &mut Vec<u8>, v: &Value) {
    write_json_like_json_pack(out, v).expect("json-pack encode must support serde_json::Value");
}

pub(super) fn write_text_like_json_pack(out: &mut Vec<u8>, text: &str) {
    write_cbor_text_like_json_pack(out, text);
}

pub(super) fn write_uint_major(out: &mut Vec<u8>, major: u8, n: u64) {
    write_cbor_uint_major(out, major, n);
}

pub(super) fn json_from_cbor(v: &CborValue) -> Result<Value, IndexedBinaryCodecError> {
    json_joy_json_pack::cbor_to_json(v).map_err(|_| IndexedBinaryCodecError::InvalidNode)
}

#[derive(Debug)]
pub(super) struct StrChunk {
    pub id: Id,
    pub span: u64,
    pub text: Option<String>,
}

#[derive(Debug)]
pub(super) struct BinChunk {
    pub id: Id,
    pub span: u64,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct ArrChunk {
    pub id: Id,
    pub span: u64,
    pub values: Option<Vec<Id>>,
}

pub(super) fn group_str_chunks(atoms: &[StrAtom]) -> Vec<StrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].ch.is_some() == atoms[j].ch.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.ch.is_some() {
            let mut s = String::new();
            for a in &atoms[i..j] {
                if let Some(ch) = a.ch {
                    s.push(ch);
                }
            }
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: Some(s),
            });
        } else {
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: None,
            });
        }
        i = j;
    }
    out
}

pub(super) fn group_bin_chunks(atoms: &[BinAtom]) -> Vec<BinChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].byte.is_some() == atoms[j].byte.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.byte.is_some() {
            let mut bytes = Vec::new();
            for a in &atoms[i..j] {
                if let Some(b) = a.byte {
                    bytes.push(b);
                }
            }
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: Some(bytes),
            });
        } else {
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: None,
            });
        }
        i = j;
    }
    out
}

pub(super) fn group_arr_chunks(atoms: &[ArrAtom]) -> Vec<ArrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].value.is_some() == atoms[j].value.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.value.is_some() {
            let mut values = Vec::new();
            for a in &atoms[i..j] {
                if let Some(v) = a.value {
                    values.push(v);
                }
            }
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: Some(values),
            });
        } else {
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: None,
            });
        }
        i = j;
    }
    out
}
