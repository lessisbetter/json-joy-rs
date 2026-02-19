//! Draft state machine for a JSON CRDT document.
//!
//! Mirrors `packages/json-joy/src/json-crdt/draft/Draft.ts`.
//!
//! # Terminology
//!
//! - `base`  — committed state (what has been saved / accepted by remote).
//! - `head`  — current editing state (`base` + applied head patches).
//! - `tip`   — future patches queued ahead of `head` (for undo/redo).
//!
//! # Overview
//!
//! A [`Draft`] is a lightweight state machine that tracks the local editing
//! buffer of a JSON CRDT document:
//!
//! 1. `base` starts as a clone of the provided model.
//! 2. Each patch in `head_patches` is applied to `head` so that the head
//!    reflects the user's current edits.
//! 3. `tip` holds future patches not yet applied to `head` (for redo).
//!
//! The `rebase` method applies a batch of remote patches to both `base` and
//! `head`, keeping the editing state up-to-date. The `advance`, `undo`, and
//! `redo` methods are stubs reserved for future implementation.

use crate::json_crdt::model::Model;
use crate::json_crdt_patch::patch::Patch;

/// Draft state machine.
///
/// Maintains two views of the same document:
/// - `base` — the last known committed state.
/// - `head` — the user's current editing state (`base` + uncommitted edits).
pub struct Draft {
    /// The committed base state.
    pub base: Model,

    /// The current editing head (base + head patches applied).
    pub head: Model,

    /// Future patches queued ahead of `head` (undo/redo buffer).
    pub tip: Vec<Patch>,
}

impl Draft {
    /// Creates a new `Draft`.
    ///
    /// - `base` is cloned to produce an independent `head`.
    /// - Each patch in `head_patches` is applied to `head` in order.
    /// - `tip` is stored as-is for future undo/redo use.
    ///
    /// Mirrors `new Draft({ base, head, tip })` in upstream TypeScript.
    pub fn new(base: Model, head_patches: Vec<Patch>, tip: Vec<Patch>) -> Self {
        let mut head = base.clone();
        for patch in &head_patches {
            head.apply_patch(patch);
        }
        Self { base, head, tip }
    }

    /// Applies a batch of remote patches to both `base` and `head`.
    ///
    /// This rebases the editing state onto the new committed ground truth
    /// without touching any uncommitted head patches or the redo tip.
    ///
    /// Mirrors `Draft.rebase(patches)` in upstream TypeScript.
    pub fn rebase(&mut self, patches: &[Patch]) {
        for patch in patches {
            self.base.apply_patch(patch);
            self.head.apply_patch(patch);
        }
    }

    /// Stub — reserved for future implementation.
    ///
    /// Mirrors `Draft.advance(index)` in upstream TypeScript.
    pub fn advance(&mut self, _index: usize) {}

    /// Stub — reserved for future implementation.
    ///
    /// Mirrors `Draft.undo()` in upstream TypeScript.
    pub fn undo(&mut self) {}

    /// Stub — reserved for future implementation.
    ///
    /// Mirrors `Draft.redo()` in upstream TypeScript.
    pub fn redo(&mut self) {}
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::constants::ORIGIN;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;
    use serde_json::json;

    fn sid() -> u64 {
        222_222
    }

    /// Build a model with `{foo: "bar"}`.
    fn make_base() -> Model {
        let s = sid();
        let mut model = Model::new(s);
        // obj node @1
        model.apply_operation(&Op::NewObj { id: ts(s, 1) });
        // str node @2
        model.apply_operation(&Op::NewStr { id: ts(s, 2) });
        // ins_str @3 "bar" into str @2
        model.apply_operation(&Op::InsStr {
            id: ts(s, 3),
            obj: ts(s, 2),
            after: ORIGIN,
            data: "bar".to_string(),
        });
        // ins_obj @6 → obj @1 gets key "foo" → str @2
        model.apply_operation(&Op::InsObj {
            id: ts(s, 6),
            obj: ts(s, 1),
            data: vec![("foo".to_string(), ts(s, 2))],
        });
        // set root to obj @1
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        model
    }

    // ── Draft::new ────────────────────────────────────────────────────────

