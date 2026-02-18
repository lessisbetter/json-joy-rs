//! `json-crdt-peritext-ui` — model/controller types for Peritext UI.
//!
//! Mirrors `packages/json-joy/src/json-crdt-peritext-ui/`.
//!
//! React/RxJS components are skipped (not portable to Rust).
//! Only the portable model types from `types.ts` are ported.

// ── Undo / Redo framework ─────────────────────────────────────────────────

/// Manages a stack of undo/redo operations.
///
/// Mirrors the `UndoManager` interface from `types.ts`.
pub trait UndoManager {
    /// The undo-state type.
    type UndoState;
    /// The redo-state type.
    type RedoState;

    /// Push a new undo item onto the stack.
    fn push(&mut self, state: Self::UndoState, callback: Box<dyn FnOnce(Self::UndoState) -> (Self::RedoState, Box<dyn FnOnce(Self::RedoState)>)>);
    /// Undo the most recent operation.
    fn undo(&mut self);
    /// Redo the most recently undone operation.
    fn redo(&mut self);
}
