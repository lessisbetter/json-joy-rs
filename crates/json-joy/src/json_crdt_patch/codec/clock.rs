//! Clock codec: ClockTable, RelativeTimestamp, ClockEncoder, ClockDecoder.
//!
//! Mirrors:
//! - `json-crdt-patch/codec/clock/ClockTable.ts`
//! - `json-crdt-patch/codec/clock/RelativeTimestamp.ts`
//! - `json-crdt-patch/codec/clock/ClockEncoder.ts`
//! - `json-crdt-patch/codec/clock/ClockDecoder.ts`
//!
//! These types are used by codecs to compress session IDs into small integer
//! indices, keeping the wire format compact.

use std::collections::HashMap;

use crate::json_crdt_patch::clock::{ts, ClockVector, Ts};

// ── RelativeTimestamp ─────────────────────────────────────────────────────

/// A timestamp encoded relative to a clock table entry.
///
/// `session_index` is the 1-based index into the clock table (0 is reserved
/// for the system session).  `time_diff` is the difference between the clock's
/// reference time and the encoded timestamp's time.
///
/// Mirrors `RelativeTimestamp` in `RelativeTimestamp.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativeTimestamp {
    /// 1-based index of the session's clock in the clock table.
    pub session_index: u32,
    /// `clock.time - timestamp.time` (non-negative in correct usage).
    pub time_diff: u64,
}

impl RelativeTimestamp {
    pub fn new(session_index: u32, time_diff: u64) -> Self {
        Self {
            session_index,
            time_diff,
        }
    }
}

// ── ClockTable ────────────────────────────────────────────────────────────

/// A flat ordered table of clock entries used during decoding.
///
/// Maps a 0-based position in `by_idx` to a reference `Ts`.
/// Also provides reverse lookup via `by_sid`.
///
/// Mirrors `ClockTable` in `ClockTable.ts`.
#[derive(Debug, Clone, Default)]
pub struct ClockTable {
    /// Entries in insertion order; position = index used on the wire.
    pub by_idx: Vec<Ts>,
    /// Session-ID → index into `by_idx`.
    pub by_sid: HashMap<u64, usize>,
}

impl ClockTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a ClockTable from a ClockVector, mirroring `ClockTable.from`.
    ///
    /// The local session is placed at index 0 with `time - 1`; all peers
    /// follow in insertion order.
    pub fn from_clock(clock: &ClockVector) -> Self {
        let mut table = Self::new();
        // Local session: stored at time - 1 (the last *issued* time).
        table.push(Ts::new(clock.sid, clock.time.saturating_sub(1)));
        // Peer sessions.
        for peer_ts in clock.peers.values() {
            table.push(*peer_ts);
        }
        table
    }

    /// Append a clock entry and record its index.
    pub fn push(&mut self, id: Ts) {
        let index = self.by_idx.len();
        self.by_sid.insert(id.sid, index);
        self.by_idx.push(id);
    }

    /// Get the reference timestamp at `index`, or `None` if out of bounds.
    pub fn get_by_index(&self, index: usize) -> Option<Ts> {
        self.by_idx.get(index).copied()
    }

    /// Get the entry for a session ID, or `None` if not present.
    pub fn get_by_sid(&self, sid: u64) -> Option<(usize, Ts)> {
        self.by_sid.get(&sid).map(|&idx| (idx, self.by_idx[idx]))
    }
}

// ── ClockEncoder ──────────────────────────────────────────────────────────

/// Encodes logical timestamps to relative form using a per-session clock table.
///
/// Must be `reset()` with the local clock before encoding any timestamps.
///
/// Mirrors `ClockEncoder` in `ClockEncoder.ts`.
#[derive(Debug, Clone, Default)]
pub struct ClockEncoder {
    /// Map: session_id → (1-based index, reference clock Ts).
    table: HashMap<u64, (u32, Ts)>,
    /// Next index to assign (starts at 1; index 0 is reserved for system).
    next_index: u32,
    /// The local ClockVector at encode time.
    clock: Option<ClockVector>,
}

