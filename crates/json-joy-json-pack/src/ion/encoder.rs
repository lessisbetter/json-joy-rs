//! Ion binary encoder (fast path).
//!
//! Upstream reference: `json-pack/src/ion/IonEncoderFast.ts`

use json_joy_buffers::Writer;

use super::constants::{TypeOverlay, ION_BVM, SID_ION_SYMBOL_TABLE, SID_SYMBOLS};
use super::symbols::IonSymbols;
use crate::PackValue;

/// Calculates the number of bytes needed for a VUint encoding.
fn vuint_len(n: u32) -> usize {
    if n <= 127 {
        1
    } else if n <= 16383 {
        2
    } else if n <= 2097151 {
        3
    } else if n <= 268435455 {
        4
    } else {
        5
    }
}

/// Ion binary encoder.
///
/// Encodes [`PackValue`] to Ion binary format with symbol tables.
pub struct IonEncoder {
    pub writer: Writer,
    symbols: IonSymbols,
}

impl Default for IonEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl IonEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
            symbols: IonSymbols::new(),
        }
    }

    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.writer.reset();
        self.symbols = IonSymbols::new();

        // First pass: collect all field keys into symbol table.
        self.collect_symbols(value);

        // Write Ion Binary Version Marker.
        self.write_ivm();

        // Write user symbol table if there are user symbols.
        if self.symbols.has_user_symbols() {
            self.write_symbol_table();
        }

        // Write the value.
        self.write_any(value);
        self.writer.flush()
    }

    fn collect_symbols(&mut self, value: &PackValue) {
        match value {
            PackValue::Object(obj) => {
                for (key, val) in obj {
                    self.symbols.add(key);
                    self.collect_symbols(val);
                }
            }
            PackValue::Array(arr) => {
                for item in arr {
                    self.collect_symbols(item);
                }
            }
            _ => {}
        }
    }

    fn write_ivm(&mut self) {
        self.writer.buf(&ION_BVM);
    }

    fn write_symbol_table(&mut self) {
        let user_symbols = self.symbols.user_symbols().to_vec();

        // Build symbols list length and struct content.
        // symbols = array of strings, each string: varint(len) + bytes
        let mut sym_bytes: Vec<u8> = Vec::new();
        for sym in &user_symbols {
            let s_bytes = sym.as_bytes();
            let s_len = s_bytes.len();
            // Write string descriptor + content
            let descriptor = if s_len < 14 {
                vec![TypeOverlay::STRI | s_len as u8]
            } else {
                let mut v = vec![TypeOverlay::STRI | 14];
                write_vuint_to(&mut v, s_len as u32);
                v
            };
            sym_bytes.extend_from_slice(&descriptor);
            sym_bytes.extend_from_slice(s_bytes);
        }

        // Build list: type_overlay for LIST + len + sym_bytes
        let sym_list_content_len = sym_bytes.len();
        let mut list_bytes: Vec<u8> = Vec::new();
        if sym_list_content_len < 14 {
            list_bytes.push(TypeOverlay::LIST | sym_list_content_len as u8);
        } else {
            list_bytes.push(TypeOverlay::LIST | 14);
            write_vuint_to(&mut list_bytes, sym_list_content_len as u32);
        }
        list_bytes.extend_from_slice(&sym_bytes);

        // Build struct: { 7 (symbols) → list }
        let field_sid = SID_SYMBOLS;
        let field_sid_len = vuint_len(field_sid);
        let struct_content_len = field_sid_len + list_bytes.len();

        let mut struct_bytes: Vec<u8> = Vec::new();
        if struct_content_len < 14 {
            struct_bytes.push(TypeOverlay::STRU | struct_content_len as u8);
        } else {
            struct_bytes.push(TypeOverlay::STRU | 14);
            write_vuint_to(&mut struct_bytes, struct_content_len as u32);
        }
        write_vuint_to(&mut struct_bytes, field_sid);
        struct_bytes.extend_from_slice(&list_bytes);

        // Wrap in annotation: annotationLen + [annotation_sid] + struct
        let anno_sid = SID_ION_SYMBOL_TABLE;
        let anno_sid_len = vuint_len(anno_sid);
        let annotation_len = anno_sid_len; // length of the annotation sequence
        let anno_seq_len = vuint_len(annotation_len as u32);
        let total_anno_content = anno_seq_len + annotation_len + struct_bytes.len();

        // Write annotation type descriptor
        if total_anno_content < 14 {
            self.writer.u8(TypeOverlay::ANNO | total_anno_content as u8);
        } else {
            self.writer.u8(TypeOverlay::ANNO | 14);
            self.write_vuint(total_anno_content as u32);
        }
        self.write_vuint(annotation_len as u32);
        self.write_vuint(anno_sid);
        self.writer.buf(&struct_bytes);
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null | PackValue::Undefined => self.write_null(),
            PackValue::Bool(b) => self.write_bool(*b),
            PackValue::Integer(n) => {
                if *n >= 0 {
                    self.write_uint(*n as u64);
                } else {
                    self.write_nint(-(*n) as u64);
                }
            }
            PackValue::UInteger(n) => self.write_uint(*n),
            PackValue::Float(f) => self.write_float(*f),
            PackValue::BigInt(n) => {
                if *n >= 0 {
                    self.write_uint(*n as u64);
                } else {
                    self.write_nint(-(*n) as u64);
                }
            }
            PackValue::Str(s) => self.write_str(s),
            PackValue::Bytes(b) => self.write_bin(b),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj(obj),
            PackValue::Extension(_) | PackValue::Blob(_) => self.write_null(),
        }
    }

    pub fn write_null(&mut self) {
        self.writer.u8(TypeOverlay::NULL | 15);
    }

    pub fn write_bool(&mut self, b: bool) {
        self.writer.u8(TypeOverlay::BOOL | if b { 1 } else { 0 });
    }

    pub fn write_uint(&mut self, n: u64) {
        if n == 0 {
            self.writer.u8(TypeOverlay::UINT);
            return;
        }
        // Calculate byte length needed.
        let len = uint_byte_len(n);
        self.writer.u8(TypeOverlay::UINT | len as u8);
        // Write big-endian bytes (only significant bytes).
        for i in (0..len).rev() {
            self.writer.u8(((n >> (i * 8)) & 0xff) as u8);
        }
    }

    pub fn write_nint(&mut self, n: u64) {
        // n is the magnitude (positive). Encode as negative integer.
        let len = uint_byte_len(n);
        self.writer.u8(TypeOverlay::NINT | len as u8);
        for i in (0..len).rev() {
            self.writer.u8(((n >> (i * 8)) & 0xff) as u8);
        }
    }

    pub fn write_float(&mut self, f: f64) {
        self.writer.u8(TypeOverlay::FLOT | 8);
        // NOTE: The Ion binary spec (§5) requires big-endian IEEE 754, but the upstream
        // TypeScript implementation uses little-endian (matching the @jsonjoy.com/buffers
        // reader). We match the upstream behavior intentionally for wire compatibility.
        let bits = f.to_bits().to_le_bytes();
        self.writer.buf(&bits);
    }

    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if len < 14 {
            self.writer.u8(TypeOverlay::STRI | len as u8);
        } else {
            self.writer.u8(TypeOverlay::STRI | 14);
            self.write_vuint(len as u32);
        }
        self.writer.buf(bytes);
    }

    pub fn write_bin(&mut self, data: &[u8]) {
        let len = data.len();
        if len < 14 {
            self.writer.u8(TypeOverlay::BINA | len as u8);
        } else {
            self.writer.u8(TypeOverlay::BINA | 14);
            self.write_vuint(len as u32);
        }
        self.writer.buf(data);
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        // Encode each element using self (shares the symbol table), measuring bytes.
        let mut content: Vec<u8> = Vec::new();
        for item in arr {
            content.extend_from_slice(&self.encode_value_to_bytes(item));
        }
        let len = content.len();
        if len < 14 {
            self.writer.u8(TypeOverlay::LIST | len as u8);
        } else {
            self.writer.u8(TypeOverlay::LIST | 14);
            self.write_vuint(len as u32);
        }
        self.writer.buf(&content);
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        // Encode each field using self (shares the symbol table), measuring bytes.
        let mut content: Vec<u8> = Vec::new();
        for (key, val) in obj {
            let sid = self.symbols.add(key);
            write_vuint_to(&mut content, sid);
            content.extend_from_slice(&self.encode_value_to_bytes(val));
        }
        let len = content.len();
        if len < 14 {
            self.writer.u8(TypeOverlay::STRU | len as u8);
        } else {
            self.writer.u8(TypeOverlay::STRU | 14);
            self.write_vuint(len as u32);
        }
        self.writer.buf(&content);
    }

    /// Encodes a value to bytes using this encoder's symbol table.
    ///
    /// Temporarily swaps writers so `write_any` targets a fresh buffer; the
    /// parent writer is restored afterwards, preserving any partially written state.
    fn encode_value_to_bytes(&mut self, value: &PackValue) -> Vec<u8> {
        let mut tmp_writer = Writer::new();
        std::mem::swap(&mut self.writer, &mut tmp_writer);
        self.write_any(value);
        std::mem::swap(&mut self.writer, &mut tmp_writer);
        tmp_writer.flush()
    }

    fn write_vuint(&mut self, n: u32) {
        let mut bytes: Vec<u8> = Vec::new();
        write_vuint_to(&mut bytes, n);
        self.writer.buf(&bytes);
    }
}

