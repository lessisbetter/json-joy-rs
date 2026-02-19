//! Clock types for the JSON CRDT Patch protocol.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/clock/`.

use crate::json_crdt_patch::enums::SESSION;
use std::collections::HashMap;
use std::fmt;

// ── Core structs ───────────────────────────────────────────────────────────

/// An immutable logical timestamp: `(session_id, logical_time)`.
///
/// Mirrors `ITimestampStruct` / `Timestamp` from upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ts {
    pub sid: u64,
    pub time: u64,
}

impl Ts {
    pub const fn new(sid: u64, time: u64) -> Self {
        Self { sid, time }
    }
}

/// An immutable logical time-span: `(session_id, logical_time, span)`.
///
/// Mirrors `ITimespanStruct` / `Timespan` from upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tss {
    pub sid: u64,
    pub time: u64,
    pub span: u64,
}

impl Tss {
    pub const fn new(sid: u64, time: u64, span: u64) -> Self {
        Self { sid, time, span }
    }

    pub fn ts(&self) -> Ts {
        Ts::new(self.sid, self.time)
    }
}

// ── Factory functions ──────────────────────────────────────────────────────

/// Create a timestamp.
#[inline]
pub fn ts(sid: u64, time: u64) -> Ts {
    Ts::new(sid, time)
}

/// Create a timespan.
#[inline]
pub fn tss(sid: u64, time: u64, span: u64) -> Tss {
    Tss::new(sid, time, span)
}

/// Advance a timestamp by `cycles`, returning the new timestamp.
#[inline]
pub fn tick(stamp: Ts, cycles: u64) -> Ts {
    Ts::new(stamp.sid, stamp.time + cycles)
}

/// Returns `true` if both timestamps are equal (time first, then sid).
#[inline]
pub fn equal(a: Ts, b: Ts) -> bool {
    a.time == b.time && a.sid == b.sid
}

/// Compare two timestamps (time first, then session ID).
/// Returns `1`, `0`, or `-1`.
#[inline]
pub fn compare(a: Ts, b: Ts) -> i8 {
    if a.time > b.time {
        return 1;
    }
    if a.time < b.time {
        return -1;
    }
    if a.sid > b.sid {
        return 1;
    }
    if a.sid < b.sid {
        return -1;
    }
    0
}

/// Returns `true` if `[ts1, span1)` completely contains `[ts2, span2)`.
pub fn contains(ts1: Ts, span1: u64, ts2: Ts, span2: u64) -> bool {
    if ts1.sid != ts2.sid {
        return false;
    }
    if ts1.time > ts2.time {
        return false;
    }
    if ts1.time + span1 < ts2.time + span2 {
        return false;
    }
    true
}

/// Returns `true` if the timespan `[ts1, span1)` contains point `ts2`.
pub fn contains_id(ts1: Ts, span1: u64, ts2: Ts) -> bool {
    if ts1.sid != ts2.sid {
        return false;
    }
    if ts1.time > ts2.time {
        return false;
    }
    if ts1.time + span1 < ts2.time + 1 {
        return false;
    }
    true
}

/// Creates a timespan at offset `tick_offset` from `stamp` with length `span`.
pub fn interval(stamp: Ts, tick_offset: u64, span: u64) -> Tss {
    Tss::new(stamp.sid, stamp.time + tick_offset, span)
}

/// Human-readable representation of a timestamp.
pub fn print_ts(id: Ts) -> String {
    if id.sid == SESSION::SERVER {
        return format!(".{}", id.time);
    }
    let s = id.sid.to_string();
    let session = if s.len() > 4 {
        format!("..{}", &s[s.len() - 4..])
    } else {
        s
    };
    format!("{}.{}", session, id.time)
}

// ── LogicalClock ───────────────────────────────────────────────────────────

/// A mutable logical clock that can be ticked.
///
/// Mirrors `LogicalClock` from upstream.
#[derive(Debug, Clone)]
pub struct LogicalClock {
    pub sid: u64,
    pub time: u64,
}

impl LogicalClock {
    pub fn new(sid: u64, time: u64) -> Self {
        Self { sid, time }
    }

    /// Returns the current timestamp and advances the clock by `cycles`.
    pub fn tick(&mut self, cycles: u64) -> Ts {
        let stamp = Ts::new(self.sid, self.time);
        self.time += cycles;
        stamp
    }

    pub fn ts(&self) -> Ts {
        Ts::new(self.sid, self.time)
    }
}

// ── ClockVector ────────────────────────────────────────────────────────────

/// A vector clock: local logical clock plus a map of peer clocks.
///
/// Mirrors `ClockVector` from upstream.
#[derive(Debug, Clone)]
pub struct ClockVector {
    pub sid: u64,
    pub time: u64,
    pub peers: HashMap<u64, Ts>,
}

impl ClockVector {
    pub fn new(sid: u64, time: u64) -> Self {
        Self {
            sid,
            time,
            peers: HashMap::new(),
        }
    }

    pub fn ts(&self) -> Ts {
        Ts::new(self.sid, self.time)
    }

    /// Returns the current timestamp and advances the clock by `cycles`.
    pub fn tick(&mut self, cycles: u64) -> Ts {
        let stamp = Ts::new(self.sid, self.time);
        self.time += cycles;
        stamp
    }

