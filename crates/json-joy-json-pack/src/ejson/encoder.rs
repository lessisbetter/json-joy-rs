//! EJSON v2 (MongoDB Extended JSON) encoder.
//!
//! Upstream reference: `json-pack/src/ejson/EjsonEncoder.ts`
//!
//! Produces UTF-8 JSON bytes where BSON types are encoded as `$`-prefixed
//! wrapper objects (e.g. `{"$oid":"..."}`) in either canonical or relaxed mode.

use json_joy_buffers::Writer;

use crate::bson::{
    BsonBinary, BsonDbPointer, BsonDecimal128, BsonFloat, BsonInt32, BsonInt64, BsonJavascriptCode,
    BsonObjectId, BsonSymbol, BsonTimestamp, BsonValue,
};

use super::error::EjsonEncodeError;
use super::value::EjsonValue;

/// Options controlling EJSON encoding behaviour.
#[derive(Debug, Clone, Default)]
pub struct EjsonEncoderOptions {
    /// When `true`, all numbers and dates are represented with explicit type
    /// wrappers (canonical mode).  When `false` (default), native JSON types
    /// are used where they are lossless (relaxed mode).
    pub canonical: bool,
}

/// EJSON encoder — writes Extended JSON to a `Writer` buffer.
pub struct EjsonEncoder {
    pub writer: Writer,
    pub options: EjsonEncoderOptions,
}