fn uint_byte_len(n: u64) -> usize {
    if n <= 0xff {
        1
    } else if n <= 0xffff {
        2
    } else if n <= 0xffffff {
        3
    } else if n <= 0xffffffff {
        4
    } else if n <= 0xffffffffff {
        5
    } else if n <= 0xffffffffffff {
        6
    } else {
        7
    }
}

/// Write a VUint to a byte vector.
/// Ion VUint: each byte has 7 data bits; MSB=1 signals continuation.
/// The last byte has MSB=0.
fn write_vuint_to(out: &mut Vec<u8>, n: u32) {
    if n <= 127 {
        out.push(0x80 | (n as u8));
        return;
    }
    if n <= 16383 {
        out.push(((n >> 7) & 0x7f) as u8);
        out.push(0x80 | (n & 0x7f) as u8);
        return;
    }
    if n <= 2097151 {
        out.push(((n >> 14) & 0x7f) as u8);
        out.push(((n >> 7) & 0x7f) as u8);
        out.push(0x80 | (n & 0x7f) as u8);
        return;
    }
    if n <= 268435455 {
        out.push(((n >> 21) & 0x7f) as u8);
        out.push(((n >> 14) & 0x7f) as u8);
        out.push(((n >> 7) & 0x7f) as u8);
        out.push(0x80 | (n & 0x7f) as u8);
        return;
    }
    out.push(((n >> 28) & 0x7f) as u8);
    out.push(((n >> 21) & 0x7f) as u8);
    out.push(((n >> 14) & 0x7f) as u8);
    out.push(((n >> 7) & 0x7f) as u8);
    out.push(0x80 | (n & 0x7f) as u8);
}
