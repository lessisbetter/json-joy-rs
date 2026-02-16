use crate::patch::Timestamp;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Id {
    pub(crate) sid: u64,
    pub(crate) time: u64,
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sid.hash(state);
        self.time.hash(state);
    }
}

pub(crate) fn cmp_id_time_sid(a: Id, b: Id) -> std::cmp::Ordering {
    match a.time.cmp(&b.time) {
        std::cmp::Ordering::Equal => a.sid.cmp(&b.sid),
        ord => ord,
    }
}

impl From<Timestamp> for Id {
    fn from(v: Timestamp) -> Self {
        Self {
            sid: v.sid,
            time: v.time,
        }
    }
}

impl From<Id> for Timestamp {
    fn from(v: Id) -> Self {
        Self {
            sid: v.sid,
            time: v.time,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeNode {
    Con(ConCell),
    Val(Id),
    Obj(Vec<(String, Id)>),
    Vec(BTreeMap<u64, Id>),
    Str(Vec<StrAtom>),
    Bin(Vec<BinAtom>),
    Arr(Vec<ArrAtom>),
}

#[derive(Debug, Clone)]
pub(crate) enum ConCell {
    Json(Value),
    Ref(Id),
    Undef,
}

#[derive(Debug, Clone)]
pub(crate) struct StrAtom {
    pub(crate) slot: Id,
    pub(crate) ch: Option<char>,
}

#[derive(Debug, Clone)]
pub(crate) struct BinAtom {
    pub(crate) slot: Id,
    pub(crate) byte: Option<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ArrAtom {
    pub(crate) slot: Id,
    pub(crate) value: Option<Id>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ClockState {
    pub(crate) observed: HashMap<u64, Vec<(u64, u64)>>,
}

impl ClockState {
    pub(crate) fn observe(&mut self, sid: u64, start: u64, span: u64) {
        let end = start + span.saturating_sub(1);
        let ranges = self.observed.entry(sid).or_default();
        ranges.push((start, end));
        ranges.sort_by_key(|(a, _)| *a);
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
        for (a, b) in ranges.iter().copied() {
            if let Some(last) = merged.last_mut() {
                if a <= last.1.saturating_add(1) {
                    last.1 = last.1.max(b);
                } else {
                    merged.push((a, b));
                }
            } else {
                merged.push((a, b));
            }
        }
        *ranges = merged;
    }
}
