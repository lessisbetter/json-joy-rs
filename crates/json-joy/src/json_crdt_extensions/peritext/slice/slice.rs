//! The `Slice` type — a rich-text annotation over a [`Range`].
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/Slice.ts`.
//!
//! Each slice is stored in the document model as a `VecNode` (holding the
//! header, endpoint IDs, type, and optional data) referenced by the slices
//! `ArrNode`.

use super::{SliceStacking, SliceType};
use crate::json_crdt_extensions::peritext::rga::{Anchor, Point, Range};
use crate::json_crdt_patch::clock::Ts;
use serde_json::Value;

// ── Slice ─────────────────────────────────────────────────────────────────

/// A rich-text annotation covering a [`Range`] of text.
///
/// Slices are the core annotation primitive in Peritext.  They carry a
/// [`SliceStacking`] that controls how overlapping slices of the same type
/// interact, a [`SliceType`] identifying the kind of annotation (bold,
/// italic, paragraph, …), and optional arbitrary metadata.
#[derive(Debug, Clone)]
pub struct Slice {
    /// The ID of the backing `VecNode` in the document model.
    /// Used to delete or update this slice.
    pub id: Ts,

    /// Stacking behaviour for overlapping slices of the same type.
    pub stacking: SliceStacking,

    /// What this annotation represents.
    pub slice_type: SliceType,

    /// Start position of the annotation.
    pub start: Point,

    /// End position of the annotation.
    pub end: Point,

    /// Optional metadata payload (arbitrary JSON).
    pub data: Option<Value>,
}

impl Slice {
    pub fn new(
        id: Ts,
        stacking: SliceStacking,
        slice_type: SliceType,
        start: Point,
        end: Point,
        data: Option<Value>,
    ) -> Self {
        Self {
            id,
            stacking,
            slice_type,
            start,
            end,
            data,
        }
    }

    /// The range this slice covers.
    pub fn range(&self) -> Range {
        Range::new(self.start, self.end)
    }

    /// `true` if this is a block-split marker (stacking = Marker).
    pub fn is_marker(&self) -> bool {
        self.stacking == SliceStacking::Marker
    }

    /// `true` if the slice covers an inline range (not a collapsed marker).
    pub fn is_inline(&self) -> bool {
        !self.is_marker()
    }
}
