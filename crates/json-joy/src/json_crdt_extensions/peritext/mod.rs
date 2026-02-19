//! Peritext rich-text extension.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/`.
//!
//! Peritext is a CRDT-native approach to rich text that separates the RGA
//! character sequence (a [`StrNode`]) from the annotations covering it
//! (slices stored in an [`ArrNode`]).  Characters are referenced by their
//! stable timestamp IDs, so annotations survive concurrent edits without
//! drifting.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use json_joy::json_crdt::model::Model;
//! use json_joy::json_crdt_extensions::peritext::{Peritext, rga::Anchor, slice::SliceStacking};
//!
//! let mut model = Model::new(1);
//! // ... set up StrNode (str_id) and ArrNode (arr_id) in the model ...
//! let peritext = Peritext::new(str_id, arr_id);
//!
//! peritext.ins_at(&mut model, 0, "hello world");
//! let range = peritext.range_at(&model, 6, 5).unwrap(); // "world"
//! peritext.saved_slices.ins_stack(&mut model, &range, "bold", None);
//! assert_eq!(peritext.text(&model), "hello world");
//! ```

pub mod rga;
pub mod slice;

pub use rga::{Anchor, Point, Range};
pub use slice::{Slice, SliceStacking, SliceType, Slices};

use serde_json::Value;

use crate::json_crdt::constants::ORIGIN;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{CrdtNode, IndexExt, TsKey};
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::operations::Op;

// ── Peritext ──────────────────────────────────────────────────────────────

/// Main Peritext context — ties a [`StrNode`] (text) to a [`Slices`]
/// collection (annotations).
///
/// Construct with the IDs of an existing `StrNode` and `ArrNode` in a
/// [`Model`].  All mutation methods apply operations to the model via
/// [`Model::apply_operation`].
#[derive(Debug, Clone, Copy)]
pub struct Peritext {
    /// ID of the `StrNode` holding the text content.
    pub str_id: Ts,

    /// The persisted annotation set (synced to all peers).
    pub saved_slices: Slices,
}

impl Peritext {
    /// Create a Peritext view over an existing StrNode + ArrNode.
    pub fn new(str_id: Ts, arr_id: Ts) -> Self {
        Self {
            str_id,
            saved_slices: Slices::new(arr_id),
        }
    }

    // ── Text queries ──────────────────────────────────────────────────────

    /// Return the current text content as a plain `String`.
    pub fn text(&self, model: &Model) -> String {
        match model.index.get(&TsKey::from(self.str_id)) {
            Some(CrdtNode::Str(s)) => s.view_str(),
            _ => String::new(),
        }
    }

    /// Number of visible characters.
    pub fn len(&self, model: &Model) -> usize {
        match model.index.get(&TsKey::from(self.str_id)) {
            Some(CrdtNode::Str(s)) => s.size(),
            _ => 0,
        }
    }

    // ── Text mutation ─────────────────────────────────────────────────────

    /// Insert `text` so that it starts at visible position `pos`.
    ///
    /// `pos = 0` prepends; `pos = len()` appends.
    pub fn ins_at(&self, model: &mut Model, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let after = {
            match model.index.get(&TsKey::from(self.str_id)) {
                Some(CrdtNode::Str(s)) => {
                    if pos == 0 {
                        ORIGIN
                    } else {
                        s.find(pos - 1).unwrap_or(ORIGIN)
                    }
                }
                _ => return,
            }
        };
        let id = model.next_ts();
        model.apply_operation(&Op::InsStr {
            id,
            obj: self.str_id,
            after,
            data: text.to_string(),
        });
    }

    /// Delete `len` visible characters starting at position `pos`.
    pub fn del_at(&self, model: &mut Model, pos: usize, len: usize) {
        if len == 0 {
            return;
        }
        let spans = {
            match model.index.get(&TsKey::from(self.str_id)) {
                Some(CrdtNode::Str(s)) => s.find_interval(pos, len),
                _ => return,
            }
        };
        if spans.is_empty() {
            return;
        }
        let id = model.next_ts();
        model.apply_operation(&Op::Del {
            id,
            obj: self.str_id,
            what: spans,
        });
    }

