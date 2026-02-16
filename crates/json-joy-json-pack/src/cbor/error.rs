use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CborError {
    #[error("invalid cbor payload")]
    InvalidPayload,
    #[error("unsupported cbor feature for json conversion")]
    Unsupported,
}