impl Default for EjsonEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EjsonEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
            options: EjsonEncoderOptions::default(),
        }
    }

    pub fn canonical() -> Self {
        Self {
            writer: Writer::new(),
            options: EjsonEncoderOptions { canonical: true },
        }
    }

    pub fn with_options(options: EjsonEncoderOptions) -> Self {
        Self {
            writer: Writer::new(),
            options,
        }
    }

    // ----------------------------------------------------------------
    // Public encode entry-points

    /// Encode an `EjsonValue` to UTF-8 JSON bytes.
    pub fn encode(&mut self, value: &EjsonValue) -> Result<Vec<u8>, EjsonEncodeError> {
        self.writer.reset();
        self.write_any(value)?;
        Ok(self.writer.flush())
    }

    /// Convenience: encode and return as a `String`.
    pub fn encode_to_string(&mut self, value: &EjsonValue) -> Result<String, EjsonEncodeError> {
        let bytes = self.encode(value)?;
        Ok(String::from_utf8(bytes).unwrap_or_default())
    }

    // ----------------------------------------------------------------
    // Core write dispatch (mirrors EjsonEncoder.writeAny)

    pub fn write_any(&mut self, value: &EjsonValue) -> Result<(), EjsonEncodeError> {
        match value {
            EjsonValue::Null => {
                self.write_null();
                Ok(())
            }
            EjsonValue::Undefined => {
                self.write_undefined_wrapper();
                Ok(())
            }
            EjsonValue::Bool(b) => {
                self.write_boolean(*b);
                Ok(())
            }
            EjsonValue::Number(n) => {
                self.write_number_as_ejson(*n);
                Ok(())
            }
            EjsonValue::Str(s) => {
                self.write_str(s);
                Ok(())
            }
            EjsonValue::Array(arr) => self.write_arr(arr),
            EjsonValue::Object(obj) => self.write_obj(obj),
            EjsonValue::Date { timestamp_ms, iso } => {
                self.write_date_as_ejson(*timestamp_ms, iso.as_deref())
            }
            EjsonValue::RegExp(source, flags) => {
                self.write_regexp_as_ejson(source, flags);
                Ok(())
            }
            EjsonValue::ObjectId(id) => {
                self.write_object_id_as_ejson(id);
                Ok(())
            }
            EjsonValue::Int32(v) => {
                self.write_bson_int32_as_ejson(v);
                Ok(())
            }
            EjsonValue::Int64(v) => {
                self.write_bson_int64_as_ejson(v);
                Ok(())
            }
            EjsonValue::BsonFloat(v) => {
                self.write_bson_float_as_ejson(v);
                Ok(())
            }
            EjsonValue::Decimal128(v) => {
                self.write_bson_decimal128_as_ejson(v);
                Ok(())
            }
            EjsonValue::Binary(v) => {
                self.write_bson_binary_as_ejson(v);
                Ok(())
            }
            EjsonValue::Code(v) => {
                self.write_bson_code_as_ejson(v);
                Ok(())
            }
            EjsonValue::CodeWithScope(v) => self.write_bson_code_wscope_bson(&v.code, &v.scope),
            EjsonValue::Symbol(v) => {
                self.write_bson_symbol_as_ejson(v);
                Ok(())
            }
            EjsonValue::Timestamp(v) => {
                self.write_bson_timestamp_as_ejson(v);
                Ok(())
            }
            EjsonValue::DbPointer(v) => self.write_bson_db_pointer_as_ejson(v),
            EjsonValue::MinKey(_) => {
                self.write_bson_min_key_as_ejson();
                Ok(())
            }
            EjsonValue::MaxKey(_) => {
                self.write_bson_max_key_as_ejson();
                Ok(())
            }
            EjsonValue::Integer(i) => {
                self.write_integer_as_ejson(*i);
                Ok(())
            }
            EjsonValue::Float(f) => {
                self.write_float_as_ejson(*f);
                Ok(())
            }
        }
    }

    // ----------------------------------------------------------------
    // Primitives

    pub fn write_null(&mut self) {
        self.writer.buf(b"null");
    }

    pub fn write_boolean(&mut self, b: bool) {
        if b {
            self.writer.buf(b"true");
        } else {
            self.writer.buf(b"false");
        }
    }

    /// Write a plain JSON number (integer-like or float).
    pub fn write_number(&mut self, n: f64) {
        let s = format_number(n);
        self.writer.ascii(&s);
    }

    /// Write a JSON-encoded string with proper escaping.
    pub fn write_str(&mut self, s: &str) {
        let json = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
        self.writer.buf(json.as_bytes());
    }

    pub fn write_arr(&mut self, arr: &[EjsonValue]) -> Result<(), EjsonEncodeError> {
        self.writer.u8(b'[');
        for (i, item) in arr.iter().enumerate() {
            if i > 0 {
                self.writer.u8(b',');
            }
            self.write_any(item)?;
        }
        self.writer.u8(b']');
        Ok(())
    }

    pub fn write_obj(&mut self, obj: &[(String, EjsonValue)]) -> Result<(), EjsonEncodeError> {
        if obj.is_empty() {
            self.writer.buf(b"{}");
            return Ok(());
        }
        self.writer.u8(b'{');
        for (i, (key, val)) in obj.iter().enumerate() {
            if i > 0 {
                self.writer.u8(b',');
            }
            self.write_str(key);
            self.writer.u8(b':');
            self.write_any(val)?;
        }
        self.writer.u8(b'}');
        Ok(())
    }

    // ----------------------------------------------------------------
    // Number dispatch — plain integers and floats from decoded values

    fn write_integer_as_ejson(&mut self, value: i64) {
        // A plain integer from decoded JSON — emit as-is
        self.writer.ascii(&value.to_string());
    }

    fn write_float_as_ejson(&mut self, value: f64) {
        // A plain float from decoded JSON — emit as-is (not a BSON wrapper)
        self.writer.ascii(&format_number(value));
    }

    // ----------------------------------------------------------------
    // Number dispatch (canonical vs relaxed)

    fn write_number_as_ejson(&mut self, value: f64) {
        if self.options.canonical {
            if value.fract() == 0.0 && value.is_finite() {
                let as_i64 = value as i64;
                if as_i64 >= i32::MIN as i64 && as_i64 <= i32::MAX as i64 {
                    self.write_number_int_wrapper(as_i64);
                } else {
                    self.write_number_long_wrapper_f64(value);
                }
            } else {
                self.write_number_double_wrapper(value);
            }
        } else {
            // Relaxed: use native JSON for finite numbers
            if !value.is_finite() {
                self.write_number_double_wrapper(value);
            } else {
                self.write_number(value);
            }
        }
    }

    fn write_number_int_wrapper(&mut self, value: i64) {
        // {"$numberInt":"value"}
        self.writer.buf(b"{\"$numberInt\":\"");
        self.writer.ascii(&value.to_string());
        self.writer.buf(b"\"}");
    }

    fn write_number_long_wrapper_i64(&mut self, value: i64) {
        // {"$numberLong":"value"}
        self.writer.buf(b"{\"$numberLong\":\"");
        self.writer.ascii(&value.to_string());
        self.writer.buf(b"\"}");
    }

    fn write_number_long_wrapper_f64(&mut self, value: f64) {
        // {"$numberLong":"value"}  (large integer stored as f64)
        self.writer.buf(b"{\"$numberLong\":\"");
        self.writer.ascii(&format_integer_f64(value));
        self.writer.buf(b"\"}");
    }

    fn write_number_double_wrapper(&mut self, value: f64) {
        // {"$numberDouble":"value"}
        self.writer.buf(b"{\"$numberDouble\":\"");
        let s = if !value.is_finite() {
            format_non_finite(value)
        } else {
            format_number(value)
        };
        self.writer.ascii(&s);
        self.writer.buf(b"\"}");
    }

    // ----------------------------------------------------------------
    // Date

    fn write_date_as_ejson(
        &mut self,
        timestamp_ms: i64,
        iso: Option<&str>,
    ) -> Result<(), EjsonEncodeError> {
        // {"$date": ...}
        self.writer.buf(b"{\"$date\":");

        if self.options.canonical {
            // Always use {"$numberLong":"timestamp"}
            self.write_number_long_wrapper_i64(timestamp_ms);
        } else {
            // Relaxed: ISO string for years 1970-9999, else $numberLong
            let year = year_from_ms(timestamp_ms);
            if (1970..=9999).contains(&year) {
                if let Some(s) = iso {
                    self.write_str(s);
                } else {
                    // Fallback to timestamp
                    self.write_number_long_wrapper_i64(timestamp_ms);
                }
            } else {
                self.write_number_long_wrapper_i64(timestamp_ms);
            }
        }

        self.writer.u8(b'}');
        Ok(())
    }

    // ----------------------------------------------------------------
    // RegExp

    fn write_regexp_as_ejson(&mut self, source: &str, flags: &str) {
        // {"$regularExpression":{"pattern":"...","options":"..."}}
        self.writer.buf(b"{\"$regularExpression\":{\"pattern\":");
        self.write_str(source);
        self.writer.buf(b",\"options\":");
        self.write_str(flags);
        self.writer.buf(b"}}");
    }

    // ----------------------------------------------------------------
    // ObjectId

    fn write_object_id_as_ejson(&mut self, id: &BsonObjectId) {
        // {"$oid":"hexstring"}
        self.writer.buf(b"{\"$oid\":\"");
        let hex = object_id_to_hex(id);
        self.writer.ascii(&hex);
        self.writer.buf(b"\"}");
    }

    // ----------------------------------------------------------------
    // Numeric BSON types

    fn write_bson_int32_as_ejson(&mut self, v: &BsonInt32) {
        if self.options.canonical {
            self.write_number_int_wrapper(v.value as i64);
        } else {
            self.write_number(v.value as f64);
        }
    }

    fn write_bson_int64_as_ejson(&mut self, v: &BsonInt64) {
        if self.options.canonical {
            self.write_number_long_wrapper_i64(v.value);
        } else {
            self.write_number(v.value as f64);
        }
    }

    fn write_bson_float_as_ejson(&mut self, v: &BsonFloat) {
        if self.options.canonical || !v.value.is_finite() {
            self.write_number_double_wrapper(v.value);
        } else {
            self.write_number(v.value);
        }
    }

    fn write_bson_decimal128_as_ejson(&mut self, v: &BsonDecimal128) {
        // {"$numberDecimal":"..."}
        // Stub: return "0" for the decimal string (matches upstream's TODO comment)
        let s = decimal128_to_string(&v.data);
        self.writer.buf(b"{\"$numberDecimal\":\"");
        self.writer.ascii(&s);
        self.writer.buf(b"\"}");
    }

    // ----------------------------------------------------------------
    // Binary

    fn write_bson_binary_as_ejson(&mut self, v: &BsonBinary) {
        // {"$binary":{"base64":"...","subType":"XX"}}
        let b64 = json_joy_base64::to_base64(&v.data);
        let subtype = format!("{:02x}", v.subtype);
        self.writer.buf(b"{\"$binary\":{\"base64\":\"");
        self.writer.ascii(&b64);
        self.writer.buf(b"\",\"subType\":\"");
        self.writer.ascii(&subtype);
        self.writer.buf(b"\"}}");
    }

    // ----------------------------------------------------------------
    // Code

    fn write_bson_code_as_ejson(&mut self, v: &BsonJavascriptCode) {
        // {"$code":"..."}
        self.writer.buf(b"{\"$code\":");
        self.write_str(&v.code);
        self.writer.u8(b'}');
    }

    #[allow(dead_code)]
    fn write_bson_code_wscope_as_ejson(
        &mut self,
        code: &str,
        scope: &[(String, EjsonValue)],
    ) -> Result<(), EjsonEncodeError> {
        // {"$code":"...","$scope":{...}}
        self.writer.buf(b"{\"$code\":");
        self.write_str(code);
        self.writer.buf(b",\"$scope\":");
        self.write_obj(scope)?;
        self.writer.u8(b'}');
        Ok(())
    }

    /// Write CodeWithScope where the scope is a `Vec<(String, BsonValue)>`.
    fn write_bson_code_wscope_bson(
        &mut self,
        code: &str,
        scope: &[(String, BsonValue)],
    ) -> Result<(), EjsonEncodeError> {
        self.writer.buf(b"{\"$code\":");
        self.write_str(code);
        self.writer.buf(b",\"$scope\":");
        // Write scope as an object, converting each BsonValue to an EjsonValue
        if scope.is_empty() {
            self.writer.buf(b"{}");
        } else {
            self.writer.u8(b'{');
            for (i, (key, val)) in scope.iter().enumerate() {
                if i > 0 {
                    self.writer.u8(b',');
                }
                self.write_str(key);
                self.writer.u8(b':');
                let ejson_val = bson_to_ejson_value(val);
                self.write_any(&ejson_val)?;
            }
            self.writer.u8(b'}');
        }
        self.writer.u8(b'}');
        Ok(())
    }

    // ----------------------------------------------------------------
    // Symbol

    fn write_bson_symbol_as_ejson(&mut self, v: &BsonSymbol) {
        // {"$symbol":"..."}
        self.writer.buf(b"{\"$symbol\":");
        self.write_str(&v.symbol);
        self.writer.u8(b'}');
    }

    // ----------------------------------------------------------------
    // Timestamp

    fn write_bson_timestamp_as_ejson(&mut self, v: &BsonTimestamp) {
        // {"$timestamp":{"t":...,"i":...}}
        self.writer.buf(b"{\"$timestamp\":{\"t\":");
        self.write_number(v.timestamp as f64);
        self.writer.buf(b",\"i\":");
        self.write_number(v.increment as f64);
        self.writer.buf(b"}}");
    }

    // ----------------------------------------------------------------
    // DbPointer

    fn write_bson_db_pointer_as_ejson(
        &mut self,
        v: &BsonDbPointer,
    ) -> Result<(), EjsonEncodeError> {
        // {"$dbPointer":{"$ref":"...","$id":{"$oid":"..."}}}
        self.writer.buf(b"{\"$dbPointer\":{\"$ref\":");
        self.write_str(&v.name);
        self.writer.buf(b",\"$id\":");
        self.write_object_id_as_ejson(&v.id);
        self.writer.buf(b"}}");
        Ok(())
    }

    // ----------------------------------------------------------------
    // MinKey / MaxKey

    fn write_bson_min_key_as_ejson(&mut self) {
        self.writer.buf(b"{\"$minKey\":1}");
    }

    fn write_bson_max_key_as_ejson(&mut self) {
        self.writer.buf(b"{\"$maxKey\":1}");
    }

    // ----------------------------------------------------------------
    // Undefined

    fn write_undefined_wrapper(&mut self) {
        self.writer.buf(b"{\"$undefined\":true}");
    }
}