    // ── Position helpers ──────────────────────────────────────────────────

    /// Create a [`Point`] at visible position `pos` with the given anchor.
    ///
    /// Returns `None` when `pos` is out of range.
    pub fn point_at(&self, model: &Model, pos: usize, anchor: Anchor) -> Option<Point> {
        match model.index.get(&TsKey::from(self.str_id)) {
            Some(CrdtNode::Str(s)) => s.find(pos).map(|id| Point::new(id, anchor)),
            _ => None,
        }
    }

    /// Create a [`Range`] covering `len` characters starting at visible
    /// position `start`.
    ///
    /// The range uses `Anchor::Before` on the start character and
    /// `Anchor::After` on the last character, matching the upstream's
    /// inclusive-range semantics.
    ///
    /// Returns `None` when `start` or `start + len - 1` is out of range.
    pub fn range_at(&self, model: &Model, start: usize, len: usize) -> Option<Range> {
        if len == 0 {
            let start_point = self.point_at(model, start, Anchor::Before)?;
            return Some(Range::new(start_point, start_point));
        }
        match model.index.get(&TsKey::from(self.str_id)) {
            Some(CrdtNode::Str(s)) => {
                let start_id = s.find(start)?;
                let end_id = s.find(start + len - 1)?;
                Some(Range::new(
                    Point::new(start_id, Anchor::Before),
                    Point::new(end_id, Anchor::After),
                ))
            }
            _ => None,
        }
    }

    /// Convenience: insert a `Many`-stacking slice covering the given range.
    ///
    /// Returns the ID of the new slice's backing `VecNode`.
    pub fn ins_slice(
        &self,
        model: &mut Model,
        range: &Range,
        stacking: SliceStacking,
        slice_type: impl Into<SliceType>,
        data: Option<Value>,
    ) -> Ts {
        self.saved_slices
            .ins(model, range, stacking, slice_type, data)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::constants::ORIGIN as ORIG;
    use crate::json_crdt::model::Model;
    use crate::json_crdt_extensions::peritext::slice::constants::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::Op;

    fn sid() -> u64 {
        42
    }

    /// Build a Model with a StrNode and ArrNode, return (model, peritext).
    fn setup() -> (Model, Peritext) {
        let s = sid();
        let mut model = Model::new(s);

        let str_id = ts(s, 1);
        let arr_id = ts(s, 2);
        model.apply_operation(&Op::NewStr { id: str_id });
        model.apply_operation(&Op::NewArr { id: arr_id });
        // Advance clock past the allocated IDs.
        model.clock.observe(str_id, 1);
        model.clock.observe(arr_id, 1);

        let peritext = Peritext::new(str_id, arr_id);
        (model, peritext)
    }

    // ── Text ──────────────────────────────────────────────────────────────

    #[test]
    fn insert_text_and_view() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        assert_eq!(pt.text(&model), "hello world");
    }

