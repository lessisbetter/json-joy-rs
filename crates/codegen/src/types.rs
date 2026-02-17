//! Type definitions for code generation.
//!
//! These types mirror the TypeScript `codegen` types for API compatibility,
//! but note that runtime code generation is not applicable in Rust.

/// Brand type for creating nominal types.
///
/// In TypeScript, this creates a branded type for type safety.
/// In Rust, we use newtypes instead.
pub trait Brand<T>: std::marker::Sized {
    /// The branded value.
    fn value(&self) -> &T;
}

/// Trait for types that can be used in generated code contexts.
///
/// This is a placeholder trait for API compatibility with the TypeScript
/// `codegen` package. It cannot be used for actual runtime code generation.
pub trait Codegenerable {}

impl Codegenerable for str {}
impl Codegenerable for String {}
impl Codegenerable for i64 {}
impl Codegenerable for i32 {}
impl Codegenerable for u64 {}
impl Codegenerable for u32 {}
impl Codegenerable for f64 {}
impl Codegenerable for bool {}
