//! RGA anchor constants for Peritext.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/rga/constants.ts`.

/// Which side of a character a [`Point`] is attached to.
///
/// When text is inserted adjacent to a range boundary, the anchor determines
/// whether the new character falls inside or outside the range:
///
/// - `Before`: the point sits to the *left* of the character.  Text inserted
///   immediately before this character pushes the boundary outward (the new
///   character is outside the range).
/// - `After`:  the point sits to the *right* of the character.  Text inserted
///   immediately after this character stays inside the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Anchor {
    Before = 0,
    After = 1,
}