impl ClockEncoder {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
            next_index: 1,
            clock: None,
        }
    }

    /// Initialise the encoder for a new document.
    ///
    /// The local session is registered at index 1 with its last-issued time
    /// (`clock.time - 1`).  This matches `ClockEncoder.reset()` upstream.
    pub fn reset(&mut self, clock: &ClockVector) {
        self.table.clear();
        self.next_index = 1;
        // The local session occupies slot 1 at `tick(clock, -1)`.
        // tick(clock, -1) in TS means clock.time - 1.
        let local_ref = Ts::new(clock.sid, clock.time.saturating_sub(1));
        let idx = self.next_index;
        self.next_index += 1;
        self.table.insert(clock.sid, (idx, local_ref));
        self.clock = Some(clock.clone());
    }

    /// Encode a timestamp to a `RelativeTimestamp`.
    ///
    /// Returns an error string if the timestamp is newer than its session's
    /// reference clock (which would indicate a clock going backwards).
    pub fn append(&mut self, stamp: Ts) -> Result<RelativeTimestamp, &'static str> {
        let sid = stamp.sid;
        let time = stamp.time;

        // Look up or lazily create an entry for this session.
        let (idx, ref_ts) = if let Some(entry) = self.table.get(&sid) {
            *entry
        } else {
            // Find this session in the peer map, or fall back to local time.
            let clock = self.clock.as_ref().ok_or("encoder not reset")?;
            let peer_ref = clock
                .peers
                .get(&sid)
                .copied()
                .unwrap_or_else(|| Ts::new(sid, clock.time.saturating_sub(1)));
            let idx = self.next_index;
            self.next_index += 1;
            self.table.insert(sid, (idx, peer_ref));
            (idx, peer_ref)
        };

        // time_diff = ref_ts.time - stamp.time; must be non-negative.
        if time > ref_ts.time {
            return Err("TIME_TRAVEL");
        }
        let time_diff = ref_ts.time - time;
        Ok(RelativeTimestamp::new(idx, time_diff))
    }

    /// Serialise the clock table as a flat `[sid, time, sid, time, ...]` array.
    ///
    /// Entries are sorted by their assigned index so the decoder can rebuild
    /// the table deterministically.  Mirrors `ClockEncoder.toJson()`.
    pub fn to_json(&self) -> Vec<u64> {
        // Collect (index, sid, time) tuples and sort by index.
        let mut entries: Vec<(u32, u64, u64)> = self
            .table
            .iter()
            .map(|(&sid, &(idx, ref_ts))| (idx, sid, ref_ts.time))
            .collect();
        entries.sort_by_key(|(idx, _, _)| *idx);

        let mut out = Vec::with_capacity(entries.len() * 2);
        for (_idx, sid, time) in entries {
            out.push(sid);
            out.push(time);
        }
        out
    }
}

// ── ClockDecoder ──────────────────────────────────────────────────────────

/// Decodes relative timestamps back to absolute logical timestamps.
///
/// Mirrors `ClockDecoder` in `ClockDecoder.ts`.
#[derive(Debug, Clone)]
pub struct ClockDecoder {
    /// 1-based table; index 0 maps to the system session.
    /// `table[i - 1]` gives the reference Ts for session index `i`.
    table: Vec<Ts>,
    /// Reconstructed clock vector.
    pub clock: ClockVector,
}

impl ClockDecoder {
    /// Create a decoder seeded with the primary session.
    ///
    /// `sid` + `time` are the reference timestamp for session index 1.
    /// The reconstructed `clock.time = time + 1` (next available time).
    ///
    /// Mirrors `new ClockDecoder(sid, time)` in the upstream.
    pub fn new(sid: u64, time: u64) -> Self {
        let clock = ClockVector::new(sid, time + 1);
        let mut decoder = Self {
            table: Vec::new(),
            clock,
        };
        decoder.table.push(ts(sid, time));
        decoder
    }