    /// Advance local time whenever we observe a timestamp with a higher value.
    /// Idempotent: calling multiple times is safe.
    pub fn observe(&mut self, id: Ts, span: u64) {
        let edge = id.time + span - 1;
        let sid = id.sid;
        if sid != self.sid {
            self.peers
                .entry(sid)
                .and_modify(|e| {
                    if edge > e.time {
                        e.time = edge;
                    }
                })
                .or_insert_with(|| Ts::new(sid, edge));
        }
        if edge >= self.time {
            self.time = edge + 1;
        }
    }

    /// Deep clone with the same session ID.
    pub fn clone_same(&self) -> ClockVector {
        self.fork(self.sid)
    }

    /// Deep copy with a (potentially different) session ID.
    pub fn fork(&self, new_sid: u64) -> ClockVector {
        let mut clock = ClockVector::new(new_sid, self.time);
        if new_sid != self.sid {
            // Record the last timestamp issued by the old session so the new
            // session knows not to use timestamps before self.time.
            if self.time > 0 {
                clock.observe(Ts::new(self.sid, self.time - 1), 1);
            }
        }
        for (&_sid, &peer) in &self.peers {
            clock.observe(peer, 1);
        }
        clock
    }
}

impl fmt::Display for ClockVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "clock {}.{}", self.sid, self.time)?;
        let peers: Vec<_> = self.peers.values().collect();
        for (i, peer) in peers.iter().enumerate() {
            let is_last = i == peers.len() - 1;
            write!(
                f,
                "\n{} {}.{}",
                if is_last { "└─" } else { "├─" },
                peer.sid,
                peer.time
            )?;
        }
        Ok(())
    }
}

// ── ServerClockVector ──────────────────────────────────────────────────────

/// A clock vector with a fixed server session ID (sid = SESSION::SERVER).
///
/// Used when the CRDT is powered by a central server.
/// Mirrors `ServerClockVector` from upstream.
#[derive(Debug, Clone)]
pub struct ServerClockVector {
    pub sid: u64,
    pub time: u64,
}

impl ServerClockVector {
    pub fn new(time: u64) -> Self {
        Self {
            sid: SESSION::SERVER,
            time,
        }
    }

    pub fn ts(&self) -> Ts {
        Ts::new(self.sid, self.time)
    }

    pub fn tick(&mut self, cycles: u64) -> Ts {
        let stamp = Ts::new(self.sid, self.time);
        self.time += cycles;
        stamp
    }

    /// Observe a timestamp, advancing the clock if necessary.
    ///
    /// Panics if `id.sid > 8` or if `id.time > self.time` (time-travel).
    pub fn observe(&mut self, id: Ts, span: u64) -> Result<(), String> {
        if id.sid > 8 {
            return Err("INVALID_SERVER_SESSION".into());
        }
        if self.time < id.time {
            return Err("TIME_TRAVEL".into());
        }
        let time = id.time + span;
        if time > self.time {
            self.time = time;
        }
        Ok(())
    }

    pub fn fork(&self) -> ServerClockVector {
        ServerClockVector::new(self.time)
    }
}

impl fmt::Display for ServerClockVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "clock {}.{}", self.sid, self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_equality() {
        assert!(equal(ts(1, 100), ts(1, 100)));
        assert!(!equal(ts(1, 100), ts(1, 101)));
        assert!(!equal(ts(1, 100), ts(2, 100)));
    }

    #[test]
    fn ts_compare_time_first() {
        assert_eq!(compare(ts(1, 10), ts(2, 9)), 1);
        assert_eq!(compare(ts(2, 9), ts(1, 10)), -1);
        assert_eq!(compare(ts(1, 10), ts(2, 10)), -1);
        assert_eq!(compare(ts(2, 10), ts(1, 10)), 1);
        assert_eq!(compare(ts(1, 10), ts(1, 10)), 0);
    }

    #[test]
    fn contains_spans() {
        assert!(contains(ts(1, 5), 10, ts(1, 7), 3));
        assert!(!contains(ts(1, 5), 10, ts(2, 7), 3));
        assert!(!contains(ts(1, 5), 3, ts(1, 7), 3));
    }

    #[test]
    fn contains_id_point() {
        assert!(contains_id(ts(1, 5), 10, ts(1, 5)));
        assert!(contains_id(ts(1, 5), 10, ts(1, 14)));
        assert!(!contains_id(ts(1, 5), 10, ts(1, 15)));
        assert!(!contains_id(ts(1, 5), 10, ts(2, 5)));
    }

    #[test]
    fn logical_clock_tick() {
        let mut clock = LogicalClock::new(42, 100);
        let t0 = clock.tick(1);
        assert_eq!(t0, ts(42, 100));
        assert_eq!(clock.time, 101);
        let t1 = clock.tick(3);
        assert_eq!(t1, ts(42, 101));
        assert_eq!(clock.time, 104);
    }

    #[test]
    fn clock_vector_observe() {
        let mut cv = ClockVector::new(1, 0);
        cv.observe(ts(2, 5), 1);
        assert_eq!(cv.time, 6); // advanced local time to edge+1
        assert_eq!(cv.peers[&2].time, 5);
    }

    #[test]
    fn print_ts_server() {
        assert_eq!(print_ts(ts(SESSION::SERVER, 42)), ".42");
    }

    #[test]
    fn print_ts_long_session() {
        assert_eq!(print_ts(ts(123456789, 1)), "..6789.1");
    }

    #[test]
    fn interval_timespan() {
        let span = interval(ts(1, 10), 5, 3);
        assert_eq!(span, Tss::new(1, 15, 3));
    }
}
