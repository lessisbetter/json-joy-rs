//! Partial edit — lazy-load editing of JSON CRDT documents.
//!
//! Mirrors:
//! - `json-crdt/partial-edit/PartialEdit.ts`
//! - `json-crdt/partial-edit/PartialEditModel.ts`
//! - `json-crdt/partial-edit/PartialEditFactory.ts`
//! - `json-crdt/partial-edit/types.ts`
//!
//! # Overview
//!
//! When a CRDT document is stored in a database one field per node (using the
//! indexed binary codec), it can be very large.  Applying a patch often only
//! needs to touch a few nodes.  `PartialEdit` implements a two-phase protocol:
//!
//! **Phase 1 – discover**
//! Given a `Patch`, call [`PartialEdit::populate_load_list`] to find which
//! indexed fields (nodes) must be loaded from the database before the patch
//! can be applied.  Repeat for every patch you want to apply in the same
//! transaction.
//!
//! **Phase 2 – apply**
//! 1. Load the named fields from storage (caller's responsibility).
//! 2. Call [`PartialEdit::load_partial_model`] with those fields.
//! 3. Call [`PartialEdit::apply_patch`] (once per patch).
//! 4. Call [`PartialEdit::populate_clock_table`] to add any new sessions.
//! 5. Call [`PartialEdit::get_field_edits`] to learn which fields must be
//!    written back to storage.
//!
//! [`PartialEditFactory`] is a thin helper that parses the clock-table blob
//! stored under the `"c"` key and creates a [`PartialEdit`] from it.

use std::collections::{HashMap, HashSet};

use crate::json_crdt::codec::indexed::binary::{self as indexed, DecodeError, IndexedFields};
use crate::json_crdt::model::Model;
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::codec::clock::ClockTable;
use crate::json_crdt_patch::enums::SESSION;
use crate::json_crdt_patch::operations::Op;
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::util::binary::CrdtReader;

// ── Types ──────────────────────────────────────────────────────────────────

/// A set of changes that must be written back to storage after a partial edit.
///
/// Mirrors `FieldEdits` in `types.ts`.
pub struct FieldEdits {
    /// New or updated fields to persist.
    pub updates: IndexedFields,
    /// Field names to delete from storage.
    pub deletes: HashSet<String>,
}

// ── PartialEditModel ───────────────────────────────────────────────────────

/// A [`Model`] variant that tracks nodes deleted during garbage collection.
///
/// In the TypeScript upstream, `PartialEditModel` extends `Model` and overrides
/// `_gcTree` to record deleted node IDs.  In Rust we use composition: the
/// inner `Model` is exposed directly, and `deletes` collects any node IDs
/// that would have been garbage-collected.
///
/// **Note:** The current Rust `Model` implementation does not perform GC, so
/// `deletes` will always be empty.  This is safe — it means partial edits only
/// produce field *updates* and never field *deletes*, which is conservative
/// but correct.
///
/// Mirrors `PartialEditModel.ts`.
pub struct PartialEditModel {
    /// The wrapped CRDT model.
    pub inner: Model,
    /// Node IDs that have been garbage-collected (deleted from the document
    /// tree).  Each entry corresponds to a field that can be removed from the
    /// indexed storage.
    pub deletes: Vec<Ts>,
}

impl PartialEditModel {
    /// Wrap an existing `Model`.
    pub fn new(model: Model) -> Self {
        Self {
            inner: model,
            deletes: Vec::new(),
        }
    }

    /// Delegate patch application to the inner model.
    pub fn apply_patch(&mut self, patch: &Patch) {
        self.inner.apply_patch(patch);
    }
}

// ── PartialEdit ────────────────────────────────────────────────────────────

/// Two-phase partial-edit controller.
///
/// Mirrors `PartialEdit.ts`.
pub struct PartialEdit {
    /// Field names that must be loaded from storage before the patch can be
    /// applied.  Populated by [`populate_load_list`].
    pub load_list: HashSet<String>,
    /// Clock table extracted from the stored `"c"` blob.  Used to map session
    /// IDs to field-name indices.
    pub clock_table: ClockTable,
    /// The partially-loaded model, available after [`load_partial_model`].
    pub doc: Option<PartialEditModel>,
}

impl PartialEdit {
    /// Create a new `PartialEdit` with an empty clock table.
    pub fn new() -> Self {
        Self {
            load_list: HashSet::new(),
            clock_table: ClockTable::new(),
            doc: None,
        }
    }

