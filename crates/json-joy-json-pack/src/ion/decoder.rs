//! Ion binary decoder wrapper.
//!
//! Upstream reference: `json-pack/src/ion/IonDecoder.ts`

use super::constants::Type;
use super::decoder_base::IonDecoderBase;
use crate::PackValue;

pub use super::decoder_base::IonDecodeError;

/// Ion binary decoder.
pub struct IonDecoder {
    base: IonDecoderBase,
}

impl Default for IonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl IonDecoder {
    pub fn new() -> Self {
        Self {
            base: IonDecoderBase::new(),
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<PackValue, IonDecodeError> {
        self.base.reset(data);
        self.base.validate_bvm()?;
        self.read_symbol_table()?;
        self.base.val()
    }

    pub fn read(&mut self) -> Result<PackValue, IonDecodeError> {
        self.base.val()
    }

    fn read_symbol_table(&mut self) -> Result<(), IonDecodeError> {
        if !self.base.has_remaining() {
            return Ok(());
        }

        if self.base.peek_type_id()? != Type::ANNO {
            return Ok(());
        }

        let annotated = self.base.val()?;
        let PackValue::Object(fields) = annotated else {
            return Ok(());
        };

        let mut new_symbols = Vec::new();
        for (key, value) in fields {
            if key != "symbols" {
                continue;
            }
            if let PackValue::Array(values) = value {
                for item in values {
                    if let PackValue::Str(text) = item {
                        new_symbols.push(text);
                    }
                }
            }
            break;
        }

        for symbol in new_symbols {
            self.base.symbols_mut().add(&symbol);
        }

        Ok(())
    }
}
