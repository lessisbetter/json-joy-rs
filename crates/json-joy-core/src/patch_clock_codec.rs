use crate::crdt_binary::{read_b1vu56, read_vu57, write_b1vu56, write_vu57, LogicalClockBase};
use crate::patch::Timestamp;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchClockCodecError {
    #[error("invalid clock table binary")]
    InvalidClockTable,
    #[error("invalid relative timestamp")]
    InvalidRelativeId,
}

pub fn encode_clock_table(clock_table: &[LogicalClockBase]) -> Vec<u8> {
    let mut out = Vec::new();
    write_vu57(&mut out, clock_table.len() as u64);
    for c in clock_table {
        write_vu57(&mut out, c.sid);
        write_vu57(&mut out, c.time);
    }
    out
}

pub fn decode_clock_table(data: &[u8]) -> Result<Vec<LogicalClockBase>, PatchClockCodecError> {
    let mut pos = 0usize;
    let len = read_vu57(data, &mut pos).ok_or(PatchClockCodecError::InvalidClockTable)? as usize;
    if len == 0 {
        return Err(PatchClockCodecError::InvalidClockTable);
    }
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let sid = read_vu57(data, &mut pos).ok_or(PatchClockCodecError::InvalidClockTable)?;
        let time = read_vu57(data, &mut pos).ok_or(PatchClockCodecError::InvalidClockTable)?;
        out.push(LogicalClockBase { sid, time });
    }
    if pos != data.len() {
        return Err(PatchClockCodecError::InvalidClockTable);
    }
    Ok(out)
}

pub fn encode_relative_timestamp(session_index: u64, time_diff: u64) -> Vec<u8> {
    let mut out = Vec::new();
    if session_index <= 0b111 && time_diff <= 0b1111 {
        out.push(((session_index as u8) << 4) | (time_diff as u8));
    } else {
        // CRDT id encoding uses b1vu56 flag set to 1 for wide form.
        write_b1vu56(&mut out, 1, session_index);
        write_vu57(&mut out, time_diff);
    }
    out
}

pub fn decode_relative_timestamp(data: &[u8]) -> Result<(u64, u64), PatchClockCodecError> {
    if data.is_empty() {
        return Err(PatchClockCodecError::InvalidRelativeId);
    }
    let first = data[0];
    if first <= 0x7f {
        return Ok(((first >> 4) as u64, (first & 0x0f) as u64));
    }
    let mut pos = 0usize;
    let (flag, session_index) =
        read_b1vu56(data, &mut pos).ok_or(PatchClockCodecError::InvalidRelativeId)?;
    if flag != 1 {
        return Err(PatchClockCodecError::InvalidRelativeId);
    }
    let time_diff = read_vu57(data, &mut pos).ok_or(PatchClockCodecError::InvalidRelativeId)?;
    if pos != data.len() {
        return Err(PatchClockCodecError::InvalidRelativeId);
    }
    Ok((session_index, time_diff))
}

pub fn decode_with_clock_table(
    clock_table: &[LogicalClockBase],
    session_index: u64,
    time_diff: u64,
) -> Result<Timestamp, PatchClockCodecError> {
    if session_index == 0 {
        return Ok(Timestamp {
            sid: 0,
            time: time_diff,
        });
    }
    let base = clock_table
        .get(session_index as usize - 1)
        .ok_or(PatchClockCodecError::InvalidRelativeId)?;
    let time = base
        .time
        .checked_sub(time_diff)
        .ok_or(PatchClockCodecError::InvalidRelativeId)?;
    Ok(Timestamp {
        sid: base.sid,
        time,
    })
}
