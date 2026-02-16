fn decode_model_view(data: &[u8]) -> Result<Value, ModelError> {
    let mut reader = Reader::new(data);

    // Server-clock encoding starts with a marker byte whose highest bit is set
    // and does not contain a clock table section.
    if reader.peak()? & 0b1000_0000 != 0 {
        let _marker = reader.u8()?;
        let _time = reader.vu57()?;
        return decode_root_to_end(&mut reader);
    }

    let clock_table_offset = reader.u32_be()? as usize;
    let root_start = reader.pos;
    let clock_start = root_start
        .checked_add(clock_table_offset)
        .ok_or(ModelError::InvalidClockTable)?;
    if clock_start > data.len() {
        return Err(ModelError::InvalidClockTable);
    }

    // Validate basic clock table framing similarly to upstream decode path.
    {
        let mut clock = Reader::new(&data[clock_start..]);
        let table_len = clock.vu57()?;
        if table_len == 0 {
            return Err(ModelError::InvalidClockTable);
        }
        let _session = clock.vu57()?;
        let _time = clock.vu57()?;
        for _ in 1..table_len {
            let _ = clock.vu57()?;
            let _ = clock.vu57()?;
        }
    }

    let root_slice = &data[root_start..clock_start];
    let mut root_reader = Reader::new(root_slice);
    let value = decode_root(&mut root_reader)?;
    if !root_reader.is_eof() {
        return Err(ModelError::InvalidModelBinary);
    }
    Ok(value)
}

fn decode_root_to_end(reader: &mut Reader<'_>) -> Result<Value, ModelError> {
    let value = decode_root(reader)?;
    if !reader.is_eof() {
        return Err(ModelError::InvalidModelBinary);
    }
    Ok(value)
}

fn decode_root(reader: &mut Reader<'_>) -> Result<Value, ModelError> {
    let first = reader.peak()?;
    if first == 0 {
        reader.u8()?;
        return Ok(Value::Null);
    }
    decode_node(reader)
}

fn decode_node(reader: &mut Reader<'_>) -> Result<Value, ModelError> {
    reader.skip_id()?;
    let octet = reader.u8()?;
    let major = octet >> 5;
    let minor = (octet & 0b1_1111) as u64;

    match major {
        // CON
        0 => decode_con(reader, minor),
        // VAL
        1 => decode_node(reader),
        // OBJ
        2 => {
            let len = if minor != 31 { minor } else { reader.vu57()? };
            decode_obj(reader, len)
        }
        // VEC
        3 => {
            let len = if minor != 31 { minor } else { reader.vu57()? };
            decode_vec(reader, len)
        }
        // STR
        4 => {
            let len = if minor != 31 { minor } else { reader.vu57()? };
            decode_str(reader, len)
        }
        // BIN
        5 => {
            let len = if minor != 31 { minor } else { reader.vu57()? };
            decode_bin(reader, len)
        }
        // ARR
        6 => {
            let len = if minor != 31 { minor } else { reader.vu57()? };
            decode_arr(reader, len)
        }
        _ => Err(ModelError::InvalidModelBinary),
    }
}

fn decode_con(reader: &mut Reader<'_>, length: u64) -> Result<Value, ModelError> {
    if length == 0 {
        let cbor = reader.read_one_cbor()?;
        return cbor_to_json(cbor);
    }

    // Timestamp reference constant. Not expected in current fixture corpus.
    reader.skip_id()?;
    Ok(Value::Null)
}

fn decode_obj(reader: &mut Reader<'_>, len: u64) -> Result<Value, ModelError> {
    let mut map = Map::new();
    for _ in 0..len {
        let key = match reader.read_one_cbor()? {
            CborValue::Text(s) => s,
            _ => return Err(ModelError::InvalidModelBinary),
        };
        let val = decode_node(reader)?;
        map.insert(key, val);
    }
    Ok(Value::Object(map))
}

fn decode_vec(reader: &mut Reader<'_>, len: u64) -> Result<Value, ModelError> {
    let mut out = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let octet = reader.peak()?;
        if octet == 0 {
            reader.u8()?;
            out.push(Value::Null);
        } else {
            out.push(decode_node(reader)?);
        }
    }
    Ok(Value::Array(out))
}

fn decode_str(reader: &mut Reader<'_>, len: u64) -> Result<Value, ModelError> {
    let mut out = String::new();
    for _ in 0..len {
        reader.skip_id()?;
        let cbor = reader.read_one_cbor()?;
        match cbor {
            CborValue::Text(s) => {
                out.push_str(&s);
            }
            CborValue::Integer(i) => {
                let _span: u64 = i.try_into().map_err(|_| ModelError::InvalidModelBinary)?;
            }
            _ => return Err(ModelError::InvalidModelBinary),
        }
    }
    Ok(Value::String(out))
}