    /// Build a decoder from a flat `[sid, time, ...]` array.
    ///
    /// Mirrors `ClockDecoder.fromArr`.
    pub fn from_arr(arr: &[u64]) -> Option<Self> {
        if arr.len() < 2 {
            return None;
        }
        let mut decoder = Self::new(arr[0], arr[1]);
        let mut i = 2;
        while i + 1 < arr.len() {
            decoder.push_tuple(arr[i], arr[i + 1]);
            i += 2;
        }
        Some(decoder)
    }

    /// Append a peer clock entry.
    ///
    /// Mirrors `ClockDecoder.pushTuple`.
    pub fn push_tuple(&mut self, sid: u64, time: u64) {
        let id = ts(sid, time);
        self.clock.observe(id, 1);
        self.table.push(id);
    }

    /// Decode a `(session_index, time_diff)` pair back to a `Ts`.
    ///
    /// - `session_index == 0` → system session: returns `ts(0, time_diff)`.
    /// - Otherwise: looks up `table[session_index - 1]` and returns
    ///   `ts(clock.sid, clock.time - time_diff)`.
    ///
    /// Returns `None` if `session_index` is out of range.
    ///
    /// Mirrors `ClockDecoder.decodeId`.
    pub fn decode_id(&self, session_index: u32, time_diff: u64) -> Option<Ts> {
        if session_index == 0 {
            return Some(ts(0, time_diff));
        }
        let clock = self.table.get((session_index - 1) as usize)?;
        Some(ts(clock.sid, clock.time - time_diff))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::{ts, ClockVector};

    // ── ClockEncoder tests (mirrors ClockEncoder.spec.ts) ─────────────────

    #[test]
    fn encoder_always_encodes_default_clock() {
        // const clock = new ClockVector(123, 5); clock.observe(ts(123,5),1);
        let mut clock = ClockVector::new(123, 5);
        clock.observe(ts(123, 5), 1);
        let mut encoder = ClockEncoder::new();
        encoder.reset(&clock);
        let encoded = encoder.to_json();
        // upstream stores tick(clock, -1) = ts(123, 5-1=4) but toJson shows
        // the raw values: sid=123, time = clock.time-1 = 5
        // Actually: clock.time after observe(ts(123,5),1) = max(5, 5+1-1)+1 = 6
        // then tick(clock,-1) = ts(123, 6-1=5).
        // But upstream test expects [123, 5].
        // reset stores tick(clock, -1): clock.time=6 → time-1=5 → [123, 5]. ✓
        assert_eq!(encoded, vec![123, 5]);
    }

    #[test]
    fn encoder_encodes_default_clock_as_first() {
        let mut clock = ClockVector::new(3, 10);
        clock.observe(ts(3, 10), 1);
        let stamp = ts(2, 5);
        clock.observe(stamp, 1);
        let mut encoder = ClockEncoder::new();
        encoder.reset(&clock);
        encoder.append(stamp).unwrap();
        let encoded = encoder.to_json();
        // clock.time after both observes = max(10+1, 5+1) = 11
        // local ref = time - 1 = 10 → [3, 10, ...]
        // peer stamp ts(2,5): peer ref from peers map = ts(2, 5), time_diff=0
        assert_eq!(encoded, vec![3, 10, 2, 5]);
    }

    #[test]
    fn encoder_does_not_encode_unappended_clocks() {
        let mut clock = ClockVector::new(3, 10);
        clock.observe(ts(3, 10), 1);
        let stamp = ts(2, 5);
        clock.observe(stamp, 1);
        let mut encoder = ClockEncoder::new();
        encoder.reset(&clock);
        // Do NOT append stamp — it should not appear in output.
        let encoded = encoder.to_json();
        assert_eq!(encoded, vec![3, 10]);
    }

    #[test]
    fn encoder_encodes_each_clock_only_once() {
        let mut clock = ClockVector::new(100, 100);
        clock.observe(ts(100, 100), 1);
        let ts1 = ts(50, 50);
        let ts2 = ts(10, 10);
        clock.observe(ts1, 1);
        clock.observe(ts2, 1);
        let mut encoder = ClockEncoder::new();
        encoder.reset(&clock);
        encoder.append(ts1).unwrap();
        encoder.append(ts2).unwrap();
        encoder.append(ts(10, 6)).unwrap();
        encoder.append(ts(10, 3)).unwrap();
        encoder.append(ts(50, 34)).unwrap();
        let encoded = encoder.to_json();
        assert_eq!(encoded, vec![100, 100, 50, 50, 10, 10]);
    }

    // ── ClockDecoder tests ────────────────────────────────────────────────

    #[test]
    fn decoder_from_arr_basic() {
        let decoder = ClockDecoder::from_arr(&[123, 5]).unwrap();
        assert_eq!(decoder.clock.sid, 123);
        assert_eq!(decoder.clock.time, 6); // time + 1
    }

    #[test]
    fn decoder_decode_id_session_zero() {
        let decoder = ClockDecoder::new(123, 5);
        // session_index=0 → system session
        let result = decoder.decode_id(0, 42).unwrap();
        assert_eq!(result, ts(0, 42));
    }

    #[test]
    fn decoder_decode_id_primary_session() {
        let decoder = ClockDecoder::new(123, 10);
        // session_index=1, time_diff=3 → ts(123, 10-3=7)
        let result = decoder.decode_id(1, 3).unwrap();
        assert_eq!(result, ts(123, 7));
    }

    #[test]
    fn decoder_decode_id_out_of_range() {
        let decoder = ClockDecoder::new(123, 10);
        assert!(decoder.decode_id(2, 0).is_none());
    }

    #[test]
    fn decoder_push_tuple_and_decode() {
        let mut decoder = ClockDecoder::new(100, 100);
        decoder.push_tuple(50, 50);
        // session_index=2, time_diff=5 → ts(50, 50-5=45)
        let result = decoder.decode_id(2, 5).unwrap();
        assert_eq!(result, ts(50, 45));
    }

    // ── ClockTable tests ─────────────────────────────────────────────────

    #[test]
    fn clock_table_push_and_lookup() {
        let mut table = ClockTable::new();
        table.push(ts(10, 5));
        table.push(ts(20, 8));
        assert_eq!(table.get_by_index(0), Some(ts(10, 5)));
        assert_eq!(table.get_by_index(1), Some(ts(20, 8)));
        assert_eq!(table.get_by_index(2), None);
    }

    #[test]
    fn clock_table_get_by_sid() {
        let mut table = ClockTable::new();
        table.push(ts(10, 5));
        table.push(ts(20, 8));
        let (idx, clock) = table.get_by_sid(20).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(clock, ts(20, 8));
        assert!(table.get_by_sid(99).is_none());
    }

    #[test]
    fn clock_table_from_clock() {
        let mut clock = ClockVector::new(42, 10);
        clock.observe(ts(7, 3), 1);
        let table = ClockTable::from_clock(&clock);
        assert_eq!(table.by_idx[0], ts(42, 9)); // time - 1
    }

    // ── RelativeTimestamp tests ───────────────────────────────────────────

    #[test]
    fn relative_timestamp_fields() {
        let rt = RelativeTimestamp::new(2, 5);
        assert_eq!(rt.session_index, 2);
        assert_eq!(rt.time_diff, 5);
    }

    // ── Round-trip test ──────────────────────────────────────────────────

    #[test]
    fn encode_decode_roundtrip() {
        let mut clock = ClockVector::new(100, 20);
        clock.observe(ts(100, 20), 1);
        clock.observe(ts(50, 15), 1);

        let mut encoder = ClockEncoder::new();
        encoder.reset(&clock);

        let rel1 = encoder.append(ts(100, 18)).unwrap();
        let rel2 = encoder.append(ts(50, 12)).unwrap();

        let flat = encoder.to_json();
        let decoder = ClockDecoder::from_arr(&flat).unwrap();

        let decoded1 = decoder
            .decode_id(rel1.session_index, rel1.time_diff)
            .unwrap();
        let decoded2 = decoder
            .decode_id(rel2.session_index, rel2.time_diff)
            .unwrap();

        assert_eq!(decoded1, ts(100, 18));
        assert_eq!(decoded2, ts(50, 12));
    }
}
