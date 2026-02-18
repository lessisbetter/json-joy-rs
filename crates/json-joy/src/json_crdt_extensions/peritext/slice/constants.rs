//! Slice constants for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/constants.ts`.

// ── SliceStacking ─────────────────────────────────────────────────────────

/// Controls how concurrent slices of the same type interact.
///
/// Stored in bits `[4:2]` of the packed slice header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SliceStacking {
    /// Block-split marker.  Only one marker is allowed at a given position;
    /// they do not stack.
    Marker = 0,
    /// Multiple annotations of the same type may overlap (e.g. bold spans
    /// from different authors).
    Many   = 1,
    /// Only one annotation of this type is active at a time (last-write-wins
    /// within a range).
    One    = 2,
    /// Soft-deletes (erases) overlapping annotations of the same type.
    Erase  = 3,
    /// A user cursor — treated like `One` but semantically a cursor.
    Cursor = 4,
}

impl TryFrom<u8> for SliceStacking {
    type Error = ();
    fn try_from(n: u8) -> Result<Self, ()> {
        match n {
            0 => Ok(Self::Marker),
            1 => Ok(Self::Many),
            2 => Ok(Self::One),
            3 => Ok(Self::Erase),
            4 => Ok(Self::Cursor),
            _ => Err(()),
        }
    }
}

// ── Header bit layout ─────────────────────────────────────────────────────

/// Bit 0 of the header: start-point anchor (0 = Before, 1 = After).
pub const HEADER_X1_ANCHOR_BIT: u64 = 1 << 0;
/// Bit 1 of the header: end-point anchor (0 = Before, 1 = After).
pub const HEADER_X2_ANCHOR_BIT: u64 = 1 << 1;
/// Bits 4–2 of the header: [`SliceStacking`] value.
pub const HEADER_STACKING_SHIFT: u32 = 2;
pub const HEADER_STACKING_MASK: u64  = 0b111 << HEADER_STACKING_SHIFT;

// ── VecNode tuple indices ─────────────────────────────────────────────────

/// Index of each field within the slice's backing `VecNode`.
pub mod tuple_index {
    pub const HEADER: usize = 0;
    pub const X1:     usize = 1;
    pub const X2:     usize = 2;
    pub const TYPE_:  usize = 3;
    pub const DATA:   usize = 4;
}

// ── Common slice type constants ───────────────────────────────────────────
//
// Inline types are negative integers; block types are non-negative.
// Values match the upstream `SliceTypeCon` enum.

/// Cursor (inline, -1).
pub const TYPE_CURSOR:       i64 = -1;
/// Remote cursor (inline, -2).
pub const TYPE_REMOTE_CURSOR: i64 = -2;
/// Bold (inline, -3).
pub const TYPE_BOLD:         i64 = -3;
/// Bold alias (inline, -4).
pub const TYPE_BOLD2:        i64 = -4;
/// Strong (inline, -5).
pub const TYPE_STRONG:       i64 = -5;
/// Italic (inline, -6).
pub const TYPE_ITALIC:       i64 = -6;
/// Italic alias (inline, -7).
pub const TYPE_ITALIC2:      i64 = -7;
/// Em (inline, -8).
pub const TYPE_EM:           i64 = -8;
/// Underline (inline, -9).
pub const TYPE_UNDERLINE:    i64 = -9;
/// Strikethrough (inline, -12).
pub const TYPE_STRIKETHROUGH: i64 = -12;
/// Inline code (inline, -15).
pub const TYPE_CODE:         i64 = -15;
/// Link (inline, -17).
pub const TYPE_LINK:         i64 = -17;

/// Paragraph (block, 0).
pub const TYPE_P:            i64 = 0;
/// Blockquote (block, 1).
pub const TYPE_BLOCKQUOTE:   i64 = 1;
/// Code block (block, 2).
pub const TYPE_CODEBLOCK:    i64 = 2;
/// Unordered list (block, 6).
pub const TYPE_UL:           i64 = 6;
/// Ordered list (block, 7).
pub const TYPE_OL:           i64 = 7;
/// Task list (block, 8).
pub const TYPE_TL:           i64 = 8;
/// List item (block, 9).
pub const TYPE_LI:           i64 = 9;
/// Heading 1 (block, 11).
pub const TYPE_H1:           i64 = 11;
/// Heading 2 (block, 12).
pub const TYPE_H2:           i64 = 12;
/// Heading 3 (block, 13).
pub const TYPE_H3:           i64 = 13;
/// Heading 4 (block, 14).
pub const TYPE_H4:           i64 = 14;
/// Heading 5 (block, 15).
pub const TYPE_H5:           i64 = 15;
/// Heading 6 (block, 16).
pub const TYPE_H6:           i64 = 16;
