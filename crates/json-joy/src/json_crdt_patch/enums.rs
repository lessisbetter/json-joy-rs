//! Enumerations for the JSON CRDT Patch protocol.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/enums.ts`.

/// Reserved session IDs.
pub mod SESSION {
    /// Reserved by the protocol — cannot be used by users.
    pub const SYSTEM: u64 = 0;
    /// The only valid session ID when running in server-clock mode.
    pub const SERVER: u64 = 1;
    /// Global/schema patches applied on all clients identically.
    pub const GLOBAL: u64 = 2;
    /// Local-only patches (e.g. cursor position, not shared).
    pub const LOCAL: u64 = 3;
    /// Maximum allowed session ID (53-bit safe integer).
    pub const MAX: u64 = 9007199254740991;
}

/// Reserved system-session time values.
pub mod SYSTEM_SESSION_TIME {
    pub const ORIGIN: u64 = 0;
    pub const UNDEFINED: u64 = 1;
}

/// 3-bit CRDT data-type discriminant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonCrdtDataType {
    Con = 0b000,
    Val = 0b001,
    Obj = 0b010,
    Vec = 0b011,
    Str = 0b100,
    Bin = 0b101,
    Arr = 0b110,
}

/// 5-bit operation opcode.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonCrdtPatchOpcode {
    NewCon = 0b00000 | JsonCrdtDataType::Con as u8, // 0
    NewVal = 0b00000 | JsonCrdtDataType::Val as u8, // 1
    NewObj = 0b00000 | JsonCrdtDataType::Obj as u8, // 2
    NewVec = 0b00000 | JsonCrdtDataType::Vec as u8, // 3
    NewStr = 0b00000 | JsonCrdtDataType::Str as u8, // 4
    NewBin = 0b00000 | JsonCrdtDataType::Bin as u8, // 5
    NewArr = 0b00000 | JsonCrdtDataType::Arr as u8, // 6
    InsVal = 0b01000 | JsonCrdtDataType::Val as u8, // 9
    InsObj = 0b01000 | JsonCrdtDataType::Obj as u8, // 10
    InsVec = 0b01000 | JsonCrdtDataType::Vec as u8, // 11
    InsStr = 0b01000 | JsonCrdtDataType::Str as u8, // 12
    InsBin = 0b01000 | JsonCrdtDataType::Bin as u8, // 13
    InsArr = 0b01000 | JsonCrdtDataType::Arr as u8, // 14
    UpdArr = 0b01000 | JsonCrdtDataType::Arr as u8 + 1, // 15
    Del = 0b10000,                                  // 16
    Nop = 0b10001,                                  // 17
}

impl JsonCrdtPatchOpcode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NewCon),
            1 => Some(Self::NewVal),
            2 => Some(Self::NewObj),
            3 => Some(Self::NewVec),
            4 => Some(Self::NewStr),
            5 => Some(Self::NewBin),
            6 => Some(Self::NewArr),
            9 => Some(Self::InsVal),
            10 => Some(Self::InsObj),
            11 => Some(Self::InsVec),
            12 => Some(Self::InsStr),
            13 => Some(Self::InsBin),
            14 => Some(Self::InsArr),
            15 => Some(Self::UpdArr),
            16 => Some(Self::Del),
            17 => Some(Self::Nop),
            _ => None,
        }
    }
}

/// Opcode shifted left 3 bits (used as the high byte of the opcode octet,
/// leaving 3 low bits for inline length hints).
pub mod OpcodeOverlay {
    use super::JsonCrdtPatchOpcode as O;

    pub const NEW_CON: u8 = (O::NewCon as u8) << 3;
    pub const NEW_VAL: u8 = (O::NewVal as u8) << 3;
    pub const NEW_OBJ: u8 = (O::NewObj as u8) << 3;
    pub const NEW_VEC: u8 = (O::NewVec as u8) << 3;
    pub const NEW_STR: u8 = (O::NewStr as u8) << 3;
    pub const NEW_BIN: u8 = (O::NewBin as u8) << 3;
    pub const NEW_ARR: u8 = (O::NewArr as u8) << 3;
    pub const INS_VAL: u8 = (O::InsVal as u8) << 3;
    pub const INS_OBJ: u8 = (O::InsObj as u8) << 3;
    pub const INS_VEC: u8 = (O::InsVec as u8) << 3;
    pub const INS_STR: u8 = (O::InsStr as u8) << 3;
    pub const INS_BIN: u8 = (O::InsBin as u8) << 3;
    pub const INS_ARR: u8 = (O::InsArr as u8) << 3;
    pub const UPD_ARR: u8 = (O::UpdArr as u8) << 3;
    pub const DEL: u8 = (O::Del as u8) << 3;
    pub const NOP: u8 = (O::Nop as u8) << 3;
}
