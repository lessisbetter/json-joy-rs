//! Code generation utilities for json-joy.
//!
//! **Note:** This crate is a stub. The upstream TypeScript `codegen` package
//! provides runtime JavaScript code generation using `eval()`, which is not
//! applicable in Rust.
//!
//! ## Why This Is a Stub
//!
//! The TypeScript `codegen` package:
//! - Builds JavaScript code strings at runtime
//! - Uses `eval()` to compile and execute the generated code
//! - Enables dynamic optimization by generating specialized functions
//!
//! In Rust, runtime code generation requires different approaches:
//! - **Procedural macros** for compile-time code generation
//! - **`dyn Trait`** for runtime polymorphism
//! - **Generic specialization** for optimized code paths
//!
//! Downstream packages that use `codegen` (e.g., `json-expression`, `json-path`)
//! will need to implement alternative strategies appropriate for Rust.
//!
//! ## Available Types
//!
//! This stub provides type definitions for interface compatibility, but they
//! are not functional for runtime code generation.

mod types;

pub use types::*;

/// Marker type for generated code.
///
/// In TypeScript, this represents a string containing JavaScript code that can
/// be evaluated. In Rust, this serves as a placeholder for API compatibility.
///
/// This type cannot be used for actual code generation in Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaScript(pub String);

impl JavaScript {
    /// Creates a new JavaScript marker.
    ///
    /// **Note:** This does not compile or execute any code in Rust.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// Returns the code string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for JavaScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Marker type for generated closures with dependencies.
///
/// In TypeScript, this represents a JavaScript closure with linked dependencies.
/// In Rust, this serves as a placeholder for API compatibility.
#[derive(Debug, Clone)]
pub struct JavaScriptLinked<T> {
    /// The generated JavaScript code (not executable in Rust).
    pub js: JavaScript,
    /// The dependencies that would be linked (placeholder).
    pub deps: Vec<T>,
}

/// Error type for codegen operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Runtime code generation is not supported in Rust.
    NotSupported,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::NotSupported => write!(
                f,
                "Runtime JavaScript code generation is not supported in Rust. \
                 Consider using procedural macros or generic specialization instead."
            ),
        }
    }
}

impl std::error::Error for CodegenError {}
