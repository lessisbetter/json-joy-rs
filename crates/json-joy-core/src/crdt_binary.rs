//! Shared CRDT binary primitives used by model/patch/runtime code.
//!
//! These helpers mirror `json-joy@17.67.0` `CrdtReader/CrdtWriter` behavior
//! for `vu57`, `b1vu56`, and logical clock-table/id handling.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalClockBase {
    pub sid: u64,
    pub time: u64,
}

pub fn write_vu57(out: &mut Vec<u8>, mut value: u64) {
    for _ in 0..7 {
        let mut b = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(b);
            return;
        }
        b |= 0x80;
        out.push(b);
    }
    out.push((value & 0xff) as u8);
}

pub fn read_vu57(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    for i in 0..8 {
        let b = *data.get(*pos)?;
        *pos += 1;
        if i < 7 {
            let part = (b & 0x7f) as u64;
            result |= part.checked_shl(shift)?;
            if (b & 0x80) == 0 {
                return Some(result);
            }
            shift += 7;
        } else {
            result |= (b as u64).checked_shl(49)?;
            return Some(result);
        }
    }
    None
}

pub fn write_b1vu56(out: &mut Vec<u8>, flag: u8, mut value: u64) {
    let low6 = (value & 0x3f) as u8;
    value >>= 6;
    let mut first = ((flag & 1) << 7) | low6;
    if value == 0 {
        out.push(first);
        return;
    }
    first |= 0x40;
    out.push(first);

    for _ in 0..6 {
        let mut b = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(b);
            return;
        }
        b |= 0x80;
        out.push(b);
    }
    out.push((value & 0xff) as u8);
}

pub fn read_b1vu56(data: &[u8], pos: &mut usize) -> Option<(u8, u64)> {
    let first = *data.get(*pos)?;
    *pos += 1;
    let flag = (first >> 7) & 1;
    let mut result: u64 = (first & 0x3f) as u64;
    if (first & 0x40) == 0 {
        return Some((flag, result));
    }

    let mut shift: u32 = 6;
    for i in 0..7 {
        let b = *data.get(*pos)?;
        *pos += 1;
        if i < 6 {
            result |= ((b & 0x7f) as u64).checked_shl(shift)?;
            if (b & 0x80) == 0 {
                return Some((flag, result));
            }
            shift += 7;
        } else {
            result |= (b as u64).checked_shl(48)?;
            return Some((flag, result));
        }
    }
    None
}

pub fn parse_logical_clock_table(data: &[u8]) -> Option<(usize, Vec<LogicalClockBase>)> {
    if data.is_empty() || (data[0] & 0x80) != 0 || data.len() < 4 {
        return None;
    }
    let offset = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = 4usize.checked_add(offset)?;
    let len = read_vu57(data, &mut pos)? as usize;
    if len == 0 {
        return None;
    }

    let mut table = Vec::with_capacity(len);
    for _ in 0..len {
        let sid = read_vu57(data, &mut pos)?;
        let time = read_vu57(data, &mut pos)?;
        table.push(LogicalClockBase { sid, time });
    }

    Some((offset, table))
}

pub fn first_logical_clock_sid_time(data: &[u8]) -> Option<(u64, u64)> {
    let (_, table) = parse_logical_clock_table(data)?;
    let first = table.first()?;
    Some((first.sid, first.time))
}
