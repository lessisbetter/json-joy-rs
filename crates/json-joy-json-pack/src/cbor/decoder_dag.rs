//! `CborDecoderDag` — DAG-JSON CBOR decoder.
//!
//! Direct port of `cbor/CborDecoderDag.ts` from upstream.
//! Differs from `CborDecoder`: tag 42 is decoded as `JsonPackExtension`;
//! all other tags are "unwrapped" (just the value is returned).

use super::decoder_base::{CborDecoderBase, Cur};
use super::error::CborError;
use crate::{JsonPackExtension, PackValue};

/// DAG-JSON CBOR decoder.
///
/// Only tag 42 (CID) is kept as a `JsonPackExtension`. All other tags are unwrapped.
#[derive(Default)]
pub struct CborDecoderDag;

impl CborDecoderDag {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, input: &[u8]) -> Result<PackValue, CborError> {
        let base = CborDecoderBase::new();
        let mut c = Cur { data: input, pos: 0 };
        self.read_any(&base, &mut c)
    }

    fn read_any(&self, base: &CborDecoderBase, c: &mut Cur) -> Result<PackValue, CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & super::constants::MINOR_MASK;
        if major == super::constants::MAJOR_TAG {
            let tag = base.read_uint(c, minor)?;
            return self.read_tag_raw(base, c, tag);
        }
        base.read_any_raw(c, octet)
    }

    fn read_tag_raw(&self, base: &CborDecoderBase, c: &mut Cur, tag: u64) -> Result<PackValue, CborError> {
        let val = self.read_any(base, c)?;
        if tag == 42 {
            Ok(PackValue::Extension(Box::new(JsonPackExtension::new(tag, val))))
        } else {
            Ok(val) // unwrap non-42 tags
        }
    }
}
