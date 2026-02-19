//! Peritext `Point` — a position within an RGA string.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/rga/Point.ts`.
//!
//! A point identifies a location in the text by a stable character ID
//! (timestamp) plus an [`Anchor`] that says whether it attaches to the left
//! (`Before`) or right (`After`) side of that character.  Using IDs instead
//! of integer offsets means points remain valid even as surrounding text is
//! inserted or deleted.

use super::constants::Anchor;
use crate::json_crdt::nodes::StrNode;
use crate::json_crdt_patch::clock::Ts;

// ── Point ─────────────────────────────────────────────────────────────────

/// A stable position inside a [`StrNode`].
///
/// Stores a character's timestamp ID and an anchor side.  All position
/// computations require a reference to the [`StrNode`] whose RGA contains
/// the character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    /// Timestamp of the character this point refers to.
    ///
    /// `Ts { sid: 0, time: 0 }` is the ORIGIN sentinel, representing a
    /// position before the very first character.
    pub id: Ts,
    /// Which side of the character this point attaches to.
    pub anchor: Anchor,
}

impl Point {
    pub fn new(id: Ts, anchor: Anchor) -> Self {
        Self { id, anchor }
    }

    /// `true` if this point is the ORIGIN sentinel (before all characters).
    pub fn is_origin(&self) -> bool {
        self.id.sid == 0 && self.id.time == 0
    }

    /// Number of live (non-deleted) characters that appear *before* this
    /// point in the visible string.
    ///
    /// - `Anchor::Before` → the character at `self.id` is NOT counted.
    /// - `Anchor::After`  → the character at `self.id` IS counted.
    ///
    /// Returns `str_node.size()` when the point is not found (e.g. it is the
    /// ORIGIN sentinel or refers to a character past the end).
    pub fn view_pos(&self, str_node: &StrNode) -> usize {
        if self.is_origin() {
            return 0;
        }

        let mut live = 0usize;
        for chunk in str_node.rga.iter() {
            // Is this the chunk containing our character ID?
            if chunk.id.sid == self.id.sid
                && chunk.id.time <= self.id.time
                && self.id.time < chunk.id.time + chunk.span
            {
                if chunk.deleted {
                    // The character is deleted; return where it would be.
                    return live;
                }
                let char_offset = (self.id.time - chunk.id.time) as usize;
                return match self.anchor {
                    Anchor::Before => live + char_offset,
                    Anchor::After => live + char_offset + 1,
                };
            }
            // Accumulate live characters from this chunk.
            live += chunk.len() as usize;
        }

        // Character not found — treat as absolute end.
        live
    }

    /// Compare two points by their visual position in `str_node`.
    ///
    /// Returns `Ordering::Less` if `self` appears before `other`, etc.
    pub fn cmp_spatial(&self, other: &Point, str_node: &StrNode) -> std::cmp::Ordering {
        self.view_pos(str_node).cmp(&other.view_pos(str_node))
    }
}