    #[test]
    fn new_base_and_head_are_independent() {
        let base = make_base();
        let draft = Draft::new(base.clone(), vec![], vec![]);
        assert_eq!(draft.base.view(), json!({"foo": "bar"}));
        assert_eq!(draft.head.view(), json!({"foo": "bar"}));
    }

    #[test]
    fn new_head_has_head_patches_applied() {
        let s = sid();
        let base = make_base();
        // head patch: add key "x" = 1 to obj @1
        let head_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(s, 10),
                    val: ConValue::Val(PackValue::Integer(1)),
                },
                Op::InsObj {
                    id: ts(s, 11),
                    obj: ts(s, 1),
                    data: vec![("x".to_string(), ts(s, 10))],
                },
            ],
            meta: None,
        };
        let draft = Draft::new(base, vec![head_patch], vec![]);
        // base unchanged
        assert_eq!(draft.base.view(), json!({"foo": "bar"}));
        // head has x=1 applied
        assert_eq!(draft.head.view(), json!({"foo": "bar", "x": 1}));
    }

    #[test]
    fn new_tip_is_stored_as_is() {
        let base = make_base();
        let s = sid();
        let tip_patch = Patch {
            ops: vec![Op::NewStr { id: ts(s, 100) }],
            meta: None,
        };
        let draft = Draft::new(base, vec![], vec![tip_patch.clone()]);
        assert_eq!(draft.tip.len(), 1);
        assert_eq!(draft.tip[0].get_id(), tip_patch.get_id());
    }

    // ── Draft::rebase ─────────────────────────────────────────────────────

    #[test]
    fn rebase_applies_patches_to_both_base_and_head() {
        let s = sid();
        let base = make_base();
        let mut draft = Draft::new(base, vec![], vec![]);
        assert_eq!(draft.head.view(), json!({"foo": "bar"}));

        // Remote patch: add "y" = 2 to obj @1.
        let remote_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(s, 20),
                    val: ConValue::Val(PackValue::Integer(2)),
                },
                Op::InsObj {
                    id: ts(s, 21),
                    obj: ts(s, 1),
                    data: vec![("y".to_string(), ts(s, 20))],
                },
            ],
            meta: None,
        };
        draft.rebase(&[remote_patch]);
        assert_eq!(draft.base.view(), json!({"foo": "bar", "y": 2}));
        assert_eq!(draft.head.view(), json!({"foo": "bar", "y": 2}));
    }

    #[test]
    fn rebase_head_retains_local_edits_after_remote_patch() {
        let s = sid();
        let base = make_base();

        // local edit in head: add "x" = 1
        let local_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(s, 10),
                    val: ConValue::Val(PackValue::Integer(1)),
                },
                Op::InsObj {
                    id: ts(s, 11),
                    obj: ts(s, 1),
                    data: vec![("x".to_string(), ts(s, 10))],
                },
            ],
            meta: None,
        };
        let mut draft = Draft::new(base, vec![local_patch], vec![]);
        assert_eq!(draft.head.view(), json!({"foo": "bar", "x": 1}));

        // Remote patch: add "y" = 2
        let remote_patch = Patch {
            ops: vec![
                Op::NewCon {
                    id: ts(s, 20),
                    val: ConValue::Val(PackValue::Integer(2)),
                },
                Op::InsObj {
                    id: ts(s, 21),
                    obj: ts(s, 1),
                    data: vec![("y".to_string(), ts(s, 20))],
                },
            ],
            meta: None,
        };
        draft.rebase(&[remote_patch]);

        // base only has the remote patch
        assert_eq!(draft.base.view(), json!({"foo": "bar", "y": 2}));
        // head has both local and remote patches
        assert_eq!(draft.head.view(), json!({"foo": "bar", "x": 1, "y": 2}));
    }

    // ── Stub methods ──────────────────────────────────────────────────────

    #[test]
    fn advance_undo_redo_do_not_panic() {
        let base = make_base();
        let mut draft = Draft::new(base, vec![], vec![]);
        draft.advance(0);
        draft.undo();
        draft.redo();
        // Still alive with unchanged views.
        assert_eq!(draft.base.view(), json!({"foo": "bar"}));
    }
}
