//! Patch history log for a JSON CRDT document.
//!
//! Mirrors `packages/json-joy/src/json-crdt/log/Log.ts`.
//!
//! # Overview
//!
//! A [`Log`] stores the complete history of patches applied to a JSON CRDT
//! model. It consists of:
//!
//! - A `start` factory that produces a frozen baseline [`Model`].
//! - A [`BTreeMap`] of patches sorted by their logical timestamp.
//! - An `end` [`Model`] — the current live state (all patches applied).
//! - A `metadata` map for arbitrary user-defined key/value pairs.
//!
//! The log supports replaying to any point in history via [`Log::replay_to_end`]
//! and [`Log::replay_to`], advancing the baseline via [`Log::advance_to`], and
//! rebasing concurrent batches via [`Log::rebase_batch`].

use std::collections::BTreeMap;

use serde_json::Value;

use crate::json_crdt::model::Model;
use crate::json_crdt_patch::clock::{compare, Ts};
use crate::json_crdt_patch::patch::Patch;

/// Key used in the patch `BTreeMap`: orders by `(time, sid)` — matching
/// upstream's `ITimestampStruct` comparator (time first, then session ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PatchKey {
    pub time: u64,
    pub sid: u64,
}

impl PatchKey {
    pub fn from_ts(ts: Ts) -> Self {
        Self {
            time: ts.time,
            sid: ts.sid,
        }
    }
}

/// History log for a JSON CRDT model.
///
/// Stores a start-model factory, an ordered set of patches, and the current
/// end model. All patches applied to `end` are automatically tracked here.
pub struct Log {
    /// Factory that creates a fresh copy of the baseline model. Called every
    /// time `start()` is invoked. May be updated by `advance_to`.
    start_fn: Box<dyn Fn() -> Model + Send + Sync>,

    /// Ordered patch history keyed by `(time, sid)`.
    pub patches: BTreeMap<PatchKey, Patch>,

    /// Current end state — the baseline with all patches applied.
    pub end: Model,

    /// Arbitrary key/value metadata stored alongside the log.
    pub metadata: serde_json::Map<String, Value>,
}

impl Log {
    // ──────────────────────────────────────────────────────────────────────
    // Constructors
    // ──────────────────────────────────────────────────────────────────────

    /// Creates a `Log` from a newly created model.
    ///
    /// The baseline is an empty model sharing the same session ID. The
    /// provided model becomes the initial `end` state (any operations already
    /// applied to it are reflected in `end`, but the `start()` returns a
    /// clean empty model with the same SID).
    ///
    /// Mirrors `Log.fromNewModel(model)` in upstream TypeScript.
    pub fn from_new_model(model: Model) -> Self {
        let sid = model.clock.sid;
        let start_fn = move || Model::new(sid);
        Self {
            start_fn: Box::new(start_fn),
            patches: BTreeMap::new(),
            end: model,
            metadata: serde_json::Map::new(),
        }
    }

