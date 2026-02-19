//! All 16 JSON CRDT Patch operations as a single Rust enum.
//!
//! Mirrors the 16 operation classes in
//! `packages/json-joy/src/json-crdt-patch/operations.ts`.

use crate::json_crdt_patch::clock::{print_ts, Ts, Tss};
use json_joy_json_pack::PackValue;

// ── ConValue ───────────────────────────────────────────────────────────────

/// The value stored in a `new_con` operation.
///
/// Upstream type is `unknown | undefined | ITimestampStruct`:
/// - A `Timestamp` → reference to another CRDT node.
/// - Anything else → a constant JSON-compatible value encoded as CBOR.
#[derive(Debug, Clone, PartialEq)]
pub enum ConValue {
    /// A timestamp reference to another CRDT node.
    Ref(Ts),
    /// A constant value (null, bool, number, string, binary, array, object, undefined).
    Val(PackValue),
}

// ── Operation ──────────────────────────────────────────────────────────────

/// A single JSON CRDT Patch operation.
///
/// Each variant carries an `id: Ts` identifying the operation in the
/// global logical clock space.
///
/// Span (the number of clock ticks consumed):
/// - Most operations consume 1 tick.
/// - `InsStr`, `InsBin`, `InsArr` consume `data.len()` ticks.
/// - `Nop` consumes `len` ticks.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    // ── Creation operations ──────────────────────────────────────────────
    /// Create a new constant `con` value.
    NewCon { id: Ts, val: ConValue },
    /// Create a new LWW-Register `val` object.
    NewVal { id: Ts },
    /// Create a new LWW-Map `obj` object.
    NewObj { id: Ts },
    /// Create a new LWW-Vector `vec` object.
    NewVec { id: Ts },
    /// Create a new RGA-String `str` object.
    NewStr { id: Ts },
    /// Create a new RGA-Binary `bin` object.
    NewBin { id: Ts },
    /// Create a new RGA-Array `arr` object.
    NewArr { id: Ts },

    // ── Mutation operations ──────────────────────────────────────────────
    /// Set the value of a `val` register.
    InsVal { id: Ts, obj: Ts, val: Ts },
    /// Set key→value pairs in an `obj` map.
    InsObj {
        id: Ts,
        obj: Ts,
        data: Vec<(String, Ts)>,
    },
    /// Set index→value pairs in a `vec` vector.
    InsVec {
        id: Ts,
        obj: Ts,
        data: Vec<(u8, Ts)>,
    },
    /// Insert a string into a `str` RGA.
    InsStr {
        id: Ts,
        obj: Ts,
        after: Ts,
        data: String,
    },
    /// Insert binary data into a `bin` RGA.
    InsBin {
        id: Ts,
        obj: Ts,
        after: Ts,
        data: Vec<u8>,
    },
    /// Insert elements into an `arr` RGA.
    InsArr {
        id: Ts,
        obj: Ts,
        after: Ts,
        data: Vec<Ts>,
    },
    /// Update an existing element in an `arr` array.
    UpdArr { id: Ts, obj: Ts, after: Ts, val: Ts },
    /// Delete ranges of operations in an object (str/bin/arr).
    Del { id: Ts, obj: Ts, what: Vec<Tss> },
    /// No-op — skips clock cycles without performing any CRDT action.
    Nop { id: Ts, len: u64 },
}

impl Op {
    /// Returns the ID (first timestamp) of this operation.
    pub fn id(&self) -> Ts {
        match self {
            Op::NewCon { id, .. }
            | Op::NewVal { id }
            | Op::NewObj { id }
            | Op::NewVec { id }
            | Op::NewStr { id }
            | Op::NewBin { id }
            | Op::NewArr { id }
            | Op::InsVal { id, .. }
            | Op::InsObj { id, .. }
            | Op::InsVec { id, .. }
            | Op::InsStr { id, .. }
            | Op::InsBin { id, .. }
            | Op::InsArr { id, .. }
            | Op::UpdArr { id, .. }
            | Op::Del { id, .. }
            | Op::Nop { id, .. } => *id,
        }
    }

    /// Number of logical clock cycles consumed by this operation.
    pub fn span(&self) -> u64 {
        match self {
            Op::InsStr { data, .. } => data.chars().count() as u64,
            Op::InsBin { data, .. } => data.len() as u64,
            Op::InsArr { data, .. } => data.len() as u64,
            Op::Nop { len, .. } => *len,
            _ => 1,
        }
    }

    /// Short mnemonic name of this operation (used in verbose JSON codec).
    pub fn name(&self) -> &'static str {
        match self {
            Op::NewCon { .. } => "new_con",
            Op::NewVal { .. } => "new_val",
            Op::NewObj { .. } => "new_obj",
            Op::NewVec { .. } => "new_vec",
            Op::NewStr { .. } => "new_str",
            Op::NewBin { .. } => "new_bin",
            Op::NewArr { .. } => "new_arr",
            Op::InsVal { .. } => "ins_val",
            Op::InsObj { .. } => "ins_obj",
            Op::InsVec { .. } => "ins_vec",
            Op::InsStr { .. } => "ins_str",
            Op::InsBin { .. } => "ins_bin",
            Op::InsArr { .. } => "ins_arr",
            Op::UpdArr { .. } => "upd_arr",
            Op::Del { .. } => "del",
            Op::Nop { .. } => "nop",
        }
    }
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = self.id();
        let span = self.span();
        let base = if span > 1 {
            format!("{} {}!{}", self.name(), print_ts(id), span)
        } else {
            format!("{} {}", self.name(), print_ts(id))
        };
        match self {
            Op::InsVal { obj, val, .. } => write!(
                f,
                "{}, obj = {}, val = {}",
                base,
                print_ts(*obj),
                print_ts(*val)
            ),
            Op::InsStr {
                obj, after, data, ..
            } => write!(
                f,
                "{}, obj = {} {{ {} ← {:?} }}",
                base,
                print_ts(*obj),
                print_ts(*after),
                data
            ),
            Op::InsBin {
                obj, after, data, ..
            } => write!(
                f,
                "{}, obj = {} {{ {} ← {:?} }}",
                base,
                print_ts(*obj),
                print_ts(*after),
                data
            ),
            Op::Del { obj, what, .. } => {
                let spans: Vec<_> = what
                    .iter()
                    .map(|s| format!("{}!{}", print_ts(s.ts()), s.span))
                    .collect();
                write!(
                    f,
                    "{}, obj = {} {{ {} }}",
                    base,
                    print_ts(*obj),
                    spans.join(", ")
                )
            }
            _ => write!(f, "{}", base),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;

    #[test]
    fn span_of_nop() {
        let op = Op::Nop {
            id: ts(1, 0),
            len: 5,
        };
        assert_eq!(op.span(), 5);
    }

    #[test]
    fn span_of_ins_str() {
        let op = Op::InsStr {
            id: ts(1, 0),
            obj: ts(1, 0),
            after: ts(1, 0),
            data: "hello".into(),
        };
        assert_eq!(op.span(), 5);
    }

    #[test]
    fn span_of_creation_op() {
        let op = Op::NewObj { id: ts(1, 0) };
        assert_eq!(op.span(), 1);
    }

    #[test]
    fn op_name() {
        assert_eq!(
            Op::NewCon {
                id: ts(1, 0),
                val: ConValue::Val(PackValue::Null)
            }
            .name(),
            "new_con"
        );
        assert_eq!(
            Op::Del {
                id: ts(1, 0),
                obj: ts(1, 0),
                what: vec![]
            }
            .name(),
            "del"
        );
    }
}
