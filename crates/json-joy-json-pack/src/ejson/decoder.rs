//! EJSON v2 (MongoDB Extended JSON) decoder.
//!
//! Upstream reference: `json-pack/src/ejson/EjsonDecoder.ts`
//!
//! Parses UTF-8 JSON bytes and returns an `EjsonValue`, recognising the
//! MongoDB Extended JSON v2 `$`-prefixed type wrapper objects.

use crate::bson::{
    BsonBinary, BsonDbPointer, BsonDecimal128, BsonFloat, BsonInt32, BsonInt64, BsonJavascriptCode,
    BsonJavascriptCodeWithScope, BsonMaxKey, BsonMinKey, BsonObjectId, BsonSymbol, BsonTimestamp,
    BsonValue,
};

use super::error::EjsonDecodeError;
use super::value::EjsonValue;

// ----------------------------------------------------------------
// Decoder state

/// EJSON decoder — reads Extended JSON bytes and produces `EjsonValue`.
pub struct EjsonDecoder {
    data: Vec<u8>,
    x: usize,
}

impl Default for EjsonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EjsonDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            x: 0,
        }
    }

    /// Decode from bytes.
    pub fn decode(&mut self, input: &[u8]) -> Result<EjsonValue, EjsonDecodeError> {
        self.data = input.to_vec();
        self.x = 0;
        self.read_any()
    }

    /// Convenience: decode from a UTF-8 string.
    pub fn decode_str(&mut self, s: &str) -> Result<EjsonValue, EjsonDecodeError> {
        self.decode(s.as_bytes())
    }

    // ----------------------------------------------------------------
    // Core read dispatch

    fn read_any(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        self.skip_ws();
        let x = self.x;
        if x >= self.data.len() {
            return Err(EjsonDecodeError::InvalidJson(x));
        }
        match self.data[x] {
            b'"' => Ok(EjsonValue::Str(self.read_string()?)),
            b'[' => self.read_array(),
            b'f' => self.read_false(),
            b'n' => self.read_null(),
            b't' => self.read_true(),
            b'{' => self.read_obj_with_ejson(),
            c if (c >= b'0' && c <= b'9') || c == b'-' => self.read_num(),
            _ => Err(EjsonDecodeError::InvalidJson(x)),
        }
    }

    // ----------------------------------------------------------------
    // Primitives

    fn skip_ws(&mut self) {
        while self.x < self.data.len() {
            match self.data[self.x] {
                b' ' | b'\t' | b'\n' | b'\r' => self.x += 1,
                _ => break,
            }
        }
    }

    fn read_null(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        if self.x + 4 > self.data.len() || &self.data[self.x..self.x + 4] != b"null" {
            return Err(EjsonDecodeError::InvalidJson(self.x));
        }
        self.x += 4;
        Ok(EjsonValue::Null)
    }

    fn read_true(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        if self.x + 4 > self.data.len() || &self.data[self.x..self.x + 4] != b"true" {
            return Err(EjsonDecodeError::InvalidJson(self.x));
        }
        self.x += 4;
        Ok(EjsonValue::Bool(true))
    }

    fn read_false(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        if self.x + 5 > self.data.len() || &self.data[self.x..self.x + 5] != b"false" {
            return Err(EjsonDecodeError::InvalidJson(self.x));
        }
        self.x += 5;
        Ok(EjsonValue::Bool(false))
    }

    fn read_num(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        let start = self.x;
        let len = self.data.len();
        let mut x = self.x;
        if x < len && self.data[x] == b'-' {
            x += 1;
        }
        while x < len && self.data[x] >= b'0' && self.data[x] <= b'9' {
            x += 1;
        }
        let mut is_float = false;
        if x < len && self.data[x] == b'.' {
            is_float = true;
            x += 1;
            while x < len && self.data[x] >= b'0' && self.data[x] <= b'9' {
                x += 1;
            }
        }
        if x < len && (self.data[x] == b'e' || self.data[x] == b'E') {
            is_float = true;
            x += 1;
            if x < len && (self.data[x] == b'+' || self.data[x] == b'-') {
                x += 1;
            }
            while x < len && self.data[x] >= b'0' && self.data[x] <= b'9' {
                x += 1;
            }
        }
        self.x = x;
        let s =
            std::str::from_utf8(&self.data[start..x]).map_err(|_| EjsonDecodeError::InvalidUtf8)?;
        if is_float {
            let f: f64 = s
                .parse()
                .map_err(|_| EjsonDecodeError::InvalidJson(start))?;
            Ok(EjsonValue::Float(f))
        } else if let Ok(i) = s.parse::<i64>() {
            Ok(EjsonValue::Integer(i))
        } else if let Ok(f) = s.parse::<f64>() {
            // Very large integer that overflows i64
            Ok(EjsonValue::Float(f))
        } else {
            Err(EjsonDecodeError::InvalidJson(start))
        }
    }

    fn read_string(&mut self) -> Result<String, EjsonDecodeError> {
        let data = &self.data;
        if self.x >= data.len() || data[self.x] != b'"' {
            return Err(EjsonDecodeError::InvalidJson(self.x));
        }
        self.x += 1;
        let start = self.x;
        let end = self.find_end_quote(start)?;
        let slice = &data[start..end];
        let s = decode_json_string(slice)?;
        self.x = end + 1; // skip closing quote
        Ok(s)
    }

    fn find_end_quote(&self, start: usize) -> Result<usize, EjsonDecodeError> {
        let data = &self.data;
        let mut i = start;
        while i < data.len() {
            match data[i] {
                b'\\' => i += 2, // skip escaped char
                b'"' => return Ok(i),
                _ => i += 1,
            }
        }
        Err(EjsonDecodeError::InvalidJson(start))
    }

    fn read_array(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        let x = self.x;
        if x >= self.data.len() || self.data[x] != b'[' {
            return Err(EjsonDecodeError::InvalidJson(x));
        }
        self.x += 1;
        let mut arr = Vec::new();
        let mut first = true;
        loop {
            self.skip_ws();
            if self.x >= self.data.len() {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            let ch = self.data[self.x];
            if ch == b']' {
                self.x += 1;
                return Ok(EjsonValue::Array(arr));
            }
            if ch == b',' {
                self.x += 1;
            } else if !first {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            self.skip_ws();
            arr.push(self.read_any()?);
            first = false;
        }
    }

    // ----------------------------------------------------------------
    // Object / EJSON dispatch

    /// Read a JSON object, transforming EJSON type wrappers.
    fn read_obj_with_ejson(&mut self) -> Result<EjsonValue, EjsonDecodeError> {
        let x = self.x;
        if x >= self.data.len() || self.data[x] != b'{' {
            return Err(EjsonDecodeError::InvalidJson(x));
        }
        self.x += 1;

        // Read all key-value pairs as raw EjsonValue
        let mut pairs: Vec<(String, EjsonValue)> = Vec::new();
        let mut first = true;
        loop {
            self.skip_ws();
            if self.x >= self.data.len() {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            let ch = self.data[self.x];
            if ch == b'}' {
                self.x += 1;
                break;
            }
            if ch == b',' {
                self.x += 1;
            } else if !first {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            self.skip_ws();
            if self.x >= self.data.len() || self.data[self.x] != b'"' {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            let key = self.read_string()?;
            if key == "__proto__" {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            self.skip_ws();
            if self.x >= self.data.len() || self.data[self.x] != b':' {
                return Err(EjsonDecodeError::InvalidJson(self.x));
            }
            self.x += 1;
            self.skip_ws();
            let val = self.read_any()?;
            pairs.push((key, val));
            first = false;
        }

        self.transform_ejson_object(pairs)
    }

    // ----------------------------------------------------------------
    // EJSON type wrapper transformation

    fn transform_ejson_object(
        &self,
        pairs: Vec<(String, EjsonValue)>,
    ) -> Result<EjsonValue, EjsonDecodeError> {
        // Find $ keys
        let dollar_keys: Vec<&str> = pairs
            .iter()
            .filter(|(k, _)| k.starts_with('$'))
            .map(|(k, _)| k.as_str())
            .collect();

        if !dollar_keys.is_empty() {
            // Helper: get single value for key, error if not exactly the right keys
            let has_exact = |expected: &[&str]| -> bool {
                if pairs.len() != expected.len() {
                    return false;
                }
                expected.iter().all(|k| pairs.iter().any(|(pk, _)| pk == k))
            };
            let get = |key: &str| -> Option<&EjsonValue> {
                pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            };

            // $oid
            if dollar_keys.contains(&"$oid") {
                if !has_exact(&["$oid"]) {
                    return Err(EjsonDecodeError::ExtraKeys("ObjectId"));
                }
                if let Some(EjsonValue::Str(s)) = get("$oid") {
                    if s.len() == 24
                        && s.bytes()
                            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F'))
                    {
                        return Ok(EjsonValue::ObjectId(parse_object_id(s)));
                    }
                }
                return Err(EjsonDecodeError::InvalidObjectId);
            }

            // $numberInt
            if dollar_keys.contains(&"$numberInt") {
                if !has_exact(&["$numberInt"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Int32"));
                }
                if let Some(EjsonValue::Str(s)) = get("$numberInt") {
                    if let Ok(v) = s.parse::<i32>() {
                        return Ok(EjsonValue::Int32(BsonInt32 { value: v }));
                    }
                }
                return Err(EjsonDecodeError::InvalidInt32);
            }

            // $numberLong
            if dollar_keys.contains(&"$numberLong") {
                if !has_exact(&["$numberLong"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Int64"));
                }
                if let Some(EjsonValue::Str(s)) = get("$numberLong") {
                    // Use parse::<f64> to handle large numbers (matching upstream's parseFloat)
                    if let Ok(v) = s.parse::<f64>() {
                        if !v.is_nan() {
                            return Ok(EjsonValue::Int64(BsonInt64 { value: v as i64 }));
                        }
                    }
                }
                return Err(EjsonDecodeError::InvalidInt64);
            }

            // $numberDouble
            if dollar_keys.contains(&"$numberDouble") {
                if !has_exact(&["$numberDouble"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Double"));
                }
                if let Some(EjsonValue::Str(s)) = get("$numberDouble") {
                    let v = match s.as_str() {
                        "Infinity" => f64::INFINITY,
                        "-Infinity" => f64::NEG_INFINITY,
                        "NaN" => f64::NAN,
                        other => {
                            let parsed: f64 =
                                other.parse().map_err(|_| EjsonDecodeError::InvalidDouble)?;
                            if parsed.is_nan() {
                                return Err(EjsonDecodeError::InvalidDouble);
                            }
                            parsed
                        }
                    };
                    return Ok(EjsonValue::BsonFloat(BsonFloat { value: v }));
                }
                return Err(EjsonDecodeError::InvalidDouble);
            }

            // $numberDecimal
            if dollar_keys.contains(&"$numberDecimal") {
                if !has_exact(&["$numberDecimal"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Decimal128"));
                }
                if let Some(EjsonValue::Str(_)) = get("$numberDecimal") {
                    // Return a zero 16-byte Decimal128 (same stub as upstream)
                    return Ok(EjsonValue::Decimal128(BsonDecimal128 {
                        data: vec![0u8; 16],
                    }));
                }
                return Err(EjsonDecodeError::InvalidDecimal128);
            }

            // $binary
            if dollar_keys.contains(&"$binary") {
                if !has_exact(&["$binary"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Binary"));
                }
                if let Some(EjsonValue::Object(inner)) = get("$binary") {
                    let has_b64 = inner.iter().any(|(k, _)| k == "base64");
                    let has_sub = inner.iter().any(|(k, _)| k == "subType");
                    if inner.len() == 2 && has_b64 && has_sub {
                        let b64 = inner.iter().find(|(k, _)| k == "base64").map(|(_, v)| v);
                        let sub = inner.iter().find(|(k, _)| k == "subType").map(|(_, v)| v);
                        if let (Some(EjsonValue::Str(b64s)), Some(EjsonValue::Str(subs))) =
                            (b64, sub)
                        {
                            let data =
                                base64_to_bytes(b64s).ok_or(EjsonDecodeError::InvalidBinary)?;
                            let subtype = u8::from_str_radix(subs, 16)
                                .map_err(|_| EjsonDecodeError::InvalidBinary)?;
                            return Ok(EjsonValue::Binary(BsonBinary { subtype, data }));
                        }
                    }
                }
                return Err(EjsonDecodeError::InvalidBinary);
            }

            // $uuid
            if dollar_keys.contains(&"$uuid") {
                if !has_exact(&["$uuid"]) {
                    return Err(EjsonDecodeError::ExtraKeys("UUID"));
                }
                if let Some(EjsonValue::Str(s)) = get("$uuid") {
                    if is_valid_uuid(s) {
                        let data = uuid_to_bytes(s);
                        return Ok(EjsonValue::Binary(BsonBinary { subtype: 4, data }));
                    }
                }
                return Err(EjsonDecodeError::InvalidUuid);
            }

            // $code (without $scope)
            if dollar_keys.contains(&"$code") && !dollar_keys.contains(&"$scope") {
                if !has_exact(&["$code"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Code"));
                }
                if let Some(EjsonValue::Str(s)) = get("$code") {
                    return Ok(EjsonValue::Code(BsonJavascriptCode { code: s.clone() }));
                }
                return Err(EjsonDecodeError::InvalidCode);
            }

            // $code + $scope (CodeWithScope)
            if dollar_keys.contains(&"$code") && dollar_keys.contains(&"$scope") {
                if !has_exact(&["$code", "$scope"]) {
                    return Err(EjsonDecodeError::ExtraKeys("CodeWScope"));
                }
                let code = match get("$code") {
                    Some(EjsonValue::Str(s)) => s.clone(),
                    _ => return Err(EjsonDecodeError::InvalidCodeWithScope),
                };
                let scope_pairs = match get("$scope") {
                    Some(EjsonValue::Object(obj)) => obj.clone(),
                    _ => return Err(EjsonDecodeError::InvalidCodeWithScope),
                };
                // Convert scope EjsonValue pairs to BsonValue pairs
                let bson_scope: Vec<(String, BsonValue)> = scope_pairs
                    .into_iter()
                    .map(|(k, v)| Ok((k, ejson_to_bson_value(v)?)))
                    .collect::<Result<_, EjsonDecodeError>>()?;
                return Ok(EjsonValue::CodeWithScope(BsonJavascriptCodeWithScope {
                    code,
                    scope: bson_scope,
                }));
            }

            // $symbol
            if dollar_keys.contains(&"$symbol") {
                if !has_exact(&["$symbol"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Symbol"));
                }
                if let Some(EjsonValue::Str(s)) = get("$symbol") {
                    return Ok(EjsonValue::Symbol(BsonSymbol { symbol: s.clone() }));
                }
                return Err(EjsonDecodeError::InvalidSymbol);
            }

            // $timestamp
            if dollar_keys.contains(&"$timestamp") {
                if !has_exact(&["$timestamp"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Timestamp"));
                }
                if let Some(EjsonValue::Object(inner)) = get("$timestamp") {
                    let has_t = inner.iter().any(|(k, _)| k == "t");
                    let has_i = inner.iter().any(|(k, _)| k == "i");
                    if inner.len() == 2 && has_t && has_i {
                        let t_val = inner.iter().find(|(k, _)| k == "t").map(|(_, v)| v);
                        let i_val = inner.iter().find(|(k, _)| k == "i").map(|(_, v)| v);
                        match (t_val, i_val) {
                            (Some(EjsonValue::Integer(t)), Some(EjsonValue::Integer(i)))
                                if *t >= 0 && *i >= 0 =>
                            {
                                return Ok(EjsonValue::Timestamp(BsonTimestamp {
                                    timestamp: *t as i32,
                                    increment: *i as i32,
                                }));
                            }
                            _ => {}
                        }
                    }
                }
                return Err(EjsonDecodeError::InvalidTimestamp);
            }

            // $regularExpression
            if dollar_keys.contains(&"$regularExpression") {
                if !has_exact(&["$regularExpression"]) {
                    return Err(EjsonDecodeError::ExtraKeys("RegularExpression"));
                }
                if let Some(EjsonValue::Object(inner)) = get("$regularExpression") {
                    let has_pat = inner.iter().any(|(k, _)| k == "pattern");
                    let has_opt = inner.iter().any(|(k, _)| k == "options");
                    if inner.len() == 2 && has_pat && has_opt {
                        let pat = inner.iter().find(|(k, _)| k == "pattern").map(|(_, v)| v);
                        let opt = inner.iter().find(|(k, _)| k == "options").map(|(_, v)| v);
                        if let (Some(EjsonValue::Str(p)), Some(EjsonValue::Str(o))) = (pat, opt) {
                            return Ok(EjsonValue::RegExp(p.clone(), o.clone()));
                        }
                    }
                }
                return Err(EjsonDecodeError::InvalidRegularExpression);
            }

            // $dbPointer
            if dollar_keys.contains(&"$dbPointer") {
                if !has_exact(&["$dbPointer"]) {
                    return Err(EjsonDecodeError::ExtraKeys("DBPointer"));
                }
                if let Some(EjsonValue::Object(inner)) = get("$dbPointer") {
                    let has_ref = inner.iter().any(|(k, _)| k == "$ref");
                    let has_id = inner.iter().any(|(k, _)| k == "$id");
                    if inner.len() == 2 && has_ref && has_id {
                        let ref_val = inner.iter().find(|(k, _)| k == "$ref").map(|(_, v)| v);
                        let id_val = inner.iter().find(|(k, _)| k == "$id").map(|(_, v)| v);
                        if let (Some(EjsonValue::Str(name)), Some(EjsonValue::ObjectId(oid))) =
                            (ref_val, id_val)
                        {
                            return Ok(EjsonValue::DbPointer(BsonDbPointer {
                                name: name.clone(),
                                id: oid.clone(),
                            }));
                        }
                    }
                }
                return Err(EjsonDecodeError::InvalidDbPointer);
            }

            // $date
            if dollar_keys.contains(&"$date") {
                if !has_exact(&["$date"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Date"));
                }
                match get("$date") {
                    Some(EjsonValue::Str(s)) => {
                        // ISO-8601 string (relaxed mode)
                        match parse_iso_date(s) {
                            Some(ms) => {
                                return Ok(EjsonValue::Date {
                                    timestamp_ms: ms,
                                    iso: None,
                                })
                            }
                            None => return Err(EjsonDecodeError::InvalidDate),
                        }
                    }
                    // Canonical: {"$numberLong":"timestamp"} was already decoded to Int64
                    Some(EjsonValue::Int64(v)) => {
                        return Ok(EjsonValue::Date {
                            timestamp_ms: v.value,
                            iso: None,
                        });
                    }
                    Some(EjsonValue::Integer(ms)) => {
                        return Ok(EjsonValue::Date {
                            timestamp_ms: *ms,
                            iso: None,
                        });
                    }
                    Some(EjsonValue::Object(inner)) => {
                        // Canonical: {"$numberLong":"timestamp"} (not yet transformed)
                        if inner.len() == 1 {
                            if let Some((k, EjsonValue::Str(s))) = inner.first() {
                                if k == "$numberLong" {
                                    if let Ok(ms) = s.parse::<f64>() {
                                        if !ms.is_nan() {
                                            return Ok(EjsonValue::Date {
                                                timestamp_ms: ms as i64,
                                                iso: None,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        return Err(EjsonDecodeError::InvalidDate);
                    }
                    _ => return Err(EjsonDecodeError::InvalidDate),
                }
            }

            // $minKey
            if dollar_keys.contains(&"$minKey") {
                if !has_exact(&["$minKey"]) {
                    return Err(EjsonDecodeError::ExtraKeys("MinKey"));
                }
                if matches!(get("$minKey"), Some(EjsonValue::Integer(1))) {
                    return Ok(EjsonValue::MinKey(BsonMinKey));
                }
                return Err(EjsonDecodeError::InvalidMinKey);
            }

            // $maxKey
            if dollar_keys.contains(&"$maxKey") {
                if !has_exact(&["$maxKey"]) {
                    return Err(EjsonDecodeError::ExtraKeys("MaxKey"));
                }
                if matches!(get("$maxKey"), Some(EjsonValue::Integer(1))) {
                    return Ok(EjsonValue::MaxKey(BsonMaxKey));
                }
                return Err(EjsonDecodeError::InvalidMaxKey);
            }

            // $undefined
            if dollar_keys.contains(&"$undefined") {
                if !has_exact(&["$undefined"]) {
                    return Err(EjsonDecodeError::ExtraKeys("Undefined"));
                }
                if matches!(get("$undefined"), Some(EjsonValue::Bool(true))) {
                    return Ok(EjsonValue::Undefined);
                }
                return Err(EjsonDecodeError::InvalidUndefined);
            }
        }

        // DBRef convention: object with $ref + $id (may have additional fields)
        let has_ref = pairs.iter().any(|(k, _)| k == "$ref");
        let has_id = pairs.iter().any(|(k, _)| k == "$id");
        if has_ref && has_id {
            // Pass through as an object, but transform the $id value
            let mut result: Vec<(String, EjsonValue)> = Vec::new();
            for (key, val) in pairs {
                if key == "$id" {
                    // Transform $id (should be an ObjectId or nested EJSON)
                    let transformed = self.transform_ejson_value(val)?;
                    result.push(("$id".to_string(), transformed));
                } else {
                    result.push((key, val));
                }
            }
            return Ok(EjsonValue::Object(result));
        }

        // Regular object: recursively transform nested objects
        let mut result = Vec::with_capacity(pairs.len());
        for (key, val) in pairs {
            let transformed = self.transform_ejson_value(val)?;
            result.push((key, transformed));
        }
        Ok(EjsonValue::Object(result))
    }

    /// Re-dispatch a value that was read as raw, in case it is a nested EJSON object.
    fn transform_ejson_value(&self, value: EjsonValue) -> Result<EjsonValue, EjsonDecodeError> {
        match value {
            EjsonValue::Object(pairs) => self.transform_ejson_object(pairs),
            EjsonValue::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.transform_ejson_value(item)?);
                }
                Ok(EjsonValue::Array(out))
            }
            other => Ok(other),
        }
    }
}

// ----------------------------------------------------------------
// Utility functions

fn parse_object_id(hex: &str) -> BsonObjectId {
    // 24-char hex → 4-byte timestamp + 5-byte process + 3-byte counter
    let timestamp = u32::from_str_radix(&hex[0..8], 16).unwrap_or(0);
    let process = u64::from_str_radix(&hex[8..18], 16).unwrap_or(0);
    let counter = u32::from_str_radix(&hex[18..24], 16).unwrap_or(0);
    BsonObjectId {
        timestamp,
        process,
        counter,
    }
}

fn base64_to_bytes(b64: &str) -> Option<Vec<u8>> {
    json_joy_base64::from_base64(b64).ok()
}

fn is_valid_uuid(s: &str) -> bool {
    // xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let dashes = [8, 13, 18, 23];
    for (i, &b) in bytes.iter().enumerate() {
        if dashes.contains(&i) {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn uuid_to_bytes(s: &str) -> Vec<u8> {
    let hex: String = s.chars().filter(|&c| c != '-').collect();
    (0..16)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect()
}

/// Parse an ISO 8601 date string into milliseconds since Unix epoch.
/// Uses a simple approach via Rust's `chrono`-free path by leveraging the
/// known format `YYYY-MM-DDTHH:MM:SS.mmmZ`.
fn parse_iso_date(s: &str) -> Option<i64> {
    // We use a minimal parser for the most common formats.
    // Full ISO 8601 is complex; for EJSON use-cases the format is stable.
    parse_iso_to_ms(s)
}

fn parse_iso_to_ms(s: &str) -> Option<i64> {
    // Support: "YYYY-MM-DDTHH:MM:SS.mmmZ" and "YYYY-MM-DDTHH:MM:SSZ"
    let bytes = s.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }

    let year: i64 = parse_digits(s, 0, 4)?;
    let month: i64 = parse_digits(s, 5, 7)?;
    let day: i64 = parse_digits(s, 8, 10)?;
    let hour: i64 = parse_digits(s, 11, 13)?;
    let min: i64 = parse_digits(s, 14, 16)?;
    let sec: i64 = parse_digits(s, 17, 19)?;

    // ms
    let ms: i64 = if bytes.len() > 20 && bytes[19] == b'.' {
        let ms_start = 20;
        let ms_end = bytes[ms_start..]
            .iter()
            .position(|&b| !b.is_ascii_digit())
            .map(|p| ms_start + p)
            .unwrap_or(bytes.len());
        let ms_str = &s[ms_start..ms_end];
        let raw: i64 = ms_str.parse().ok()?;
        // Normalize to milliseconds
        match ms_str.len() {
            1 => raw * 100,
            2 => raw * 10,
            3 => raw,
            _ => raw / 10i64.pow((ms_str.len() as u32).saturating_sub(3)),
        }
    } else {
        0
    };

    // Convert date components to days since epoch, then to ms
    // Using the proleptic Gregorian calendar algorithm
    let days = days_from_civil(year, month, day)?;
    let total_seconds = days * 86400 + hour * 3600 + min * 60 + sec;
    Some(total_seconds * 1000 + ms)
}

fn parse_digits(s: &str, start: usize, end: usize) -> Option<i64> {
    s.get(start..end)?.parse().ok()
}

/// Convert civil date to number of days since Unix epoch (1970-01-01).
/// Algorithm from Howard Hinnant's date library.
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if m < 1 || m > 12 || d < 1 || d > 31 {
        return None;
    }
    let yy = if m <= 2 { y - 1 } else { y };
    let mm = if m <= 2 { m + 9 } else { m - 3 };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400; // [0, 399]
    let doy = (153 * mm + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146097 + doe - 719468; // days since 1970-01-01
    Some(days)
}

/// Decode a JSON string body (content between quotes), handling escape sequences.
fn decode_json_string(bytes: &[u8]) -> Result<String, EjsonDecodeError> {
    if !bytes.contains(&b'\\') {
        return std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| EjsonDecodeError::InvalidUtf8);
    }
    // Wrap in quotes and use serde_json for proper unescaping
    let mut quoted = Vec::with_capacity(bytes.len() + 2);
    quoted.push(b'"');
    quoted.extend_from_slice(bytes);
    quoted.push(b'"');
    let s: String =
        serde_json::from_slice(&quoted).map_err(|_| EjsonDecodeError::InvalidJson(0))?;
    Ok(s)
}

/// Convert an `EjsonValue` to a `BsonValue` for use in CodeWithScope scopes.
fn ejson_to_bson_value(v: EjsonValue) -> Result<BsonValue, EjsonDecodeError> {
    match v {
        EjsonValue::Null => Ok(BsonValue::Null),
        EjsonValue::Bool(b) => Ok(BsonValue::Boolean(b)),
        EjsonValue::Integer(i) => Ok(BsonValue::Int64(i)),
        EjsonValue::Float(f) => Ok(BsonValue::Float(f)),
        EjsonValue::Str(s) => Ok(BsonValue::Str(s)),
        EjsonValue::Array(arr) => {
            let bson_arr: Result<Vec<BsonValue>, _> =
                arr.into_iter().map(ejson_to_bson_value).collect();
            Ok(BsonValue::Array(bson_arr?))
        }
        EjsonValue::Object(pairs) => {
            let bson_pairs: Result<Vec<(String, BsonValue)>, _> = pairs
                .into_iter()
                .map(|(k, v)| ejson_to_bson_value(v).map(|bv| (k, bv)))
                .collect();
            Ok(BsonValue::Document(bson_pairs?))
        }
        EjsonValue::Number(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                Ok(BsonValue::Int64(f as i64))
            } else {
                Ok(BsonValue::Float(f))
            }
        }
        EjsonValue::Int32(v) => Ok(BsonValue::Int32(v.value)),
        EjsonValue::Int64(v) => Ok(BsonValue::Int64(v.value)),
        EjsonValue::BsonFloat(v) => Ok(BsonValue::Float(v.value)),
        EjsonValue::ObjectId(v) => Ok(BsonValue::ObjectId(v)),
        EjsonValue::Binary(v) => Ok(BsonValue::Binary(v)),
        EjsonValue::Date {
            timestamp_ms: ms, ..
        } => Ok(BsonValue::DateTime(ms)),
        EjsonValue::Symbol(v) => Ok(BsonValue::Symbol(v)),
        EjsonValue::Timestamp(v) => Ok(BsonValue::Timestamp(v)),
        // Fall back to null for types that don't map cleanly
        _ => Ok(BsonValue::Null),
    }
}