    #[test]
    fn insert_at_position() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "helo");
        pt.ins_at(&mut model, 2, "l"); // "hello"
        assert_eq!(pt.text(&model), "hello");
    }

    #[test]
    fn insert_then_append() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        pt.ins_at(&mut model, 5, " world");
        assert_eq!(pt.text(&model), "hello world");
    }

    #[test]
    fn delete_characters() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        pt.del_at(&mut model, 5, 6); // delete " world"
        assert_eq!(pt.text(&model), "hello");
    }

    #[test]
    fn delete_from_middle() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        pt.del_at(&mut model, 2, 3); // delete "llo"
        assert_eq!(pt.text(&model), "he world");
    }

    #[test]
    fn len_returns_visible_char_count() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        assert_eq!(pt.len(&model), 5);
        pt.del_at(&mut model, 0, 2);
        assert_eq!(pt.len(&model), 3);
    }

    // ── Points ────────────────────────────────────────────────────────────

    #[test]
    fn point_at_returns_correct_position() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let p = pt.point_at(&model, 0, Anchor::Before).unwrap();
        assert_eq!(
            p.view_pos(match model.index.get(&TsKey::from(pt.str_id)) {
                Some(CrdtNode::Str(s)) => s,
                _ => panic!("expected StrNode"),
            }),
            0
        );
    }

    #[test]
    fn point_at_after_anchor() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let p = pt.point_at(&model, 2, Anchor::After).unwrap();
        // After the 3rd char ('l') = view pos 3.
        assert_eq!(p.view_pos(&str_node), 3);
    }

    #[test]
    fn point_at_out_of_range_returns_none() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hi");
        assert!(pt.point_at(&model, 5, Anchor::Before).is_none());
    }

    // ── Ranges ────────────────────────────────────────────────────────────

    #[test]
    fn range_text_extraction() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let range = pt.range_at(&model, 6, 5).unwrap();
        assert_eq!(range.text(&str_node), "world");
    }

    #[test]
    fn range_first_char() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let range = pt.range_at(&model, 0, 1).unwrap();
        assert_eq!(range.text(&str_node), "h");
    }

    #[test]
    fn range_all_chars() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let range = pt.range_at(&model, 0, 5).unwrap();
        assert_eq!(range.text(&str_node), "hello");
    }

    // ── Slices ────────────────────────────────────────────────────────────

    #[test]
    fn insert_slice_and_count() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 0, 5).unwrap();
        pt.ins_slice(&mut model, &range, SliceStacking::Many, "bold", None);
        assert_eq!(pt.saved_slices.size(&model), 1);
    }

    #[test]
    fn insert_two_slices() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let r1 = pt.range_at(&model, 0, 5).unwrap();
        let r2 = pt.range_at(&model, 6, 5).unwrap();
        pt.ins_slice(&mut model, &r1, SliceStacking::Many, "bold", None);
        pt.ins_slice(&mut model, &r2, SliceStacking::Many, "italic", None);
        assert_eq!(pt.saved_slices.size(&model), 2);
    }

    #[test]
    fn slice_type_and_stacking_roundtrip() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 6, 5).unwrap();
        let slice_id = pt.ins_slice(
            &mut model,
            &range,
            SliceStacking::Many,
            "bold",
            Some(serde_json::json!({"bold": true})),
        );

        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.stacking, SliceStacking::Many);
        assert_eq!(slice.slice_type, SliceType::from("bold"));
        assert_eq!(slice.data, Some(serde_json::json!({"bold": true})));
    }

    #[test]
    fn delete_slice_removes_it() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 0, 5).unwrap();
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, "bold", None);
        assert_eq!(pt.saved_slices.size(&model), 1);

        pt.saved_slices.del(&mut model, slice_id);
        assert_eq!(pt.saved_slices.size(&model), 0);
    }

    #[test]
    fn slice_endpoints_match_range() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 6, 5).unwrap();
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, TYPE_BOLD, None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();

        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        // Start should be Before the 'w' at position 6.
        assert_eq!(slice.start.anchor, Anchor::Before);
        assert_eq!(slice.start.view_pos(&str_node), 6);
        // End should be After the 'd' at position 10.
        assert_eq!(slice.end.anchor, Anchor::After);
        assert_eq!(slice.end.view_pos(&str_node), 11);
    }

    #[test]
    fn iter_slices_returns_all_live() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let r1 = pt.range_at(&model, 0, 5).unwrap();
        let r2 = pt.range_at(&model, 6, 5).unwrap();
        pt.ins_slice(&mut model, &r1, SliceStacking::Many, "bold", None);
        pt.ins_slice(&mut model, &r2, SliceStacking::Many, "italic", None);
        let slices = pt.saved_slices.iter_slices(&model);
        assert_eq!(slices.len(), 2);
    }

    #[test]
    fn numeric_slice_type_roundtrip() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 0, 5).unwrap();
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::One, TYPE_BOLD, None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.slice_type, SliceType::from(TYPE_BOLD));
    }

    #[test]
    fn marker_slice_is_collapsed() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello\nworld");
        // Insert a paragraph marker at position 5 (the '\n').
        let range = pt.range_at(&model, 5, 1).unwrap();
        let range_collapsed = Range::new(range.start, range.start);
        let slice_id = pt.ins_slice(
            &mut model,
            &range_collapsed,
            SliceStacking::Marker,
            TYPE_P,
            None,
        );
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert!(slice.is_marker());
        assert!(slice.range().is_collapsed());
    }

    // ── All 4 anchor combinations ──────────────────────────────────────────

    #[test]
    fn anchor_before_before() {
        // start=Before, end=Before — both endpoints are exclusive
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let start_id = str_node.find(1).unwrap();
        let end_id = str_node.find(3).unwrap();
        let range = Range::new(
            Point::new(start_id, Anchor::Before),
            Point::new(end_id, Anchor::Before),
        );
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, "x", None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.start.anchor, Anchor::Before);
        assert_eq!(slice.end.anchor, Anchor::Before);
    }

    #[test]
    fn anchor_before_after() {
        // start=Before, end=After — standard inclusive range
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let range = pt.range_at(&model, 1, 3).unwrap();
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, "bold", None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.start.anchor, Anchor::Before);
        assert_eq!(slice.end.anchor, Anchor::After);
    }

    #[test]
    fn anchor_after_before() {
        // start=After, end=Before — slice shrinks inward from both sides
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let start_id = str_node.find(1).unwrap();
        let end_id = str_node.find(3).unwrap();
        let range = Range::new(
            Point::new(start_id, Anchor::After),
            Point::new(end_id, Anchor::Before),
        );
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, "x", None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.start.anchor, Anchor::After);
        assert_eq!(slice.end.anchor, Anchor::Before);
    }

    #[test]
    fn anchor_after_after() {
        // start=After, end=After — start is exclusive, end is inclusive
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        let start_id = str_node.find(1).unwrap();
        let end_id = str_node.find(3).unwrap();
        let range = Range::new(
            Point::new(start_id, Anchor::After),
            Point::new(end_id, Anchor::After),
        );
        let slice_id = pt.ins_slice(&mut model, &range, SliceStacking::Many, "x", None);
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.start.anchor, Anchor::After);
        assert_eq!(slice.end.anchor, Anchor::After);
    }

    // ── SliceType::Steps roundtrip ────────────────────────────────────────

    #[test]
    fn steps_slice_type_int_roundtrip() {
        use crate::json_crdt_extensions::peritext::slice::types::{SliceType, TypeTag};
        // Steps([TYPE_UL, TYPE_LI]) — a list item inside an unordered list.
        let st = SliceType::Steps(vec![TypeTag::Int(6), TypeTag::Int(9)]);
        let packed = st.to_pack();
        let rt = SliceType::from_pack(&packed).unwrap();
        assert_eq!(rt, st);
    }

    #[test]
    fn steps_slice_type_str_roundtrip() {
        use crate::json_crdt_extensions::peritext::slice::types::{SliceType, TypeTag};
        let st = SliceType::Steps(vec![
            TypeTag::Str("ul".to_string()),
            TypeTag::Str("li".to_string()),
        ]);
        let packed = st.to_pack();
        let rt = SliceType::from_pack(&packed).unwrap();
        assert_eq!(rt, st);
    }

    #[test]
    fn steps_inserted_into_model_and_retrieved() {
        use crate::json_crdt_extensions::peritext::slice::types::{SliceType, TypeTag};
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello world");
        let range = pt.range_at(&model, 0, 5).unwrap();
        let steps = SliceType::Steps(vec![TypeTag::Int(TYPE_UL), TypeTag::Int(TYPE_LI)]);
        let slice_id = pt.ins_slice(
            &mut model,
            &range,
            SliceStacking::Marker,
            steps.clone(),
            None,
        );
        let slice = pt.saved_slices.get(&model, slice_id).unwrap();
        assert_eq!(slice.slice_type, steps);
    }

    // ── Point::view_pos with deleted characters ───────────────────────────

    #[test]
    fn view_pos_before_deleted_char() {
        // When a character is deleted, a Before-anchor point on it returns
        // the position of the surrounding live chars.
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "abc");
        // Delete 'b' (index 1).
        pt.del_at(&mut model, 1, 1);
        // "ac" is now the visible string.
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        // A point on the deleted char should land at position 1 (between a and c).
        // The chunk for 'b' is deleted; view_pos returns the live count before it.
        assert_eq!(str_node.view_str(), "ac");
        // Point at 'b' (still addressable by ID) — deleted chunk returns live count.
        // We verify indirectly: a point on 'a' (After) is position 1.
        let a_id = str_node.find(0).unwrap(); // 'a' is now at index 0
        let after_a = Point::new(a_id, Anchor::After);
        assert_eq!(after_a.view_pos(&str_node), 1);
    }

    #[test]
    fn view_pos_unicode_multibyte() {
        // view_pos counts Unicode scalar values, not bytes.
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "héllo"); // 'é' is 2 bytes (U+00E9)
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        assert_eq!(str_node.view_str(), "héllo");
        assert_eq!(str_node.size(), 5); // 5 chars, not 6 bytes
                                        // The point After the 'é' (index 1) should be at view position 2.
        let e_id = str_node.find(1).unwrap();
        let after_e = Point::new(e_id, Anchor::After);
        assert_eq!(after_e.view_pos(&str_node), 2);
    }

    #[test]
    fn view_pos_emoji_unicode() {
        // Emoji are single scalar values.
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "a😀b"); // emoji is 4 bytes
        let str_node = match model.index.get(&TsKey::from(pt.str_id)) {
            Some(CrdtNode::Str(s)) => s.clone(),
            _ => panic!(),
        };
        assert_eq!(str_node.size(), 3); // 3 chars: 'a', '😀', 'b'
        let emoji_id = str_node.find(1).unwrap();
        let before_emoji = Point::new(emoji_id, Anchor::Before);
        assert_eq!(before_emoji.view_pos(&str_node), 1);
        let after_emoji = Point::new(emoji_id, Anchor::After);
        assert_eq!(after_emoji.view_pos(&str_node), 2);
    }

    // ── Stacking variants ─────────────────────────────────────────────────

    #[test]
    fn stacking_one_roundtrip() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let range = pt.range_at(&model, 0, 5).unwrap();
        let sid = pt.ins_slice(&mut model, &range, SliceStacking::One, "link", None);
        let slice = pt.saved_slices.get(&model, sid).unwrap();
        assert_eq!(slice.stacking, SliceStacking::One);
    }

    #[test]
    fn stacking_erase_roundtrip() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let range = pt.range_at(&model, 0, 5).unwrap();
        let sid = pt.ins_slice(&mut model, &range, SliceStacking::Erase, "bold", None);
        let slice = pt.saved_slices.get(&model, sid).unwrap();
        assert_eq!(slice.stacking, SliceStacking::Erase);
    }

    #[test]
    fn stacking_cursor_roundtrip() {
        let (mut model, pt) = setup();
        pt.ins_at(&mut model, 0, "hello");
        let range = pt.range_at(&model, 2, 1).unwrap();
        let sid = pt.ins_slice(&mut model, &range, SliceStacking::Cursor, TYPE_CURSOR, None);
        let slice = pt.saved_slices.get(&model, sid).unwrap();
        assert_eq!(slice.stacking, SliceStacking::Cursor);
        assert!(slice.is_inline());
    }
}
