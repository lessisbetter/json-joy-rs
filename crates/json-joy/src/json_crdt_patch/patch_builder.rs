//! [`PatchBuilder`] — fluent builder for constructing [`Patch`]es.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/PatchBuilder.ts`.

use crate::json_crdt_patch::clock::{ts, ClockVector, LogicalClock, ServerClockVector, Ts, Tss};
use crate::json_crdt_patch::constants::ORIGIN;
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;
use json_joy_json_pack::PackValue;

// ── Clock variants ─────────────────────────────────────────────────────────

/// The internal clock of a [`PatchBuilder`].
///
/// Either a [`LogicalClock`], a [`ClockVector`], or a [`ServerClockVector`].
#[derive(Debug, Clone)]
pub enum BuilderClock {
    Logical(LogicalClock),
    Vector(ClockVector),
    Server(ServerClockVector),
}

impl BuilderClock {
    pub fn sid(&self) -> u64 {
        match self {
            BuilderClock::Logical(c) => c.sid,
            BuilderClock::Vector(c) => c.sid,
            BuilderClock::Server(c) => c.sid,
        }
    }

    pub fn time(&self) -> u64 {
        match self {
            BuilderClock::Logical(c) => c.time,
            BuilderClock::Vector(c) => c.time,
            BuilderClock::Server(c) => c.time,
        }
    }

    /// Advance the clock by `cycles` and return the timestamp *before* advancement.
    pub fn tick(&mut self, cycles: u64) -> Ts {
        match self {
            BuilderClock::Logical(c) => c.tick(cycles),
            BuilderClock::Vector(c) => c.tick(cycles),
            BuilderClock::Server(c) => c.tick(cycles),
        }
    }
}

// ── PatchBuilder ───────────────────────────────────────────────────────────

/// Utility for constructing a [`Patch`] operation by operation.
pub struct PatchBuilder {
    pub clock: BuilderClock,
    pub patch: Patch,
}

impl PatchBuilder {
    /// Creates a new builder with a [`LogicalClock`].
    pub fn new(sid: u64, time: u64) -> Self {
        Self {
            clock: BuilderClock::Logical(LogicalClock::new(sid, time)),
            patch: Patch::new(),
        }
    }

    /// Creates a builder from a [`ClockVector`].
    pub fn from_clock_vector(cv: ClockVector) -> Self {
        Self {
            clock: BuilderClock::Vector(cv),
            patch: Patch::new(),
        }
    }

    /// Creates a builder from a [`ServerClockVector`].
    pub fn from_server_clock(cv: ServerClockVector) -> Self {
        Self {
            clock: BuilderClock::Server(cv),
            patch: Patch::new(),
        }
    }

    /// Returns the sequence number of the next operation's timestamp.
    pub fn next_time(&self) -> u64 {
        let patch_next = self.patch.next_time();
        if patch_next == 0 {
            self.clock.time()
        } else {
            patch_next
        }
    }

    /// Inserts a `Nop` to fill any gap between the clock time and the patch's
    /// last operation, then returns the current patch and resets the builder.
    pub fn flush(&mut self) -> Patch {
        let patch = std::mem::replace(&mut self.patch, Patch::new());
        patch
    }

    // ── Padding ────────────────────────────────────────────────────────────

    /// Adds a `Nop` if the clock has drifted ahead of the patch's last op.
    pub fn pad(&mut self) {
        let next_time = self.patch.next_time();
        if next_time == 0 {
            return;
        }
        let drift = self.clock.time().saturating_sub(next_time);
        if drift > 0 {
            let id = ts(self.clock.sid(), next_time);
            self.patch.ops.push(Op::Nop { id, len: drift });
        }
    }

    // ── Creation operations ────────────────────────────────────────────────