    /// Create a `PartialEdit` seeded with a pre-parsed `ClockTable`.
    ///
    /// This is the constructor used by [`PartialEditFactory`].
    pub fn with_clock_table(clock_table: ClockTable) -> Self {
        Self {
            load_list: HashSet::new(),
            clock_table,
            doc: None,
        }
    }

    /// Inspect `patch` and add every field name that must be loaded before the
    /// patch can be applied.
    ///
    /// For each operation that targets an existing node (via the `obj` field):
    /// - If `obj.sid == SESSION::SYSTEM` (== 0) the operation targets the
    ///   document root → add `"r"`.
    /// - Otherwise, look up the session in the clock table to obtain its index
    ///   and build `"<sidIdx>_<time>"` in base-36.
    ///
    /// Mirrors `PartialEdit.populateLoadList`.
    pub fn populate_load_list(&mut self, patch: &Patch) {
        for op in &patch.ops {
            if let Some(obj) = op_obj(op) {
                if obj.sid == SESSION::SYSTEM {
                    self.load_list.insert("r".to_string());
                    continue;
                }
                if let Some((idx, _)) = self.clock_table.get_by_sid(obj.sid) {
                    let field_name = format!("{}_{}", to_base36(idx as u64), to_base36(obj.time));
                    self.load_list.insert(field_name);
                }
            }
        }
    }

    /// Returns the set of field names that must be loaded from storage.
    pub fn get_fields_to_load(&self) -> &HashSet<String> {
        &self.load_list
    }

    /// Decode a partial set of indexed fields into an in-memory model and
    /// store it for subsequent [`apply_patch`] / [`get_field_edits`] calls.
    ///
    /// `fields` should be a subset of the full document's fields, containing
    /// only those identified by [`populate_load_list`].  The clock table
    /// (`"c"` key) must be present so the decoder can reconstruct session IDs.
    ///
    /// Mirrors `PartialEdit.loadPartialModel`.
    pub fn load_partial_model(&mut self, fields: &IndexedFields) -> Result<(), DecodeError> {
        let model = indexed::decode(fields)?;
        self.doc = Some(PartialEditModel::new(model));
        Ok(())
    }

    /// Apply `patch` to the loaded model.
    ///
    /// Panics if [`load_partial_model`] has not been called yet.
    ///
    /// Mirrors `PartialEdit.applyPatch`.
    pub fn apply_patch(&mut self, patch: &Patch) {
        self.doc
            .as_mut()
            .expect("model not loaded")
            .apply_patch(patch);
    }

    /// After applying patches, propagate any new sessions from the model's
    /// clock back into `self.clock_table` so they are reflected when encoding
    /// the updated fields.
    ///
    /// Mirrors `PartialEdit.populateClockTable`.
    pub fn populate_clock_table(&mut self) {
        if let Some(doc) = &self.doc {
            let peers = &doc.inner.clock;
            // Add the local session if not already present.
            let local_sid = peers.sid;
            if self.clock_table.get_by_sid(local_sid).is_none() {
                use crate::json_crdt_patch::clock::ts;
                self.clock_table
                    .push(ts(local_sid, peers.time.saturating_sub(1)));
            }
            // Add all peer sessions.
            for (_sid, &peer_ts) in &peers.peers {
                if self.clock_table.get_by_sid(peer_ts.sid).is_none() {
                    self.clock_table.push(peer_ts);
                }
            }
        }
    }

    /// Encode the updated model back to indexed fields and compute which
    /// previously-stored fields should be deleted.
    ///
    /// Returns a [`FieldEdits`] containing:
    /// - `updates`: the full re-encoded set of fields from the updated model.
    /// - `deletes`: fields for every node that was garbage-collected during the
    ///   edit (always empty with the current Rust `Model`, which does not GC).
    ///
    /// Panics if [`load_partial_model`] has not been called yet.
    ///
    /// Mirrors `PartialEdit.getFieldEdits`.
    pub fn get_field_edits(&self) -> FieldEdits {
        let doc = self.doc.as_ref().expect("model not loaded");
        let updates = indexed::encode(&doc.inner);

        // Build the delete set from GC'd node IDs.
        let mut deletes = HashSet::new();
        for id in &doc.deletes {
            if let Some((idx, _)) = self.clock_table.get_by_sid(id.sid) {
                let field_name = format!("{}_{}", idx, to_base36(id.time));
                deletes.insert(field_name);
            }
        }
        FieldEdits { updates, deletes }
    }
}

