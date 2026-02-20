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
use crate::json_crdt::nodes::{rga::ChunkData, CrdtNode, TsKey};
use crate::json_crdt::schema::to_schema;
use crate::json_crdt_patch::clock::{compare, Ts, Tss};
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;

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

    /// Build an undo patch for `patch` against the current end state.
    ///
    /// Mirrors `Log.undo(patch)` in upstream TypeScript.
    pub fn undo(&self, patch: &Patch) -> Patch {
        let ops = &patch.ops;
        if ops.is_empty() {
            panic!("EMPTY_PATCH");
        }
        let id = patch.get_id().expect("EMPTY_PATCH");
        let mut replay_model: Option<Model> = None;
        let mut builder = PatchBuilder::new(self.end.clock.sid, self.end.clock.time);

        for op in ops.iter().rev() {
            let op_id = op.id();
            match op {
                crate::json_crdt_patch::operations::Op::InsStr { obj, .. }
                | crate::json_crdt_patch::operations::Op::InsArr { obj, .. }
                | crate::json_crdt_patch::operations::Op::InsBin { obj, .. } => {
                    builder.del(*obj, vec![Tss::new(op_id.sid, op_id.time, op.span())]);
                    continue;
                }
                _ => {}
            }

            let model = replay_model.get_or_insert_with(|| self.replay_to(id, false));

            match op {
                crate::json_crdt_patch::operations::Op::InsVal { obj, .. } => {
                    if let Some(CrdtNode::Val(val)) = model.index.get(&TsKey::from(*obj)) {
                        let new_id = if let Some(node) = model.index.get(&TsKey::from(val.val)) {
                            let schema = to_schema(node, &model.index);
                            schema.build(&mut builder)
                        } else {
                            builder.con_val(PackValue::Undefined)
                        };
                        builder.set_val(*obj, new_id);
                    }
                }
                crate::json_crdt_patch::operations::Op::InsObj { obj, data, .. } => {
                    let container = model.index.get(&TsKey::from(*obj));
                    let mut restore: Vec<(String, Ts)> = Vec::with_capacity(data.len());
                    for (key, _) in data {
                        let restored = match container {
                            Some(CrdtNode::Obj(node)) => node
                                .keys
                                .get(key)
                                .and_then(|id| model.index.get(&TsKey::from(*id)))
                                .map(|node| {
                                    let schema = to_schema(node, &model.index);
                                    schema.build(&mut builder)
                                }),
                            _ => None,
                        }
                        .unwrap_or_else(|| builder.con_val(PackValue::Undefined));
                        restore.push((key.clone(), restored));
                    }
                    if !restore.is_empty() {
                        builder.ins_obj(*obj, restore);
                    }
                }
                crate::json_crdt_patch::operations::Op::InsVec { obj, data, .. } => {
                    let container = model.index.get(&TsKey::from(*obj));
                    let mut restore: Vec<(u8, Ts)> = Vec::with_capacity(data.len());
                    for (key, _) in data {
                        let restored = match container {
                            Some(CrdtNode::Vec(node)) => node
                                .elements
                                .get(*key as usize)
                                .and_then(|id| *id)
                                .and_then(|id| model.index.get(&TsKey::from(id)))
                                .map(|node| {
                                    let schema = to_schema(node, &model.index);
                                    schema.build(&mut builder)
                                }),
                            _ => None,
                        }
                        .unwrap_or_else(|| builder.con_val(PackValue::Undefined));
                        restore.push((*key, restored));
                    }
                    if !restore.is_empty() {
                        builder.ins_vec(*obj, restore);
                    }
                }
                crate::json_crdt_patch::operations::Op::Del { obj, what, .. } => {
                    if let Some(node) = model.index.get(&TsKey::from(*obj)) {
                        match node {
                            CrdtNode::Str(str_node) => {
                                let mut restored = String::new();
                                for span in what {
                                    for part in span_view_str(&str_node.rga, *span) {
                                        restored.push_str(&part);
                                    }
                                }
                                let mut after = *obj;
                                if let Some(first_span) = what.first() {
                                    let first = Ts::new(first_span.sid, first_span.time);
                                    if let Some(prev) = prev_id(&str_node.rga, first) {
                                        after = prev;
                                    }
                                }
                                if !restored.is_empty() {
                                    builder.ins_str(*obj, after, restored);
                                }
                            }
                            CrdtNode::Bin(bin_node) => {
                                let mut restored: Vec<u8> = Vec::new();
                                for span in what {
                                    for part in span_view_bin(&bin_node.rga, *span) {
                                        restored.extend(part);
                                    }
                                }
                                let mut after = *obj;
                                if let Some(first_span) = what.first() {
                                    let first = Ts::new(first_span.sid, first_span.time);
                                    if let Some(prev) = prev_id(&bin_node.rga, first) {
                                        after = prev;
                                    }
                                }
                                if !restored.is_empty() {
                                    builder.ins_bin(*obj, after, restored);
                                }
                            }
                            CrdtNode::Arr(arr_node) => {
                                let mut copies: Vec<Ts> = Vec::new();
                                for span in what {
                                    for ids in span_view_arr(&arr_node.rga, *span) {
                                        for id in ids {
                                            if let Some(src) = model.index.get(&TsKey::from(id)) {
                                                let schema = to_schema(src, &model.index);
                                                let new_id = schema.build(&mut builder);
                                                copies.push(new_id);
                                            }
                                        }
                                    }
                                }
                                let mut after = *obj;
                                if let Some(first_span) = what.first() {
                                    let first = Ts::new(first_span.sid, first_span.time);
                                    if let Some(prev) = prev_id(&arr_node.rga, first) {
                                        after = prev;
                                    }
                                }
                                if !copies.is_empty() {
                                    builder.ins_arr(*obj, after, copies);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        builder.flush()
    }
}

fn prev_id<T: Clone + ChunkData>(rga: &crate::json_crdt::nodes::rga::Rga<T>, id: Ts) -> Option<Ts> {
    let mut prev: Option<Ts> = None;
    for chunk in rga.iter() {
        for offset in 0..chunk.span {
            let curr = Ts::new(chunk.id.sid, chunk.id.time + offset);
            if curr == id {
                return prev;
            }
            prev = Some(curr);
        }
    }
    None
}

fn span_view_str(rga: &crate::json_crdt::nodes::rga::Rga<String>, span: Tss) -> Vec<String> {
    let mut view: Vec<String> = Vec::new();
    let mut remaining = span.span as usize;
    let time = span.time;
    let Some(chunk_idx) = rga.find_by_id(Ts::new(span.sid, time)) else {
        return view;
    };
    let mut next = Some(chunk_idx);
    let mut is_first = true;

    while let Some(idx) = next {
        let chunk = rga.slot(idx);
        let chunk_span = chunk.span as usize;
        if !chunk.deleted {
            if is_first {
                let offset = (time - chunk.id.time) as usize;
                if chunk_span >= remaining + offset {
                    if let Some(data) = &chunk.data {
                        view.push(data.chars().skip(offset).take(remaining).collect());
                    }
                    return view;
                }
                if let Some(data) = &chunk.data {
                    let take = chunk_span.saturating_sub(offset);
                    if take > 0 {
                        view.push(data.chars().skip(offset).take(take).collect());
                    }
                }
                remaining = remaining.saturating_sub(chunk_span.saturating_sub(offset));
            } else if chunk_span > remaining {
                if let Some(data) = &chunk.data {
                    view.push(data.chars().take(remaining).collect());
                }
                break;
            } else if let Some(data) = &chunk.data {
                view.push(data.clone());
            }
        }
        remaining = remaining.saturating_sub(chunk_span);
        if remaining == 0 {
            break;
        }
        next = chunk.s;
        is_first = false;
    }

    view
}

fn span_view_bin(rga: &crate::json_crdt::nodes::rga::Rga<Vec<u8>>, span: Tss) -> Vec<Vec<u8>> {
    let mut view: Vec<Vec<u8>> = Vec::new();
    let mut remaining = span.span as usize;
    let time = span.time;
    let Some(chunk_idx) = rga.find_by_id(Ts::new(span.sid, time)) else {
        return view;
    };
    let mut next = Some(chunk_idx);
    let mut is_first = true;

    while let Some(idx) = next {
        let chunk = rga.slot(idx);
        let chunk_span = chunk.span as usize;
        if !chunk.deleted {
            if is_first {
                let offset = (time - chunk.id.time) as usize;
                if chunk_span >= remaining + offset {
                    if let Some(data) = &chunk.data {
                        view.push(data[offset..offset + remaining].to_vec());
                    }
                    return view;
                }
                if let Some(data) = &chunk.data {
                    let take = chunk_span.saturating_sub(offset);
                    if take > 0 {
                        view.push(data[offset..offset + take].to_vec());
                    }
                }
                remaining = remaining.saturating_sub(chunk_span.saturating_sub(offset));
            } else if chunk_span > remaining {
                if let Some(data) = &chunk.data {
                    view.push(data[..remaining].to_vec());
                }
                break;
            } else if let Some(data) = &chunk.data {
                view.push(data.clone());
            }
        }
        remaining = remaining.saturating_sub(chunk_span);
        if remaining == 0 {
            break;
        }
        next = chunk.s;
        is_first = false;
    }

    view
}

fn span_view_arr(rga: &crate::json_crdt::nodes::rga::Rga<Vec<Ts>>, span: Tss) -> Vec<Vec<Ts>> {
    let mut view: Vec<Vec<Ts>> = Vec::new();
    let mut remaining = span.span as usize;
    let time = span.time;
    let Some(chunk_idx) = rga.find_by_id(Ts::new(span.sid, time)) else {
        return view;
    };
    let mut next = Some(chunk_idx);
    let mut is_first = true;

    while let Some(idx) = next {
        let chunk = rga.slot(idx);
        let chunk_span = chunk.span as usize;
        if !chunk.deleted {
            if is_first {
                let offset = (time - chunk.id.time) as usize;
                if chunk_span >= remaining + offset {
                    if let Some(data) = &chunk.data {
                        view.push(data[offset..offset + remaining].to_vec());
                    }
                    return view;
                }
                if let Some(data) = &chunk.data {
                    let take = chunk_span.saturating_sub(offset);
                    if take > 0 {
                        view.push(data[offset..offset + take].to_vec());
                    }
                }
                remaining = remaining.saturating_sub(chunk_span.saturating_sub(offset));
            } else if chunk_span > remaining {
                if let Some(data) = &chunk.data {
                    view.push(data[..remaining].to_vec());
                }
                break;
            } else if let Some(data) = &chunk.data {
                view.push(data.clone());
            }
        }
        remaining = remaining.saturating_sub(chunk_span);
        if remaining == 0 {
            break;
        }
        next = chunk.s;
        is_first = false;
    }

    view
}

// ──────────────────────────────────────────────────────────────────────────────
// Log codec
// ──────────────────────────────────────────────────────────────────────────────

pub mod codec {
    //! Mirrors `packages/json-joy/src/json-crdt/log/codec/*`.

    use std::panic::{catch_unwind, AssertUnwindSafe};

    use json_joy_json_pack::json::{JsonDecoder, JsonEncoder};
    use json_joy_json_pack::{decode_cbor_value_with_consumed, CborEncoder, PackValue};
    use serde_json::Value;

    use crate::json_crdt::codec::sidecar::binary as sidecar_binary;
    use crate::json_crdt::codec::structural::compact as structural_compact;
    use crate::json_crdt::codec::structural::verbose as structural_verbose;
    use crate::json_crdt_patch::codec::compact::decode as patch_decode_compact;
    use crate::json_crdt_patch::codec::compact::encode as patch_encode_compact;
    use crate::json_crdt_patch::codec::verbose::decode as patch_decode_verbose;
    use crate::json_crdt_patch::codec::verbose::encode as patch_encode_verbose;
    use crate::json_crdt_patch::enums::SESSION;
    use crate::json_crdt_patch::patch::Patch;

    /// `log/codec/constants.ts`.
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FileModelEncoding {
        Auto = 0,
        SidecarBinary = 1,
    }

    /// `LogEncoder.SerializeParams`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SerializeParams {
        pub no_view: bool,
        pub model: ModelFormat,
        pub history: HistoryFormat,
    }

    impl Default for SerializeParams {
        fn default() -> Self {
            Self {
                no_view: false,
                model: ModelFormat::Sidecar,
                history: HistoryFormat::Binary,
            }
        }
    }

    /// `LogEncoder.EncodingParams`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EncodingParams {
        pub format: EncodingFormat,
        pub no_view: bool,
        pub model: ModelFormat,
        pub history: HistoryFormat,
    }

    impl Default for EncodingParams {
        fn default() -> Self {
            Self {
                format: EncodingFormat::SeqCbor,
                no_view: false,
                model: ModelFormat::Sidecar,
                history: HistoryFormat::Binary,
            }
        }
    }

    /// `LogDecoder.DeserializeParams`.
    #[derive(Debug, Clone, Default)]
    pub struct DeserializeParams {
        pub view: bool,
        pub sidecar_view: Option<Value>,
        pub frontier: bool,
        pub history: bool,
    }

    /// `LogDecoder.DecodeParams`.
    #[derive(Debug, Clone)]
    pub struct DecodeParams {
        pub format: EncodingFormat,
        pub view: bool,
        pub sidecar_view: Option<Value>,
        pub frontier: bool,
        pub history: bool,
    }

    impl Default for DecodeParams {
        fn default() -> Self {
            Self {
                format: EncodingFormat::SeqCbor,
                view: false,
                sidecar_view: None,
                frontier: false,
                history: false,
            }
        }
    }

    /// High-level wire format.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EncodingFormat {
        Ndjson,
        SeqCbor,
    }

    /// Model encoding format in serialized components.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ModelFormat {
        Sidecar,
        Binary,
        Compact,
        Verbose,
        None,
    }

    /// History encoding format in serialized components.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HistoryFormat {
        Binary,
        Compact,
        Verbose,
        None,
    }

    /// Decoding output.
    #[derive(Default)]
    pub struct DecodeResult {
        pub view: Option<Value>,
        pub frontier: Option<super::Log>,
        pub history: Option<super::Log>,
    }

    fn pack_to_json(pack: &PackValue) -> Value {
        Value::from(pack.clone())
    }

    fn parse_header(
        header: &PackValue,
    ) -> Result<(serde_json::Map<String, Value>, FileModelEncoding), String> {
        let fields = match header {
            PackValue::Array(fields) => fields,
            _ => return Err("INVALID_HEADER".to_string()),
        };
        let metadata = match fields.first() {
            Some(field) => match pack_to_json(field) {
                Value::Object(map) => map,
                _ => return Err("INVALID_HEADER_METADATA".to_string()),
            },
            None => return Err("INVALID_HEADER_METADATA".to_string()),
        };
        let model_encoding = match fields.get(1) {
            Some(PackValue::UInteger(1)) | Some(PackValue::Integer(1)) => {
                FileModelEncoding::SidecarBinary
            }
            _ => FileModelEncoding::Auto,
        };
        Ok((metadata, model_encoding))
    }

    fn parse_history(
        history: Option<&PackValue>,
    ) -> Result<(Option<PackValue>, Vec<PackValue>), String> {
        let Some(history) = history else {
            return Ok((None, Vec::new()));
        };
        let fields = match history {
            PackValue::Array(fields) => fields,
            _ => return Err("INVALID_HISTORY".to_string()),
        };
        let start = fields.first().and_then(|v| match v {
            PackValue::Null => None,
            other => Some(other.clone()),
        });
        let patches = match fields.get(1) {
            None => Vec::new(),
            Some(PackValue::Array(patches)) => patches.clone(),
            Some(_) => return Err("INVALID_HISTORY_PATCHES".to_string()),
        };
        Ok((start, patches))
    }

    pub struct LogEncoder;

    impl LogEncoder {
        pub fn new() -> Self {
            Self
        }

        pub fn serialize(
            &self,
            log: &super::Log,
            params: SerializeParams,
        ) -> Result<Vec<PackValue>, String> {
            let (model_encoding, model_component) = match params.model {
                ModelFormat::Sidecar => {
                    let (_, meta) = sidecar_binary::encode(&log.end);
                    (
                        FileModelEncoding::SidecarBinary as u8,
                        PackValue::Bytes(meta),
                    )
                }
                ModelFormat::Binary => (
                    FileModelEncoding::Auto as u8,
                    PackValue::Bytes(log.end.to_binary()),
                ),
                ModelFormat::Compact => (
                    FileModelEncoding::Auto as u8,
                    PackValue::from(structural_compact::encode(&log.end)),
                ),
                ModelFormat::Verbose => (
                    FileModelEncoding::Auto as u8,
                    PackValue::from(structural_verbose::encode(&log.end)),
                ),
                ModelFormat::None => (FileModelEncoding::Auto as u8, PackValue::Null),
            };

            let header = PackValue::Array(vec![
                PackValue::from(Value::Object(log.metadata.clone())),
                PackValue::UInteger(model_encoding as u64),
            ]);

            let (history_start, history_patches) = match params.history {
                HistoryFormat::Binary => {
                    let start = PackValue::Bytes(log.start().to_binary());
                    let patches = log
                        .patches
                        .values()
                        .map(|patch| PackValue::Bytes(patch.to_binary()))
                        .collect();
                    (start, patches)
                }
                HistoryFormat::Compact => {
                    let start = PackValue::from(structural_compact::encode(&log.start()));
                    let patches = log
                        .patches
                        .values()
                        .map(|patch| {
                            let encoded = patch_encode_compact(patch);
                            PackValue::from(Value::Array(encoded))
                        })
                        .collect();
                    (start, patches)
                }
                HistoryFormat::Verbose => {
                    let start = PackValue::from(structural_verbose::encode(&log.start()));
                    let patches = log
                        .patches
                        .values()
                        .map(|patch| PackValue::from(patch_encode_verbose(patch)))
                        .collect();
                    (start, patches)
                }
                HistoryFormat::None => (PackValue::Null, Vec::new()),
            };

            let history = PackValue::Array(vec![history_start, PackValue::Array(history_patches)]);
            let view = if params.no_view {
                PackValue::Null
            } else {
                PackValue::from(log.end.view())
            };
            Ok(vec![view, header, model_component, history])
        }

        pub fn encode(&self, log: &super::Log, params: EncodingParams) -> Result<Vec<u8>, String> {
            let sequence = self.serialize(
                log,
                SerializeParams {
                    no_view: params.no_view,
                    model: params.model,
                    history: params.history,
                },
            )?;
            match params.format {
                EncodingFormat::Ndjson => {
                    let mut json = JsonEncoder::new();
                    for component in &sequence {
                        json.write_any(component);
                        json.writer.u8(b'\n');
                    }
                    Ok(json.writer.flush())
                }
                EncodingFormat::SeqCbor => {
                    let mut cbor = CborEncoder::new();
                    for component in &sequence {
                        cbor.write_any(component);
                    }
                    Ok(cbor.writer.flush())
                }
            }
        }
    }

    impl Default for LogEncoder {
        fn default() -> Self {
            Self::new()
        }
    }

    pub struct LogDecoder;

    impl LogDecoder {
        pub fn new() -> Self {
            Self
        }

        pub fn decode(&self, blob: &[u8], params: DecodeParams) -> Result<DecodeResult, String> {
            let components = match params.format {
                EncodingFormat::Ndjson => self.decode_ndjson_components(blob)?,
                EncodingFormat::SeqCbor => self.decode_seq_cbor_components(blob)?,
            };
            self.deserialize(
                &components,
                DeserializeParams {
                    view: params.view,
                    sidecar_view: params.sidecar_view,
                    frontier: params.frontier,
                    history: params.history,
                },
            )
        }

        pub fn decode_ndjson_components(&self, blob: &[u8]) -> Result<Vec<PackValue>, String> {
            let mut decoder = JsonDecoder::new();
            let mut components: Vec<PackValue> = Vec::new();
            let mut start = 0usize;
            while start < blob.len() {
                let Some(nl) = blob[start..].iter().position(|b| *b == b'\n') else {
                    return Err("NDJSON_UNEXPECTED_NEWLINE".to_string());
                };
                let end = start + nl;
                let component = decoder
                    .decode(&blob[start..end])
                    .map_err(|e| e.to_string())?;
                components.push(component);
                start = end + 1;
            }
            Ok(components)
        }

        pub fn decode_seq_cbor_components(&self, blob: &[u8]) -> Result<Vec<PackValue>, String> {
            let mut components = Vec::new();
            let mut offset = 0usize;
            while offset < blob.len() {
                let (component, consumed) =
                    decode_cbor_value_with_consumed(&blob[offset..]).map_err(|e| e.to_string())?;
                if consumed == 0 {
                    return Err("SEQ_CBOR_EMPTY_COMPONENT".to_string());
                }
                components.push(component);
                offset += consumed;
            }
            Ok(components)
        }

        pub fn deserialize(
            &self,
            components: &[PackValue],
            params: DeserializeParams,
        ) -> Result<DecodeResult, String> {
            if components.len() < 4 {
                return Err("INVALID_COMPONENTS".to_string());
            }
            let mut view = components[0].clone();
            if matches!(view, PackValue::Null) {
                if let Some(sidecar_view) = params.sidecar_view {
                    view = PackValue::from(sidecar_view);
                }
            }
            let header = &components[1];
            let model = &components[2];

            let mut result = DecodeResult::default();
            if params.view {
                result.view = Some(pack_to_json(&view));
            }
            if params.history {
                result.history = Some(self.deserialize_history(components)?);
            }
            if params.frontier {
                if matches!(model, PackValue::Null) && result.history.is_none() {
                    result.history = Some(self.deserialize_history(components)?);
                }
                if let Some(history) = &result.history {
                    result.frontier = Some(history.clone_log());
                } else if !matches!(model, PackValue::Null) {
                    let (metadata, model_encoding) = parse_header(header)?;
                    let end = if model_encoding == FileModelEncoding::SidecarBinary {
                        let meta = match model {
                            PackValue::Bytes(blob) => blob.as_slice(),
                            _ => return Err("NOT_BLOB".to_string()),
                        };
                        let view_json = pack_to_json(&view);
                        let mut cbor = CborEncoder::new();
                        let view_blob = cbor.encode_json(&view_json);
                        sidecar_binary::decode(&view_blob, meta).map_err(|e| e.to_string())?
                    } else {
                        self.deserialize_model(model)?
                    };
                    let mut log = super::Log::from_model(end);
                    log.metadata = metadata;
                    for patch in components.iter().skip(4) {
                        let patch = self.deserialize_patch(patch)?;
                        log.apply(patch);
                    }
                    result.frontier = Some(log);
                } else {
                    return Err("NO_MODEL".to_string());
                }
            }
            Ok(result)
        }

        pub fn deserialize_history(&self, components: &[PackValue]) -> Result<super::Log, String> {
            let header = components
                .get(1)
                .ok_or_else(|| "INVALID_COMPONENTS".to_string())?;
            let (metadata, _) = parse_header(header)?;
            let (start, patches) = parse_history(components.get(3))?;

            let mut log = match start {
                Some(start) => {
                    let model = self.deserialize_model(&start)?;
                    super::Log::from_model(model)
                }
                None => super::Log::from_new_model(super::Model::new(SESSION::GLOBAL)),
            };
            log.metadata = metadata;

            for patch in patches {
                let patch = self.deserialize_patch(&patch)?;
                log.apply(patch);
            }
            for patch in components.iter().skip(4) {
                let patch = self.deserialize_patch(patch)?;
                log.apply(patch);
            }
            Ok(log)
        }

        pub fn deserialize_model(&self, serialized: &PackValue) -> Result<super::Model, String> {
            match serialized {
                PackValue::Null => Err("NO_MODEL".to_string()),
                PackValue::Bytes(blob) => super::Model::from_binary(blob),
                PackValue::Array(_) => {
                    let json = pack_to_json(serialized);
                    structural_compact::decode(&json).map_err(|e| e.to_string())
                }
                PackValue::Object(_) => {
                    let json = pack_to_json(serialized);
                    structural_verbose::decode(&json).map_err(|e| e.to_string())
                }
                _ => Err("UNKNOWN_MODEL".to_string()),
            }
        }

        pub fn deserialize_patch(&self, serialized: &PackValue) -> Result<Patch, String> {
            match serialized {
                PackValue::Null => Err("NO_PATCH".to_string()),
                PackValue::Bytes(blob) => Patch::from_binary(blob).map_err(|e| e.to_string()),
                PackValue::Array(_) => {
                    let json = pack_to_json(serialized);
                    let arr = json
                        .as_array()
                        .ok_or_else(|| "INVALID_PATCH_COMPACT".to_string())?;
                    catch_unwind(AssertUnwindSafe(|| patch_decode_compact(arr.as_slice())))
                        .map_err(|_| "INVALID_PATCH_COMPACT".to_string())
                }
                PackValue::Object(_) => {
                    let json = pack_to_json(serialized);
                    catch_unwind(AssertUnwindSafe(|| patch_decode_verbose(&json)))
                        .map_err(|_| "INVALID_PATCH_VERBOSE".to_string())
                }
                _ => Err("UNKNOWN_PATCH".to_string()),
            }
        }
    }

    impl Default for LogDecoder {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::log::codec::{
        DecodeParams, DeserializeParams, EncodingFormat, EncodingParams, HistoryFormat, LogDecoder,
        LogEncoder, ModelFormat, SerializeParams,
    };
    use crate::json_crdt_patch::clock::{ts, tss};
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::json::JsonDecoder;
    use json_joy_json_pack::{decode_cbor_value_with_consumed, PackValue};
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

    fn make_base_object_patch(s: u64) -> Patch {
        Patch {
            ops: vec![
                Op::NewObj { id: ts(s, 1) },
                Op::NewCon {
                    id: ts(s, 2),
                    val: ConValue::Val(PackValue::Str("bar".to_string())),
                },
                Op::InsObj {
                    id: ts(s, 3),
                    obj: ts(s, 1),
                    data: vec![("foo".to_string(), ts(s, 2))],
                },
                Op::InsVal {
                    id: ts(s, 4),
                    obj: crate::json_crdt::constants::ORIGIN,
                    val: ts(s, 1),
                },
            ],
            meta: None,
        }
    }

    fn make_frontier_patch(s: u64) -> Patch {
        Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(s, 5),
                    val: ConValue::Val(PackValue::Integer(123)),
                },
                Op::InsObj {
                    id: ts(s, 6),
                    obj: ts(s, 1),
                    data: vec![("xyz".to_string(), ts(s, 5))],
                },
            ],
            meta: None,
        }
    }

    fn setup_log_for_codec() -> Log {
        let s = sid();
        let mut log = Log::from_new_model(Model::new(s));
        log.apply(make_base_object_patch(s));
        log
    }

    fn make_root_str_log(initial: &str) -> (Log, Ts) {
        let s = sid();
        let str_id = ts(s, 1);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewStr { id: str_id });
        if !initial.is_empty() {
            model.apply_operation(&Op::InsStr {
                id: ts(s, 2),
                obj: str_id,
                after: crate::json_crdt::constants::ORIGIN,
                data: initial.to_string(),
            });
        }
        model.apply_operation(&Op::InsVal {
            id: ts(s, model.clock.time),
            obj: crate::json_crdt::constants::ORIGIN,
            val: str_id,
        });
        (Log::from_model(model), str_id)
    }

    fn make_root_bin_log(initial: &[u8]) -> (Log, Ts) {
        let s = sid();
        let bin_id = ts(s, 1);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewBin { id: bin_id });
        if !initial.is_empty() {
            model.apply_operation(&Op::InsBin {
                id: ts(s, 2),
                obj: bin_id,
                after: crate::json_crdt::constants::ORIGIN,
                data: initial.to_vec(),
            });
        }
        model.apply_operation(&Op::InsVal {
            id: ts(s, model.clock.time),
            obj: crate::json_crdt::constants::ORIGIN,
            val: bin_id,
        });
        (Log::from_model(model), bin_id)
    }

    fn make_root_arr_log(initial: &[i64]) -> (Log, Ts, Ts) {
        let s = sid();
        let arr_id = ts(s, 1);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewArr { id: arr_id });
        let mut value_ids = Vec::new();
        for value in initial {
            let id = ts(s, model.clock.time);
            model.apply_operation(&Op::NewCon {
                id,
                val: ConValue::Val(PackValue::Integer(*value)),
            });
            value_ids.push(id);
        }
        let ins_arr_id = if value_ids.is_empty() {
            arr_id
        } else {
            let id = ts(s, model.clock.time);
            model.apply_operation(&Op::InsArr {
                id,
                obj: arr_id,
                after: crate::json_crdt::constants::ORIGIN,
                data: value_ids,
            });
            id
        };
        model.apply_operation(&Op::InsVal {
            id: ts(s, model.clock.time),
            obj: crate::json_crdt::constants::ORIGIN,
            val: arr_id,
        });
        (Log::from_model(model), arr_id, ins_arr_id)
    }

    fn make_root_obj_with_foo_bar() -> (Log, Ts) {
        let s = sid();
        let obj_id = ts(s, 1);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewObj { id: obj_id });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Str("bar".to_string())),
        });
        model.apply_operation(&Op::InsObj {
            id: ts(s, 3),
            obj: obj_id,
            data: vec![("foo".to_string(), ts(s, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: crate::json_crdt::constants::ORIGIN,
            val: obj_id,
        });
        (Log::from_model(model), obj_id)
    }

    fn make_root_vec_with_bar() -> (Log, Ts) {
        let s = sid();
        let vec_id = ts(s, 1);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewVec { id: vec_id });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Str("bar".to_string())),
        });
        model.apply_operation(&Op::InsVec {
            id: ts(s, 3),
            obj: vec_id,
            data: vec![(0u8, ts(s, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: crate::json_crdt::constants::ORIGIN,
            val: vec_id,
        });
        (Log::from_model(model), vec_id)
    }

    fn make_root_arr_with_one_register(value: i64) -> (Log, Ts) {
        let s = sid();
        let arr_id = ts(s, 1);
        let val_id = ts(s, 2);
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewArr { id: arr_id });
        model.apply_operation(&Op::NewVal { id: val_id });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 3),
            val: ConValue::Val(PackValue::Integer(value)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: val_id,
            val: ts(s, 3),
        });
        model.apply_operation(&Op::InsArr {
            id: ts(s, 5),
            obj: arr_id,
            after: crate::json_crdt::constants::ORIGIN,
            data: vec![val_id],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 6),
            obj: crate::json_crdt::constants::ORIGIN,
            val: arr_id,
        });
        (Log::from_model(model), val_id)
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

    // ── Log::undo ──────────────────────────────────────────────────────────

    #[test]
    fn undo_string_insert() {
        let (mut log, str_id) = make_root_str_log("");
        let patch = Patch {
            ops: vec![Op::InsStr {
                id: ts(sid(), log.end.clock.time),
                obj: str_id,
                after: str_id,
                data: "a".to_string(),
            }],
            meta: None,
        };
        log.apply(patch.clone());
        let undo = log.undo(&patch);
        assert_eq!(undo.ops.len(), 1);
        if let Op::Del { what, .. } = &undo.ops[0] {
            assert_eq!(what.len(), 1);
            assert_eq!(what[0].sid, patch.ops[0].id().sid);
            assert_eq!(what[0].time, patch.ops[0].id().time);
            assert_eq!(what[0].span, 1);
        } else {
            panic!("expected del");
        }
        assert_eq!(log.end.view(), json!("a"));
        log.apply(undo);
        assert_eq!(log.end.view(), json!(""));
    }

    #[test]
    fn undo_string_delete() {
        let (mut log, str_id) = make_root_str_log("a");
        let patch = Patch {
            ops: vec![Op::Del {
                id: ts(sid(), log.end.clock.time),
                obj: str_id,
                what: vec![tss(sid(), 2, 1)],
            }],
            meta: None,
        };
        log.apply(patch.clone());
        let undo = log.undo(&patch);
        assert_eq!(undo.ops.len(), 1);
        if let Op::InsStr {
            obj, after, data, ..
        } = &undo.ops[0]
        {
            assert_eq!(data, "a");
            assert_eq!(*obj, str_id);
            assert_eq!(*after, str_id);
        } else {
            panic!("expected ins_str");
        }
        assert_eq!(log.end.view(), json!(""));
        log.apply(undo);
        assert_eq!(log.end.view(), json!("a"));
    }

    #[test]
    fn undo_string_delete_sequence() {
        let (mut log, str_id) = make_root_str_log("12345");
        let patch1 = Patch {
            ops: vec![Op::Del {
                id: ts(sid(), log.end.clock.time),
                obj: str_id,
                what: vec![tss(sid(), 3, 1)],
            }],
            meta: None,
        };
        log.apply(patch1.clone());
        let patch2 = Patch {
            ops: vec![Op::Del {
                id: ts(sid(), log.end.clock.time),
                obj: str_id,
                what: vec![tss(sid(), 4, 2)],
            }],
            meta: None,
        };
        log.apply(patch2.clone());
        let undo2 = log.undo(&patch2);
        let undo1 = log.undo(&patch1);
        assert_eq!(log.end.view(), json!("15"));
        log.apply(undo2);
        assert_eq!(log.end.view(), json!("1345"));
        log.apply(undo1);
        assert_eq!(log.end.view(), json!("12345"));
    }

    #[test]
    fn undo_bin_insert_and_delete() {
        let (mut log, bin_id) = make_root_bin_log(&[]);
        let ins_patch = Patch {
            ops: vec![Op::InsBin {
                id: ts(sid(), log.end.clock.time),
                obj: bin_id,
                after: bin_id,
                data: vec![1, 2, 3],
            }],
            meta: None,
        };
        log.apply(ins_patch.clone());
        let undo_ins = log.undo(&ins_patch);
        log.apply(undo_ins);
        assert_eq!(log.end.view(), json!([]));

        let (mut log, bin_id) = make_root_bin_log(&[1, 2, 3]);
        let del_patch = Patch {
            ops: vec![Op::Del {
                id: ts(sid(), log.end.clock.time),
                obj: bin_id,
                what: vec![tss(sid(), 3, 1)],
            }],
            meta: None,
        };
        log.apply(del_patch.clone());
        assert_eq!(log.end.view(), json!([1, 3]));
        let undo_del = log.undo(&del_patch);
        log.apply(undo_del);
        assert_eq!(log.end.view(), json!([1, 2, 3]));
    }

    #[test]
    fn undo_arr_insert_and_delete() {
        let (mut log, arr_id, _) = make_root_arr_log(&[]);
        let patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(sid(), log.end.clock.time),
                    val: ConValue::Val(PackValue::Integer(1)),
                },
                Op::InsArr {
                    id: ts(sid(), log.end.clock.time + 1),
                    obj: arr_id,
                    after: arr_id,
                    data: vec![ts(sid(), log.end.clock.time)],
                },
            ],
            meta: None,
        };
        log.apply(patch.clone());
        assert_eq!(log.end.view(), json!([1]));
        let undo = log.undo(&patch);
        assert_eq!(undo.ops.len(), 1);
        assert!(matches!(undo.ops[0], Op::Del { .. }));
        log.apply(undo);
        assert_eq!(log.end.view(), json!([]));

        let (mut log, arr_id, ins_arr_id) = make_root_arr_log(&[1, 2, 3]);
        let del_patch = Patch {
            ops: vec![Op::Del {
                id: ts(sid(), log.end.clock.time),
                obj: arr_id,
                what: vec![tss(sid(), ins_arr_id.time + 1, 1)],
            }],
            meta: None,
        };
        log.apply(del_patch.clone());
        assert_eq!(log.end.view(), json!([1, 3]));
        let undo = log.undo(&del_patch);
        log.apply(undo);
        assert_eq!(log.end.view(), json!([1, 2, 3]));
    }

    #[test]
    fn undo_lww_obj_vec_and_val_writes() {
        let (mut log, obj_id) = make_root_obj_with_foo_bar();
        let patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(sid(), log.end.clock.time),
                    val: ConValue::Val(PackValue::Str("baz".to_string())),
                },
                Op::InsObj {
                    id: ts(sid(), log.end.clock.time + 1),
                    obj: obj_id,
                    data: vec![("foo".to_string(), ts(sid(), log.end.clock.time))],
                },
            ],
            meta: None,
        };
        log.apply(patch.clone());
        assert_eq!(log.end.view(), json!({"foo": "baz"}));
        let undo = log.undo(&patch);
        log.apply(undo);
        assert_eq!(log.end.view(), json!({"foo": "bar"}));

        let (mut log, obj_id) = make_root_obj_with_foo_bar();
        let del_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(sid(), log.end.clock.time),
                    val: ConValue::Val(PackValue::Undefined),
                },
                Op::InsObj {
                    id: ts(sid(), log.end.clock.time + 1),
                    obj: obj_id,
                    data: vec![("foo".to_string(), ts(sid(), log.end.clock.time))],
                },
            ],
            meta: None,
        };
        log.apply(del_patch.clone());
        assert_eq!(log.end.view(), json!({}));
        let undo_del = log.undo(&del_patch);
        log.apply(undo_del);
        assert_eq!(log.end.view(), json!({"foo": "bar"}));

        let (mut log, vec_id) = make_root_vec_with_bar();
        let vec_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(sid(), log.end.clock.time),
                    val: ConValue::Val(PackValue::Str("baz".to_string())),
                },
                Op::InsVec {
                    id: ts(sid(), log.end.clock.time + 1),
                    obj: vec_id,
                    data: vec![(0u8, ts(sid(), log.end.clock.time))],
                },
            ],
            meta: None,
        };
        log.apply(vec_patch.clone());
        assert_eq!(log.end.view(), json!(["baz"]));
        let undo_vec = log.undo(&vec_patch);
        log.apply(undo_vec);
        assert_eq!(log.end.view(), json!(["bar"]));

        let (mut log, val_id) = make_root_arr_with_one_register(1);
        let val_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(sid(), log.end.clock.time),
                    val: ConValue::Val(PackValue::Integer(2)),
                },
                Op::InsVal {
                    id: ts(sid(), log.end.clock.time + 1),
                    obj: val_id,
                    val: ts(sid(), log.end.clock.time),
                },
            ],
            meta: None,
        };
        log.apply(val_patch.clone());
        assert_eq!(log.end.view(), json!([2]));
        let undo_val = log.undo(&val_patch);
        log.apply(undo_val);
        assert_eq!(log.end.view(), json!([1]));
    }

    // ── Log codec ───────────────────────────────────────────────────────────

    #[test]
    fn log_encoder_ndjson_first_component_is_view() {
        let log = setup_log_for_codec();
        let encoder = LogEncoder::new();
        let blob = encoder
            .encode(
                &log,
                EncodingParams {
                    format: EncodingFormat::Ndjson,
                    model: ModelFormat::Compact,
                    history: HistoryFormat::Compact,
                    ..EncodingParams::default()
                },
            )
            .expect("encode ndjson");
        let newline = blob
            .iter()
            .position(|b| *b == b'\n')
            .expect("first newline");
        let mut decoder = JsonDecoder::new();
        let first = decoder
            .decode(&blob[..newline])
            .expect("decode first ndjson");
        assert_eq!(Value::from(first), json!({"foo": "bar"}));
    }

    #[test]
    fn log_encoder_seq_cbor_first_component_is_view() {
        let log = setup_log_for_codec();
        let encoder = LogEncoder::new();
        let blob = encoder
            .encode(&log, EncodingParams::default())
            .expect("encode seq.cbor");
        let (first, _) = decode_cbor_value_with_consumed(&blob).expect("decode first cbor");
        assert_eq!(Value::from(first), json!({"foo": "bar"}));
    }

    #[test]
    fn log_decoder_sidecar_model_without_stored_view_uses_sidecar_view() {
        let log = setup_log_for_codec();
        let view = log.end.view();
        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let blob = encoder
            .encode(
                &log,
                EncodingParams {
                    format: EncodingFormat::SeqCbor,
                    model: ModelFormat::Sidecar,
                    history: HistoryFormat::Binary,
                    no_view: true,
                },
            )
            .expect("encode sidecar");
        let decoded = decoder
            .decode(
                &blob,
                DecodeParams {
                    format: EncodingFormat::SeqCbor,
                    frontier: true,
                    sidecar_view: Some(view.clone()),
                    ..DecodeParams::default()
                },
            )
            .expect("decode sidecar");
        assert_eq!(decoded.frontier.expect("frontier").end.view(), view);
    }

    #[test]
    fn log_decoder_decodes_ndjson_frontier_and_history() {
        let log = setup_log_for_codec();
        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let blob = encoder
            .encode(
                &log,
                EncodingParams {
                    format: EncodingFormat::Ndjson,
                    model: ModelFormat::Compact,
                    history: HistoryFormat::Compact,
                    ..EncodingParams::default()
                },
            )
            .expect("encode");
        let decoded = decoder
            .decode(
                &blob,
                DecodeParams {
                    format: EncodingFormat::Ndjson,
                    frontier: true,
                    history: true,
                    ..DecodeParams::default()
                },
            )
            .expect("decode");
        let frontier = decoded.frontier.expect("frontier");
        let history = decoded.history.expect("history");
        assert_eq!(frontier.end.view(), json!({"foo": "bar"}));
        assert_eq!(history.start().view(), json!(null));
        assert_eq!(history.end.view(), json!({"foo": "bar"}));
    }

    #[test]
    fn log_decoder_decodes_seq_cbor_frontier_and_history() {
        let log = setup_log_for_codec();
        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let blob = encoder
            .encode(
                &log,
                EncodingParams {
                    format: EncodingFormat::SeqCbor,
                    model: ModelFormat::Binary,
                    history: HistoryFormat::Binary,
                    ..EncodingParams::default()
                },
            )
            .expect("encode");
        let decoded = decoder
            .decode(
                &blob,
                DecodeParams {
                    format: EncodingFormat::SeqCbor,
                    frontier: true,
                    history: true,
                    ..DecodeParams::default()
                },
            )
            .expect("decode");
        let frontier = decoded.frontier.expect("frontier");
        let history = decoded.history.expect("history");
        assert_eq!(frontier.end.view(), json!({"foo": "bar"}));
        assert_eq!(history.start().view(), json!(null));
        assert_eq!(history.end.view(), json!({"foo": "bar"}));
    }

    #[test]
    fn log_codec_round_trip_preserves_metadata() {
        let mut log = setup_log_for_codec();
        log.metadata.insert("baz".into(), json!("qux"));
        log.metadata.insert("time".into(), json!(123));
        log.metadata.insert("active".into(), json!(true));

        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let blob = encoder
            .encode(
                &log,
                EncodingParams {
                    format: EncodingFormat::SeqCbor,
                    ..EncodingParams::default()
                },
            )
            .expect("encode");
        let decoded = decoder
            .decode(
                &blob,
                DecodeParams {
                    format: EncodingFormat::SeqCbor,
                    frontier: true,
                    history: true,
                    ..DecodeParams::default()
                },
            )
            .expect("decode");
        assert_eq!(decoded.frontier.expect("frontier").metadata, log.metadata);
        assert_eq!(decoded.history.expect("history").metadata, log.metadata);
    }

    fn assert_encoding(log: &Log, params: EncodingParams) {
        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let encoded = encoder.encode(log, params).expect("encode");
        let decoded = decoder
            .decode(
                &encoded,
                DecodeParams {
                    format: params.format,
                    frontier: true,
                    history: true,
                    ..DecodeParams::default()
                },
            )
            .expect("decode");
        let frontier = decoded.frontier.expect("frontier");
        let history = decoded.history.expect("history");
        assert_eq!(frontier.end.view(), log.end.view());
        assert_eq!(history.start().view(), json!(null));
        assert_eq!(history.replay_to_end().view(), log.end.view());
        assert_eq!(history.patches.len(), log.patches.len());
    }

    #[test]
    fn log_codec_all_format_combinations_round_trip() {
        let formats = [EncodingFormat::Ndjson, EncodingFormat::SeqCbor];
        let model_formats = [
            ModelFormat::Sidecar,
            ModelFormat::Binary,
            ModelFormat::Compact,
            ModelFormat::Verbose,
        ];
        let history_formats = [
            HistoryFormat::Binary,
            HistoryFormat::Compact,
            HistoryFormat::Verbose,
        ];
        let no_views = [true, false];
        for format in formats {
            for model in model_formats {
                for history in history_formats {
                    for no_view in no_views {
                        if no_view && model == ModelFormat::Sidecar {
                            continue;
                        }
                        let params = EncodingParams {
                            format,
                            model,
                            history,
                            no_view,
                        };
                        let log = setup_log_for_codec();
                        assert_encoding(&log, params);
                    }
                }
            }
        }
    }

    #[test]
    fn log_decoder_deserialize_applies_frontier() {
        let log = setup_log_for_codec();
        let encoder = LogEncoder::new();
        let decoder = LogDecoder::new();
        let mut serialized = encoder
            .serialize(
                &log,
                SerializeParams {
                    history: HistoryFormat::Binary,
                    ..SerializeParams::default()
                },
            )
            .expect("serialize");
        serialized.push(PackValue::Bytes(make_frontier_patch(sid()).to_binary()));

        let deserialized_frontier = decoder
            .deserialize(
                &serialized,
                DeserializeParams {
                    frontier: true,
                    ..DeserializeParams::default()
                },
            )
            .expect("deserialize frontier");
        let deserialized_history = decoder
            .deserialize(
                &serialized,
                DeserializeParams {
                    history: true,
                    ..DeserializeParams::default()
                },
            )
            .expect("deserialize history");

        assert_eq!(
            deserialized_frontier.frontier.expect("frontier").end.view(),
            json!({"foo": "bar", "xyz": 123})
        );
        assert_eq!(
            deserialized_history.history.expect("history").end.view(),
            json!({"foo": "bar", "xyz": 123})
        );
    }

    #[test]
    fn log_decoder_rejects_malformed_payload() {
        let decoder = LogDecoder::new();
        let err = decoder.decode(
            b"{\"foo\":\"bar\"}",
            DecodeParams {
                format: EncodingFormat::Ndjson,
                ..DecodeParams::default()
            },
        );
        assert!(err.is_err());
    }
}