// ----------------------------------------------------------------
// Utility functions

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn format_integer_f64(n: f64) -> String {
    // Large integer stored as f64 — format without decimal point
    format!("{}", n as i64)
}

fn format_non_finite(n: f64) -> String {
    if n == f64::INFINITY {
        "Infinity".to_string()
    } else if n == f64::NEG_INFINITY {
        "-Infinity".to_string()
    } else {
        "NaN".to_string()
    }
}

fn object_id_to_hex(id: &BsonObjectId) -> String {
    // 4-byte timestamp (8 hex) + 5-byte process (10 hex) + 3-byte counter (6 hex) = 24 hex chars
    format!("{:08x}{:010x}{:06x}", id.timestamp, id.process, id.counter)
}

/// Estimate the calendar year from a Unix epoch millisecond timestamp.
/// This is a simplified approximation sufficient for the 1970-9999 range check.
fn year_from_ms(ms: i64) -> i64 {
    // ~365.25 days/year * 24h * 60m * 60s * 1000ms
    const MS_PER_YEAR: i64 = 31_557_600_000;
    1970 + ms / MS_PER_YEAR
}

/// Simplified decimal128 to string conversion.
/// Upstream also uses a stub returning "0" (TODO comment in EjsonEncoder.ts).
fn decimal128_to_string(_data: &[u8]) -> String {
    "0".to_string()
}

