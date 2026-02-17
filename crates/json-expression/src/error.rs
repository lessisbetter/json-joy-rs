use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum JsError {
    #[error("{0}")]
    ArityError(String),

    #[error("Unknown expression: {0}")]
    UnknownExpression(String),

    #[error("NOT_FOUND")]
    NotFound,

    #[error("DIVISION_BY_ZERO")]
    DivisionByZero,

    #[error("NOT_CONTAINER")]
    NotContainer,

    #[error("NOT_STRING_INDEX")]
    NotStringIndex,

    #[error("NOT_BINARY")]
    NotBinary,

    #[error("NOT_STRING")]
    NotString,

    #[error("NOT_ARRAY")]
    NotArray,

    #[error("NOT_OBJECT")]
    NotObject,

    #[error("OUT_OF_BOUNDS")]
    OutOfBounds,

    #[error("NOT_PAIR")]
    NotPair,

    #[error("INVALID_INDEX")]
    InvalidIndex,

    #[error("PROTO_KEY")]
    ProtoKey,

    #[error("Invalid literal.")]
    InvalidLiteral,

    #[error("Invalid varname.")]
    InvalidVarname,

    #[error("varname must be a string.")]
    VarnameMustBeString,

    #[error("{0}")]
    Thrown(String),

    #[error("{0}")]
    Other(String),
}