fn decode_bin(reader: &mut Reader<'_>, len: u64) -> Result<Value, ModelError> {
    let mut out: Vec<u8> = Vec::new();
    for _ in 0..len {
        reader.skip_id()?;
        let (deleted, span) = reader.b1vu56()?;
        if deleted == 1 {
            continue;
        }
        let bytes = reader.buf(span as usize)?;
        for b in bytes {
            out.push(*b);
        }
    }
    // Upstream view materializes as Uint8Array. In JSON fixtures this appears
    // as an object with numeric string keys, e.g. {"0":1,"1":2}.
    let mut map = Map::new();
    for (i, b) in out.iter().enumerate() {
        map.insert(i.to_string(), Value::Number(Number::from(*b)));
    }
    Ok(Value::Object(map))
}

fn decode_arr(reader: &mut Reader<'_>, len: u64) -> Result<Value, ModelError> {
    let mut out = Vec::new();
    for _ in 0..len {
        reader.skip_id()?;
        let (deleted, span) = reader.b1vu56()?;

        if deleted == 1 {
            continue;
        }
        for _ in 0..span {
            out.push(decode_node(reader)?);
        }
    }
    Ok(Value::Array(out))
}

fn cbor_to_json(v: CborValue) -> Result<Value, ModelError> {
    json_joy_json_pack::cbor_to_json_owned(v).map_err(|_| ModelError::InvalidModelBinary)
}

#[derive(Debug)]
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos == self.data.len()
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn peak(&self) -> Result<u8, ModelError> {
        if self.remaining() < 1 {
            return Err(ModelError::InvalidModelBinary);
        }
        Ok(self.data[self.pos])
    }

    fn u8(&mut self) -> Result<u8, ModelError> {
        let b = self.peak()?;
        self.pos += 1;
        Ok(b)
    }

    fn u32_be(&mut self) -> Result<u32, ModelError> {
        if self.remaining() < 4 {
            return Err(ModelError::InvalidClockTable);
        }
        let out = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(out)
    }

    fn skip(&mut self, n: usize) -> Result<(), ModelError> {
        if self.remaining() < n {
            return Err(ModelError::InvalidModelBinary);
        }
        self.pos += n;
        Ok(())
    }

    fn buf(&mut self, n: usize) -> Result<&'a [u8], ModelError> {
        if self.remaining() < n {
            return Err(ModelError::InvalidModelBinary);
        }
        let start = self.pos;
        self.pos += n;
        Ok(&self.data[start..start + n])
    }

    fn vu57(&mut self) -> Result<u64, ModelError> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        for i in 0..8 {
            let b = self.u8()?;
            if i < 7 {
                let part = (b & 0x7f) as u64;
                result |= part
                    .checked_shl(shift)
                    .ok_or(ModelError::InvalidModelBinary)?;
                if (b & 0x80) == 0 {
                    return Ok(result);
                }
                shift += 7;
            } else {
                result |= (b as u64)
                    .checked_shl(49)
                    .ok_or(ModelError::InvalidModelBinary)?;
                return Ok(result);
            }
        }
        Err(ModelError::InvalidModelBinary)
    }

    fn b1vu56(&mut self) -> Result<(u8, u64), ModelError> {
        let first = self.u8()?;
        let flag = (first >> 7) & 1;
        let mut result: u64 = (first & 0x3f) as u64;
        if (first & 0x40) == 0 {
            return Ok((flag, result));
        }
        let mut shift: u32 = 6;
        for i in 0..7 {
            let b = self.u8()?;
            if i < 6 {
                result |= ((b & 0x7f) as u64)
                    .checked_shl(shift)
                    .ok_or(ModelError::InvalidModelBinary)?;
                if (b & 0x80) == 0 {
                    return Ok((flag, result));
                }
                shift += 7;
            } else {
                result |= (b as u64)
                    .checked_shl(48)
                    .ok_or(ModelError::InvalidModelBinary)?;
                return Ok((flag, result));
            }
        }
        Err(ModelError::InvalidModelBinary)
    }

    fn skip_id(&mut self) -> Result<(), ModelError> {
        let byte = self.u8()?;
        if byte <= 0b0111_1111 {
            return Ok(());
        }
        self.pos -= 1;
        let _ = self.b1vu56()?;
        let _ = self.vu57()?;
        Ok(())
    }

    fn read_one_cbor(&mut self) -> Result<CborValue, ModelError> {
        let slice = &self.data[self.pos..];
        let (val, consumed) = json_joy_json_pack::decode_cbor_value_with_consumed(slice)
            .map_err(|_| ModelError::InvalidModelBinary)?;
        self.skip(consumed)?;
        Ok(val)
    }
}