/// Convert a `BsonValue` to an `EjsonValue` for encoding scope fields.
fn bson_to_ejson_value(v: &BsonValue) -> EjsonValue {
    match v {
        BsonValue::Null => EjsonValue::Null,
        BsonValue::Boolean(b) => EjsonValue::Bool(*b),
        BsonValue::Int32(i) => EjsonValue::Int32(BsonInt32 { value: *i }),
        BsonValue::Int64(i) => EjsonValue::Int64(BsonInt64 { value: *i }),
        BsonValue::Float(f) => EjsonValue::BsonFloat(BsonFloat { value: *f }),
        BsonValue::Str(s) => EjsonValue::Str(s.clone()),
        BsonValue::ObjectId(id) => EjsonValue::ObjectId(id.clone()),
        BsonValue::Binary(b) => EjsonValue::Binary(b.clone()),
        BsonValue::DateTime(ms) => EjsonValue::Date {
            timestamp_ms: *ms,
            iso: None,
        },
        BsonValue::Symbol(s) => EjsonValue::Symbol(s.clone()),
        BsonValue::Timestamp(t) => EjsonValue::Timestamp(t.clone()),
        BsonValue::Array(arr) => EjsonValue::Array(arr.iter().map(bson_to_ejson_value).collect()),
        BsonValue::Document(fields) => EjsonValue::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_ejson_value(v)))
                .collect(),
        ),
        _ => EjsonValue::Null,
    }
}