    /// Create a new `con` constant with a [`PackValue`].
    pub fn con_val(&mut self, val: PackValue) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewCon {
            id,
            val: ConValue::Val(val),
        });
        id
    }

    /// Create a new `con` constant referencing another operation ID.
    pub fn con_ref(&mut self, ref_id: Ts) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewCon {
            id,
            val: ConValue::Ref(ref_id),
        });
        id
    }

    /// Create a new `val` LWW-Register.
    pub fn val(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewVal { id });
        id
    }

    /// Create a new `obj` LWW-Map.
    pub fn obj(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewObj { id });
        id
    }

    /// Create a new `vec` LWW-Vector.
    pub fn vec(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewVec { id });
        id
    }

    /// Create a new `str` RGA-String.
    pub fn str_node(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewStr { id });
        id
    }

    /// Create a new `bin` RGA-Binary.
    pub fn bin(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewBin { id });
        id
    }

    /// Create a new `arr` RGA-Array.
    pub fn arr(&mut self) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::NewArr { id });
        id
    }

    // ── Mutation operations ────────────────────────────────────────────────

    /// Set the value of the document root `val` register.
    pub fn root(&mut self, val: Ts) -> Ts {
        self.set_val(ORIGIN, val)
    }

    /// Set key→value pairs in an `obj`.
    pub fn ins_obj(&mut self, obj: Ts, data: Vec<(String, Ts)>) -> Ts {
        assert!(!data.is_empty(), "EMPTY_TUPLES");
        self.pad();
        let id = self.clock.tick(1);
        let op = Op::InsObj { id, obj, data };
        let span = op.span();
        if span > 1 {
            self.clock.tick(span - 1);
        }
        self.patch.ops.push(op);
        id
    }

    /// Set index→value pairs in a `vec`.
    pub fn ins_vec(&mut self, obj: Ts, data: Vec<(u8, Ts)>) -> Ts {
        assert!(!data.is_empty(), "EMPTY_TUPLES");
        self.pad();
        let id = self.clock.tick(1);
        let op = Op::InsVec { id, obj, data };
        let span = op.span();
        if span > 1 {
            self.clock.tick(span - 1);
        }
        self.patch.ops.push(op);
        id
    }

    /// Set the value of a `val` register.
    pub fn set_val(&mut self, obj: Ts, val: Ts) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::InsVal { id, obj, val });
        id
    }

    /// Insert a string into a `str` object.
    pub fn ins_str(&mut self, obj: Ts, after: Ts, data: String) -> Ts {
        assert!(!data.is_empty(), "EMPTY_STRING");
        self.pad();
        let id = self.clock.tick(1);
        let op = Op::InsStr {
            id,
            obj,
            after,
            data,
        };
        let span = op.span();
        if span > 1 {
            self.clock.tick(span - 1);
        }
        self.patch.ops.push(op);
        id
    }

    /// Insert binary data into a `bin` object.
    pub fn ins_bin(&mut self, obj: Ts, after: Ts, data: Vec<u8>) -> Ts {
        assert!(!data.is_empty(), "EMPTY_BINARY");
        self.pad();
        let id = self.clock.tick(1);
        let op = Op::InsBin {
            id,
            obj,
            after,
            data,
        };
        let span = op.span();
        if span > 1 {
            self.clock.tick(span - 1);
        }
        self.patch.ops.push(op);
        id
    }

    /// Insert elements into an `arr` object.
    pub fn ins_arr(&mut self, arr: Ts, after: Ts, data: Vec<Ts>) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        let op = Op::InsArr {
            id,
            obj: arr,
            after,
            data,
        };
        let span = op.span();
        if span > 1 {
            self.clock.tick(span - 1);
        }
        self.patch.ops.push(op);
        id
    }

    /// Update an element in an `arr`.
    pub fn upd_arr(&mut self, arr: Ts, after: Ts, val: Ts) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::UpdArr {
            id,
            obj: arr,
            after,
            val,
        });
        id
    }

    /// Delete spans of operations in an object.
    pub fn del(&mut self, obj: Ts, what: Vec<Tss>) -> Ts {
        self.pad();
        let id = self.clock.tick(1);
        self.patch.ops.push(Op::Del { id, obj, what });
        id
    }

    /// Insert a no-op of the given span.
    pub fn nop(&mut self, span: u64) -> Ts {
        self.pad();
        let id = self.clock.tick(span);
        self.patch.ops.push(Op::Nop { id, len: span });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;

    #[test]
    fn builder_creates_new_obj() {
        let mut b = PatchBuilder::new(1, 0);
        let id = b.obj();
        assert_eq!(id, ts(1, 0));
        assert_eq!(b.clock.time(), 1);
    }

    #[test]
    fn builder_pads_on_drift() {
        let mut b = PatchBuilder::new(1, 0);
        b.obj(); // time = 1
                 // advance clock externally (simulating external tick)
        b.clock.tick(2); // time = 3
        b.obj(); // should insert nop(2) then new_obj at time=3
        assert_eq!(b.patch.ops.len(), 3); // NewObj, Nop, NewObj
    }

    #[test]
    fn flush_resets_patch() {
        let mut b = PatchBuilder::new(1, 0);
        b.obj();
        let p = b.flush();
        assert_eq!(p.ops.len(), 1);
        assert_eq!(b.patch.ops.len(), 0);
    }

    #[test]
    fn ins_str_advances_clock_by_char_count() {
        let mut b = PatchBuilder::new(1, 0);
        let str_id = b.str_node();
        b.ins_str(str_id, str_id, "hello".into());
        // str_node: tick 1 → time=1
        // ins_str: tick 1 → op id at time=1, then tick 4 more → time=6
        assert_eq!(b.clock.time(), 6);
    }
}
