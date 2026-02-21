//! Shallow reader generator for MessagePack paths.
//!
//! Upstream reference: `json-pack/src/msgpack/shallow-read.ts`
//!
//! Upstream compiles specialized JS for each path. In Rust we return a closure
//! over the captured path and reuse `MsgPackDecoder::find_path`.

use super::{MsgPackDecoder, MsgPackError, MsgPackPathSegment};

/// Path reader closure returned by [`gen_shallow_reader`].
pub type ShallowReader<'a> = Box<dyn Fn(&mut MsgPackDecoder) -> Result<usize, MsgPackError> + 'a>;

/// Builds a reusable shallow reader for a fixed MessagePack path.
///
/// The returned closure walks the path and returns the decoder offset of the
/// selected value, matching upstream `genShallowReader` observable behavior.
pub fn gen_shallow_reader<'a>(path: &'a [MsgPackPathSegment<'a>]) -> ShallowReader<'a> {
    let path = path.to_vec();
    Box::new(move |decoder: &mut MsgPackDecoder| {
        decoder.find_path(&path)?;
        Ok(decoder.inner.x)
    })
}
