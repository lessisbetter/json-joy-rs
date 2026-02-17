//! Codegen result types — mirrors upstream `codegen-steps.ts`.
//!
//! In the upstream TypeScript, these are used to build JavaScript code strings.
//! In Rust we use them only as metadata during expression compilation.

/// A compile-time constant expression result — value is known at "compile time".
///
/// Mirrors upstream `Literal` class from `codegen-steps.ts`.
#[derive(Debug, Clone)]
pub struct Literal {
    pub val: serde_json::Value,
}

impl Literal {
    pub fn new(val: serde_json::Value) -> Self {
        Literal { val }
    }
}

/// A runtime expression result — value must be computed at runtime.
///
/// In the upstream TypeScript this holds a JavaScript code string.
/// In Rust we use it as a marker that the expression is dynamic.
///
/// Mirrors upstream `Expression` class from `codegen-steps.ts`.
#[derive(Debug, Clone)]
pub struct DynamicExpr {
    /// A human-readable description of the dynamic expression (debug only).
    pub desc: String,
}

impl DynamicExpr {
    pub fn new(desc: impl Into<String>) -> Self {
        DynamicExpr { desc: desc.into() }
    }
}

/// Result of evaluating an expression during codegen.
///
/// Mirrors upstream `ExpressionResult = Literal | Expression`.
#[derive(Debug, Clone)]
pub enum ExpressionResult {
    Literal(Literal),
    Dynamic(DynamicExpr),
}

impl ExpressionResult {
    pub fn is_literal(&self) -> bool {
        matches!(self, ExpressionResult::Literal(_))
    }

    pub fn literal_val(&self) -> Option<&serde_json::Value> {
        match self {
            ExpressionResult::Literal(l) => Some(&l.val),
            _ => None,
        }
    }
}