impl Default for PartialEdit {
    fn default() -> Self {
        Self::new()
    }
}

// ── PartialEditFactory ─────────────────────────────────────────────────────

/// High-level factory that creates a [`PartialEdit`] from the raw clock-table
/// bytes stored under the `"c"` key of an indexed-binary-encoded document.
///
/// Mirrors `PartialEditFactory.ts`.
pub struct PartialEditFactory;

impl PartialEditFactory {
    pub fn new() -> Self {
        Self
    }

    /// Parse the `clock_blob` bytes (the `"c"` field from an
    /// [`IndexedFields`] map) into a [`ClockTable`] and return a ready-to-use
    /// [`PartialEdit`].
    ///
    /// Mirrors `PartialEditFactory.startPartialEdit`.
    pub fn start_partial_edit(&self, clock_blob: &[u8]) -> Result<PartialEdit, DecodeClockError> {
        let clock_table = decode_clock_table_from_bytes(clock_blob)?;
        Ok(PartialEdit::with_clock_table(clock_table))
    }
}

impl Default for PartialEditFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned when the clock blob cannot be parsed.
#[derive(Debug, thiserror::Error)]
pub enum DecodeClockError {
    #[error("empty clock blob")]
    Empty,
    #[error("invalid clock table: {0}")]
    Invalid(String),
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Return the `obj` timestamp for any operation that targets an existing node.
///
/// Operations that *create* nodes (`NewCon`, `NewVal`, …) don't have an `obj`
/// field — they are identified by the absence of a return value here.
fn op_obj(op: &Op) -> Option<Ts> {
    match op {
        Op::InsVal { obj, .. } => Some(*obj),
        Op::InsObj { obj, .. } => Some(*obj),
        Op::InsVec { obj, .. } => Some(*obj),
        Op::InsStr { obj, .. } => Some(*obj),
        Op::InsBin { obj, .. } => Some(*obj),
        Op::InsArr { obj, .. } => Some(*obj),
        Op::UpdArr { obj, .. } => Some(*obj),
        Op::Del { obj, .. } => Some(*obj),
        // Creation ops and Nop don't reference an existing node.
        _ => None,
    }
}

/// Decode the clock table from the raw bytes stored in the `"c"` field.
///
/// Wire format (same as `encode_clock_table` in `binary.rs`):
/// ```text
/// vu57(count)  [vu57(sid) vu57(time)] × count
/// ```
fn decode_clock_table_from_bytes(data: &[u8]) -> Result<ClockTable, DecodeClockError> {
    if data.is_empty() {
        return Err(DecodeClockError::Empty);
    }
    let mut r = CrdtReader::new(data);
    let n = r.vu57() as usize;
    if n == 0 {
        return Err(DecodeClockError::Invalid("count is zero".into()));
    }
    use crate::json_crdt_patch::clock::ts;
    let mut table = ClockTable::new();
    for _ in 0..n {
        let sid = r.vu57();
        let time = r.vu57();
        table.push(ts(sid, time));
    }
    Ok(table)
}

/// Encode `n` as a base-36 string (lowercase), matching the JavaScript
/// `Number.prototype.toString(36)` behaviour.
fn to_base36(n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut result = Vec::new();
    let mut n = n;
    while n > 0 {
        result.push(CHARS[(n % 36) as usize]);
        n /= 36;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::codec::indexed::binary as indexed;
    use crate::json_crdt::constants::ORIGIN;
    use crate::json_crdt::model::Model;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 {
        111222
    }

    // ── PartialEdit unit tests ─────────────────────────────────────────────

    #[test]
    fn partial_edit_new_empty() {
        let pe = PartialEdit::new();
        assert!(pe.load_list.is_empty());
        assert!(pe.clock_table.by_idx.is_empty());
        assert!(pe.doc.is_none());
    }

    #[test]
    fn populate_load_list_root_op() {
        // Build a model, encode it, then derive a clock table from the encoded fields.
        let mut model = Model::new(sid());
        model.apply_operation(&Op::NewCon {
            id: ts(sid(), 1),
            val: ConValue::Val(PackValue::Integer(1)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(sid(), 2),
            obj: ORIGIN,
            val: ts(sid(), 1),
        });
        let fields = indexed::encode(&model);

        let factory = PartialEditFactory::new();
        let clock_blob = fields.get("c").expect("clock field");
        let mut pe = factory.start_partial_edit(clock_blob).expect("parse clock");

        // Build a patch that targets the document root (ORIGIN sid == 0).
        use crate::json_crdt_patch::patch::Patch;
        let mut patch = Patch::new();
        patch.ops.push(Op::InsVal {
            id: ts(sid(), 3),
            obj: ORIGIN, // sid == SESSION::SYSTEM == 0
            val: ts(sid(), 1),
        });

        pe.populate_load_list(&patch);
        assert!(
            pe.load_list.contains("r"),
            "root op must add 'r' to load list"
        );
    }

    #[test]
    fn populate_load_list_node_op() {
        let s = sid();
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let fields = indexed::encode(&model);
        let clock_blob = fields.get("c").expect("clock field");

        let factory = PartialEditFactory::new();
        let mut pe = factory.start_partial_edit(clock_blob).expect("parse clock");

        // A patch inserting into str node ts(s,1).
        let mut patch = Patch::new();
        patch.ops.push(Op::InsStr {
            id: ts(s, 10),
            obj: ts(s, 1), // targets str node
            after: ORIGIN,
            data: "!".to_string(),
        });
        pe.populate_load_list(&patch);

        // The load list should contain a field name for node ts(s, 1).
        // The field name is "<sidIdx>_<time36>" where sidIdx=0 (first entry),
        // time = 1 → base36 = "1".
        assert!(!pe.load_list.is_empty(), "should need to load the str node");
        // Check that exactly one field is queued (the str node).
        let fields_to_load: Vec<&String> = pe.load_list.iter().collect();
        assert_eq!(fields_to_load.len(), 1);
        // The field should be in the full fields map.
        let field_name = fields_to_load[0];
        assert!(
            fields.contains_key(field_name),
            "field {} should exist in encoded document",
            field_name
        );
    }

    #[test]
    fn load_apply_get_edits_roundtrip() {
        let s = sid();
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let fields = indexed::encode(&model);
        assert_eq!(model.view(), serde_json::json!("hello"));

        // Set up the partial edit.
        let factory = PartialEditFactory::new();
        let clock_blob = fields.get("c").expect("clock");
        let mut pe = factory.start_partial_edit(clock_blob).expect("parse clock");

        // Build a patch that appends "!" to the str node.
        let mut patch = Patch::new();
        patch.ops.push(Op::InsStr {
            id: ts(s, 10),
            obj: ts(s, 1),
            after: ts(s, 6), // after last char of "hello" (span 5: ids 2..6)
            data: "!".to_string(),
        });

        // Phase 1: discover.
        pe.populate_load_list(&patch);
        let to_load = pe.get_fields_to_load();
        assert!(!to_load.is_empty());

        // Load just the needed fields (plus the clock).
        let mut partial_fields: IndexedFields = HashMap::new();
        partial_fields.insert("c".to_string(), fields["c"].clone());
        for name in to_load {
            if let Some(bytes) = fields.get(name) {
                partial_fields.insert(name.clone(), bytes.clone());
            }
        }
        if let Some(r) = fields.get("r") {
            partial_fields.insert("r".to_string(), r.clone());
        }

        // Phase 2: apply.
        pe.load_partial_model(&partial_fields).expect("load");
        pe.apply_patch(&patch);
        pe.populate_clock_table();
        let edits = pe.get_field_edits();

        // Merge edits back into the full field set.
        let mut updated_fields = fields.clone();
        for (k, v) in edits.updates {
            updated_fields.insert(k, v);
        }
        for k in &edits.deletes {
            updated_fields.remove(k);
        }

        // The merged document should have "hello!".
        let final_model = indexed::decode(&updated_fields).expect("decode final");
        assert_eq!(final_model.view(), serde_json::json!("hello!"));
    }

    #[test]
    fn field_edits_deletes_empty_when_no_gc() {
        // Since the Rust Model doesn't GC, deletes is always empty.
        let s = sid();
        let mut model = Model::new(s);
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let fields = indexed::encode(&model);
        let clock_blob = fields.get("c").expect("clock");

        let factory = PartialEditFactory::new();
        let mut pe = factory.start_partial_edit(clock_blob).expect("parse clock");
        pe.load_partial_model(&fields).expect("load");
        let edits = pe.get_field_edits();
        assert!(edits.deletes.is_empty(), "no GC in Rust → no field deletes");
    }
}