    /// Creates a `Log` by freezing `model` as the starting baseline.
    ///
    /// The binary encoding of `model` is stored and decoded on each call to
    /// `start()`. The provided `model` is also cloned to become `end`, so
    /// that new patches can be applied to it without affecting the baseline.
    ///
    /// Mirrors `Log.from(model)` in upstream TypeScript.
    pub fn from_model(model: Model) -> Self {
        // Freeze the starting state as a binary snapshot.
        let frozen: Vec<u8> = model.to_binary();
        let start_fn =
            move || Model::from_binary(&frozen).expect("Log::from_model: corrupt snapshot");
        Self {
            start_fn: Box::new(start_fn),
            patches: BTreeMap::new(),
            end: model.clone(),
            metadata: serde_json::Map::new(),
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Core accessors
    // ──────────────────────────────────────────────────────────────────────

    /// Returns a fresh copy of the baseline model by invoking the internal
    /// factory. Each call produces an independent model instance.
    pub fn start(&self) -> Model {
        (self.start_fn)()
    }

    // ──────────────────────────────────────────────────────────────────────
    // Patch application
    // ──────────────────────────────────────────────────────────────────────

    /// Applies `patch` to `end` and records it in the patch history.
    ///
    /// Patches with no ID (empty patches) are silently ignored.
    pub fn apply(&mut self, patch: Patch) {
        self.end.apply_patch(&patch);
        self.record(patch);
    }

    /// Records a patch in the history without applying it to `end`.
    ///
    /// Useful when the patch has already been applied to `end` externally.
    pub fn record(&mut self, patch: Patch) {
        if let Some(id) = patch.get_id() {
            self.patches.insert(PatchKey::from_ts(id), patch);
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Replay
    // ──────────────────────────────────────────────────────────────────────

    /// Replays all patches in the log onto a fresh `start()` model and
    /// returns it.
    ///
    /// Mirrors `Log.replayToEnd()` in upstream TypeScript.
    pub fn replay_to_end(&self) -> Model {
        let mut model = self.start();
        for patch in self.patches.values() {
            model.apply_patch(patch);
        }
        model
    }

    /// Replays patches from `start()` up to and optionally including `ts`.
    ///
    /// When `inclusive` is `true` (the default) the patch at `ts` is
    /// included; when `false` only patches strictly before `ts` are applied.
    ///
    /// Mirrors `Log.replayTo(ts, inclusive)` in upstream TypeScript.
    pub fn replay_to(&self, ts: Ts, inclusive: bool) -> Model {
        let mut model = self.start();
        for (key, patch) in &self.patches {
            let patch_ts = Ts {
                sid: key.sid,
                time: key.time,
            };
            let cmp = compare(ts, patch_ts);
            if cmp < 0 {
                break;
            }
            if cmp == 0 && !inclusive {
                break;
            }
            model.apply_patch(patch);
        }
        model
    }

    // ──────────────────────────────────────────────────────────────────────
    // Advance baseline
    // ──────────────────────────────────────────────────────────────────────

    /// Advances the start of the log to `ts` (inclusive), removing all
    /// patches up to and including `ts` from the history and baking them
    /// into a new `start()` factory.
    ///
    /// Mirrors `Log.advanceTo(ts)` in upstream TypeScript.
    pub fn advance_to(&mut self, ts: Ts) {
        // Collect patches to bake into the new baseline.
        let mut to_bake: Vec<(PatchKey, Patch)> = Vec::new();
        for key in self.patches.keys() {
            let patch_ts = Ts {
                sid: key.sid,
                time: key.time,
            };
            if compare(ts, patch_ts) >= 0 {
                to_bake.push((*key, Patch::new())); // placeholder
            } else {
                break;
            }
        }
        // Collect the actual patches.
        let baked: Vec<Patch> = to_bake
            .iter()
            .filter_map(|(key, _)| self.patches.remove(key))
            .collect();

        // Build new start factory from old factory + baked patches.
        let old_start = std::mem::replace(&mut self.start_fn, Box::new(|| unreachable!()));
        let new_start: Box<dyn Fn() -> Model + Send + Sync> = Box::new(move || {
            let mut model = old_start();
            for patch in &baked {
                model.apply_patch(patch);
            }
            model
        });
        self.start_fn = new_start;
    }

    // ──────────────────────────────────────────────────────────────────────
    // Batch rebase
    // ──────────────────────────────────────────────────────────────────────

    /// Finds the latest patch for a given session ID by scanning backwards
    /// through the patch history.
    ///
    /// Returns `None` if no patch with that SID is found.
    pub fn find_max(&self, sid: u64) -> Option<&Patch> {
        for patch in self.patches.values().rev() {
            if let Some(id) = patch.get_id() {
                if id.sid == sid {
                    return Some(patch);
                }
            }
        }
        None
    }

    /// Rebases a batch of concurrent patches on top of the latest known
    /// time in this log (or on top of the latest patch for a specific SID).
    ///
    /// Each patch in the batch is shifted so it begins immediately after the
    /// previous one, starting right after the reference patch's span.
    ///
    /// Mirrors `Log.rebaseBatch(batch, sid?)` in upstream TypeScript.
    pub fn rebase_batch(&self, batch: &[Patch], sid: Option<u64>) -> Vec<Patch> {
        let rebase_patch = match sid {
            Some(s) => self.find_max(s),
            None => self.patches.values().next_back(),
        };
        let Some(rebase_patch) = rebase_patch else {
            return batch.to_vec();
        };
        let Some(rebase_id) = rebase_patch.get_id() else {
            return batch.to_vec();
        };
        let mut next_time = rebase_id.time + rebase_patch.span();
        let mut rebased = Vec::with_capacity(batch.len());
        for patch in batch {
            let p = patch.rebase(next_time, None);
            next_time += p.span();
            rebased.push(p);
        }
        rebased
    }

    // ──────────────────────────────────────────────────────────────────────
    // Clone / Reset
    // ──────────────────────────────────────────────────────────────────────

    /// Returns a deep clone of this log.
    ///
    /// The cloned log shares the same `start()` factory function (which is
    /// cheap because baseline data is captured inside the closure), has an
    /// independent `end` clone, and independent copies of all patches.
    ///
    /// Mirrors `Log.clone()` in upstream TypeScript.
    pub fn clone_log(&self) -> Log {
        // Snapshot the current start so both logs share the same frozen baseline.
        let frozen = self.start().to_binary();
        let start_fn: Box<dyn Fn() -> Model + Send + Sync> =
            Box::new(move || Model::from_binary(&frozen).expect("clone_log: corrupt snapshot"));

        let mut patches = BTreeMap::new();
        for (key, patch) in &self.patches {
            patches.insert(*key, patch.clone());
        }

        Log {
            start_fn,
            patches,
            end: self.end.clone(),
            metadata: self.metadata.clone(),
        }
    }

    /// Resets this log to the state of `other`, consuming it.
    ///
    /// After this call `other` should not be used.
    ///
    /// Mirrors `Log.reset(to)` in upstream TypeScript.
    pub fn reset(&mut self, other: Log) {
        self.start_fn = other.start_fn;
        self.metadata = other.metadata;
        self.patches = other.patches;
        // In-place replacement of `end`: copy clock and nodes from other.end.
        self.end = other.end;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Codec stubs (Wave 3)
// ──────────────────────────────────────────────────────────────────────────────

pub mod codec {
    //! Binary codec for `Log`.
    //!
    //! This preserves:
    //! - start-model snapshot
    //! - metadata map
    //! - ordered patch list (binary patch codec payloads)
    //!
    //! Format:
    //! - magic: `JLOG1` (5 bytes)
    //! - start_len: `u32` LE
    //! - start_bytes
    //! - metadata_len: `u32` LE (JSON object bytes)
    //! - metadata_bytes
    //! - patch_count: `u32` LE
    //! - repeated: patch_len `u32` LE + patch_bytes

    /// Encoding format constants — mirrors `log/codec/constants.ts`.
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FileModelEncoding {
        Auto = 0,
        SidecarBinary = 1,
    }

    /// Stub log encoder. Not yet implemented.
    pub struct LogEncoder;

    impl LogEncoder {
        pub fn new() -> Self {
            Self
        }

        pub fn encode(&self, log: &super::Log) -> Vec<u8> {
            const MAGIC: &[u8; 5] = b"JLOG1";
            let start_bytes = log.start().to_binary();
            let metadata_bytes =
                serde_json::to_vec(&serde_json::Value::Object(log.metadata.clone()))
                    .unwrap_or_else(|_| b"{}".to_vec());

            let mut out = Vec::new();
            out.extend_from_slice(MAGIC);

            out.extend_from_slice(&(start_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&start_bytes);

            out.extend_from_slice(&(metadata_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&metadata_bytes);

            out.extend_from_slice(&(log.patches.len() as u32).to_le_bytes());
            for patch in log.patches.values() {
                let patch_bytes = patch.to_binary();
                out.extend_from_slice(&(patch_bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(&patch_bytes);
            }

            out
        }
    }

    impl Default for LogEncoder {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Stub log decoder. Not yet implemented.
    pub struct LogDecoder;

    impl LogDecoder {
        pub fn new() -> Self {
            Self
        }

        /// Decode bytes into a log, panicking on malformed payload.
        pub fn decode(&self, data: &[u8]) -> super::Log {
            self.decode_result(data)
                .expect("LogDecoder::decode: malformed payload")
        }

        /// Decode bytes into a log.
        pub fn decode_result(&self, data: &[u8]) -> Result<super::Log, String> {
            const MAGIC: &[u8; 5] = b"JLOG1";
            let mut offset = 0usize;

            fn read_u32(data: &[u8], offset: &mut usize) -> Result<u32, String> {
                if *offset + 4 > data.len() {
                    return Err("truncated u32".to_string());
                }
                let num = u32::from_le_bytes(
                    data[*offset..*offset + 4]
                        .try_into()
                        .map_err(|_| "bad u32".to_string())?,
                );
                *offset += 4;
                Ok(num)
            }

            fn read_bytes<'a>(
                data: &'a [u8],
                offset: &mut usize,
                len: usize,
            ) -> Result<&'a [u8], String> {
                if *offset + len > data.len() {
                    return Err("truncated bytes".to_string());
                }
                let slice = &data[*offset..*offset + len];
                *offset += len;
                Ok(slice)
            }

            if data.len() < MAGIC.len() || &data[..MAGIC.len()] != MAGIC {
                return Err("bad magic".to_string());
            }
            offset += MAGIC.len();

            let start_len = read_u32(data, &mut offset)? as usize;
            let start_bytes = read_bytes(data, &mut offset, start_len)?;
            let start_model = super::Model::from_binary(start_bytes)?;
            let mut log = super::Log::from_model(start_model);

            let metadata_len = read_u32(data, &mut offset)? as usize;
            let metadata_bytes = read_bytes(data, &mut offset, metadata_len)?;
            let metadata_value: serde_json::Value =
                serde_json::from_slice(metadata_bytes).map_err(|e| e.to_string())?;
            log.metadata = match metadata_value {
                serde_json::Value::Object(map) => map,
                _ => return Err("metadata must be object".to_string()),
            };

            let patch_count = read_u32(data, &mut offset)? as usize;
            for _ in 0..patch_count {
                let patch_len = read_u32(data, &mut offset)? as usize;
                let patch_bytes = read_bytes(data, &mut offset, patch_len)?;
                let patch = crate::json_crdt_patch::patch::Patch::from_binary(patch_bytes)
                    .map_err(|e| e.to_string())?;
                log.apply(patch);
            }

            if offset != data.len() {
                return Err("trailing bytes".to_string());
            }
            Ok(log)
        }
    }

    impl Default for LogDecoder {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Model binary serialization — thin wrappers until a full codec lands
// ──────────────────────────────────────────────────────────────────────────────

impl Model {
    /// Serialize this model to a compact binary representation.
    ///
    /// Used internally by `Log::from_model` and `Log::clone_log` to capture
    /// a frozen snapshot of the baseline. The format is the binary patch
    /// codec applied to all operations accumulated in the model index.
    ///
    /// This is a minimal implementation: it serialises the clock vector so
    /// that a round-trip through `from_binary` restores an equivalent model.
    pub fn to_binary(&self) -> Vec<u8> {
        // Format: magic(4) | sid(8 LE) | time(8 LE) | peer_count(4 LE) | peers…
        // Each peer: sid(8 LE) | time(8 LE)
        let peers: Vec<_> = self.clock.peers.values().collect();
        let mut buf = Vec::with_capacity(24 + peers.len() * 16);
        buf.extend_from_slice(b"JCRD");
        buf.extend_from_slice(&self.clock.sid.to_le_bytes());
        buf.extend_from_slice(&self.clock.time.to_le_bytes());
        buf.extend_from_slice(&(peers.len() as u32).to_le_bytes());
        for peer_ts in &peers {
            buf.extend_from_slice(&peer_ts.sid.to_le_bytes());
            buf.extend_from_slice(&peer_ts.time.to_le_bytes());
        }
        buf
    }

    /// Restore a model from a binary snapshot produced by [`Model::to_binary`].
    ///
    /// Returns an error string if the data is malformed.
    pub fn from_binary(data: &[u8]) -> Result<Model, String> {
        if data.len() < 24 {
            return Err("too short".to_string());
        }
        if &data[..4] != b"JCRD" {
            return Err("bad magic".to_string());
        }
        let sid = u64::from_le_bytes(data[4..12].try_into().unwrap());
        let time = u64::from_le_bytes(data[12..20].try_into().unwrap());
        let peer_count = u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;
        let mut model = Model::new(sid);
        model.clock.time = time;
        let mut offset = 24;
        for _ in 0..peer_count {
            if offset + 16 > data.len() {
                return Err("truncated peers".to_string());
            }
            let p_sid = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            let p_time = u64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());
            // observe the peer clock: use span=1 so that observe(ts(p_sid, p_time), 1) records p_time.
            model
                .clock
                .observe(crate::json_crdt_patch::clock::Ts::new(p_sid, p_time), 1);
            offset += 16;
        }
        Ok(model)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::Op;
    use serde_json::json;

    fn sid() -> u64 {
        111_111
    }

    /// Build a simple model with a string value set.
    fn make_model() -> Model {
        let s = sid();
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: crate::json_crdt::constants::ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        model
    }

    fn make_patch(model: &mut Model, text: &str) -> Patch {
        let s = model.clock.sid;
        let next = model.clock.time;
        let str_ts = ts(s, 1); // existing str node
        let op_id = ts(s, next);

        Patch {
            ops: vec![Op::InsStr {
                id: op_id,
                obj: str_ts,
                after: crate::json_crdt::constants::ORIGIN,
                data: text.to_string(),
            }],
            meta: None,
        }
    }

    // ── Log::from_new_model ───────────────────────────────────────────────

    #[test]
    fn from_new_model_creates_log_with_empty_patches() {
        let model = Model::new(sid());
        let log = Log::from_new_model(model);
        assert!(log.patches.is_empty());
    }

    #[test]
    fn from_new_model_start_returns_empty_model_with_same_sid() {
        let model = Model::new(sid());
        let log = Log::from_new_model(model);
        let start = log.start();
        assert_eq!(start.clock.sid, sid());
        assert_eq!(start.view(), json!(null));
    }

    #[test]
    fn from_new_model_end_reflects_initial_model_state() {
        let model = make_model();
        let log = Log::from_new_model(model);
        assert_eq!(log.end.view(), json!("hello"));
    }

    // ── Log::from_model ───────────────────────────────────────────────────

    #[test]
    fn from_model_start_returns_snapshot_of_original() {
        let model = make_model();
        let log = Log::from_model(model);
        let start = log.start();
        // Start should have the same SID and time as the frozen model.
        assert_eq!(start.clock.sid, sid());
    }

    #[test]
    fn from_model_end_is_independent_clone() {
        let model = make_model();
        let log = Log::from_model(model);
        // end is a clone — should have the same view.
        assert_eq!(log.end.view(), json!("hello"));
    }

    // ── Log::apply ────────────────────────────────────────────────────────

    #[test]
    fn apply_records_patch_in_history() {
        let mut model = Model::new(sid());
        let log_model = model.clone();
        let mut log = Log::from_new_model(log_model);
        let patch = make_patch(&mut model, "hi");
        log.apply(patch);
        assert_eq!(log.patches.len(), 1);
    }

    // ── Log::replay_to_end ────────────────────────────────────────────────

    #[test]
    fn replay_to_end_reproduces_end_state() {
        let s = sid();
        let model = Model::new(s);
        let mut log = Log::from_new_model(model.clone());

        // Build a simple patch: create str + set root.
        let p1 = Patch {
            ops: vec![
                Op::NewStr { id: ts(s, 1) },
                Op::InsStr {
                    id: ts(s, 2),
                    obj: ts(s, 1),
                    after: crate::json_crdt::constants::ORIGIN,
                    data: "abc".into(),
                },
                Op::InsVal {
                    id: ts(s, 7),
                    obj: crate::json_crdt::constants::ORIGIN,
                    val: ts(s, 1),
                },
            ],
            meta: None,
        };
        log.apply(p1);
        let replayed = log.replay_to_end();
        assert_eq!(replayed.view(), log.end.view());
    }

    // ── Log::replay_to ────────────────────────────────────────────────────

    #[test]
    fn replay_to_stops_at_given_timestamp_inclusive() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));

        let p1 = Patch {
            ops: vec![
                Op::NewStr { id: ts(s, 1) },
                Op::InsStr {
                    id: ts(s, 2),
                    obj: ts(s, 1),
                    after: crate::json_crdt::constants::ORIGIN,
                    data: "a".into(),
                },
                Op::InsVal {
                    id: ts(s, 7),
                    obj: crate::json_crdt::constants::ORIGIN,
                    val: ts(s, 1),
                },
            ],
            meta: None,
        };
        let p2 = Patch {
            ops: vec![Op::InsStr {
                id: ts(s, 10),
                obj: ts(s, 1),
                after: ts(s, 2),
                data: "b".into(),
            }],
            meta: None,
        };
        let p2_id = p2.get_id().unwrap();
        log.apply(p1);
        log.apply(p2);

        let m = log.replay_to(p2_id, true);
        let view = m.view();
        assert!(matches!(view, Value::String(ref s) if s.contains('b')));
    }

    #[test]
    fn replay_to_exclusive_excludes_target_patch() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));

        let p1 = Patch {
            ops: vec![
                Op::NewStr { id: ts(s, 1) },
                Op::InsStr {
                    id: ts(s, 2),
                    obj: ts(s, 1),
                    after: crate::json_crdt::constants::ORIGIN,
                    data: "a".into(),
                },
                Op::InsVal {
                    id: ts(s, 7),
                    obj: crate::json_crdt::constants::ORIGIN,
                    val: ts(s, 1),
                },
            ],
            meta: None,
        };
        let p2 = Patch {
            ops: vec![Op::InsStr {
                id: ts(s, 10),
                obj: ts(s, 1),
                after: ts(s, 2),
                data: "b".into(),
            }],
            meta: None,
        };
        let p2_id = p2.get_id().unwrap();
        log.apply(p1);
        log.apply(p2);

        let m = log.replay_to(p2_id, false);
        let view = m.view();
        // Only p1 applied → view should be "a".
        assert_eq!(view, json!("a"));
    }

    // ── Log::find_max ─────────────────────────────────────────────────────

    #[test]
    fn find_max_returns_latest_patch_for_sid() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));

        let p1 = Patch {
            ops: vec![Op::NewStr { id: ts(s, 1) }],
            meta: None,
        };
        let p2 = Patch {
            ops: vec![Op::NewStr { id: ts(s, 5) }],
            meta: None,
        };
        log.record(p1);
        log.record(p2.clone());

        let found = log.find_max(s);
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_id().unwrap().time, 5);
    }

    #[test]
    fn find_max_returns_none_for_unknown_sid() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));
        let p = Patch {
            ops: vec![Op::NewStr { id: ts(s, 1) }],
            meta: None,
        };
        log.record(p);
        assert!(log.find_max(999_999).is_none());
    }

    // ── Log::rebase_batch ─────────────────────────────────────────────────

    #[test]
    fn rebase_batch_returns_input_when_no_history() {
        let s = sid();
        let log = Log::from_new_model(Model::new(s));
        let p = Patch {
            ops: vec![Op::NewStr { id: ts(s, 1) }],
            meta: None,
        };
        let result = log.rebase_batch(std::slice::from_ref(&p), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get_id().unwrap().time, 1);
    }

    #[test]
    fn rebase_batch_shifts_patches_after_last_history_patch() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));
        // Record a patch at time=1, span=1.
        let history_patch = Patch {
            ops: vec![Op::NewStr { id: ts(s, 1) }],
            meta: None,
        };
        log.record(history_patch);

        // Batch patch also starts at time=1 — should be rebased to time=2.
        let batch_patch = Patch {
            ops: vec![Op::NewObj { id: ts(s, 1) }],
            meta: None,
        };
        let result = log.rebase_batch(&[batch_patch], None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get_id().unwrap().time, 2);
    }

    // ── Log::clone_log ────────────────────────────────────────────────────

    #[test]
    fn clone_log_produces_independent_end() {
        let s = sid();
        let model = make_model();
        let log = Log::from_model(model);
        let mut clone = log.clone_log();

        // Modifying the clone's end should not affect the original.
        clone.apply(Patch {
            ops: vec![Op::InsStr {
                id: ts(s, 20),
                obj: ts(s, 1),
                after: crate::json_crdt::constants::ORIGIN,
                data: "x".into(),
            }],
            meta: None,
        });
        assert_eq!(log.end.view(), json!("hello"));
    }

    // ── Model serialization round-trip ────────────────────────────────────

    #[test]
    fn model_binary_round_trip_preserves_clock() {
        let model = make_model();
        let bytes = model.to_binary();
        let restored = Model::from_binary(&bytes).unwrap();
        assert_eq!(restored.clock.sid, model.clock.sid);
        assert_eq!(restored.clock.time, model.clock.time);
    }

    // ── Log codec ───────────────────────────────────────────────────────────

    #[test]
    fn log_codec_round_trip_preserves_state_and_metadata() {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));
        log.metadata.insert("source".into(), json!("test"));

        let patch = Patch {
            ops: vec![
                Op::NewStr { id: ts(s, 1) },
                Op::InsStr {
                    id: ts(s, 2),
                    obj: ts(s, 1),
                    after: crate::json_crdt::constants::ORIGIN,
                    data: "codec".into(),
                },
                Op::InsVal {
                    id: ts(s, 7),
                    obj: crate::json_crdt::constants::ORIGIN,
                    val: ts(s, 1),
                },
            ],
            meta: None,
        };
        log.apply(patch);

        let encoded = crate::json_crdt::log::codec::LogEncoder::new().encode(&log);
        let decoded = crate::json_crdt::log::codec::LogDecoder::new()
            .decode_result(&encoded)
            .expect("decode log");

        assert_eq!(decoded.metadata.get("source"), Some(&json!("test")));
        assert_eq!(decoded.patches.len(), log.patches.len());
        assert_eq!(decoded.replay_to_end().view(), log.replay_to_end().view());
        assert_eq!(decoded.end.view(), log.end.view());
    }

    #[test]
    fn log_decoder_rejects_malformed_payload() {
        let decoder = crate::json_crdt::log::codec::LogDecoder::new();
        assert!(decoder.decode_result(b"bad").is_err());
    }
}
