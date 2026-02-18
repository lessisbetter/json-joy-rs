//! Peritext `Range` — an ordered pair of [`Point`]s delimiting a text selection.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/rga/Range.ts`.

use crate::json_crdt::nodes::StrNode;
use super::{Point, Anchor};

// ── Range ─────────────────────────────────────────────────────────────────

/// A text selection defined by two [`Point`]s.
///
/// `start` must appear at or before `end` in the visible string.  Use
/// [`Range::from_points`] to normalise an unordered pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    pub start: Point,
    pub end:   Point,
}

impl Range {
    /// Create a range from two already-ordered points.
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    /// Create a range from two points in any order, normalising so that
    /// `start ≤ end` by visual position.
    pub fn from_points(str_node: &StrNode, p1: Point, p2: Point) -> Self {
        match p1.cmp_spatial(&p2, str_node) {
            std::cmp::Ordering::Greater => Self::new(p2, p1),
            _ => Self::new(p1, p2),
        }
    }

    /// `true` when start and end refer to the same point (a caret, not a
    /// selection).
    pub fn is_collapsed(&self) -> bool {
        self.start == self.end
    }

    /// Number of visible characters covered by this range.
    pub fn length(&self, str_node: &StrNode) -> usize {
        let s = self.start.view_pos(str_node);
        let e = self.end.view_pos(str_node);
        e.saturating_sub(s)
    }

    /// The visible text inside this range as a `String`.
    ///
    /// Characters at boundary positions are included/excluded according to
    /// the anchor sides of `start` and `end`.
    pub fn text(&self, str_node: &StrNode) -> String {
        let s = self.start.view_pos(str_node);
        let e = self.end.view_pos(str_node);
        if e <= s {
            return String::new();
        }
        str_node.view_str().chars().skip(s).take(e - s).collect()
    }

    /// `true` when `other` is fully contained within `self` (by view
    /// position).
    pub fn contains(&self, other: &Range, str_node: &StrNode) -> bool {
        self.start.view_pos(str_node) <= other.start.view_pos(str_node)
            && other.end.view_pos(str_node) <= self.end.view_pos(str_node)
    }
}
