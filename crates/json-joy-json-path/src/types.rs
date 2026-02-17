//! JSONPath types and interfaces based on RFC 9535.

use serde_json::Value;

/// Selector types for JSONPath.
#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    /// Named selector for property access: `.name`, `['key']`
    Name(String),
    /// Index selector for array element access: `[0]`, `[-1]`
    Index(isize),
    /// Slice selector for array slicing: `[start:end:step]`
    Slice { start: Option<isize>, end: Option<isize>, step: Option<isize> },
    /// Wildcard selector for selecting all elements: `.*`, `[*]`
    Wildcard,
    /// Filter expression for conditional selection: `[?(@.price < 10)]`
    Filter(FilterExpression),
}

/// Path segment containing one or more selectors.
#[derive(Debug, Clone, PartialEq)]
pub struct PathSegment {
    /// Selectors in this segment.
    pub selectors: Vec<Selector>,
    /// Whether this is a recursive descent segment (`..`).
    pub recursive: bool,
}

impl PathSegment {
    pub fn new(selectors: Vec<Selector>, recursive: bool) -> Self {
        Self { selectors, recursive }
    }
}

/// Complete JSONPath expression.
#[derive(Debug, Clone, PartialEq)]
pub struct JSONPath {
    /// Path segments.
    pub segments: Vec<PathSegment>,
}

impl JSONPath {
    pub fn new(segments: Vec<PathSegment>) -> Self {
        Self { segments }
    }
}

/// Filter expression types.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpression {
    /// Comparison expression: `@.price < 10`
    Comparison {
        operator: ComparisonOperator,
        left: ValueExpression,
        right: ValueExpression,
    },
    /// Logical expression: `@.a && @.b`
    Logical {
        operator: LogicalOperator,
        left: Box<FilterExpression>,
        right: Box<FilterExpression>,
    },
    /// Existence test: `@.name`
    Existence { path: JSONPath },
    /// Function call: `length(@)`
    Function { name: String, args: Vec<FunctionArg> },
    /// Parenthesized expression: `(@.a || @.b)`
    Paren(Box<FilterExpression>),
    /// Negation: `!@.flag`
    Negation(Box<FilterExpression>),
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,       // ==
    NotEqual,    // !=
    Less,        // <
    LessEqual,   // <=
    Greater,     // >
    GreaterEqual, // >=
}

/// Logical operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    And, // &&
    Or,  // ||
}

/// Value expressions in filters.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueExpression {
    /// Current node: `@`
    Current,
    /// Root node: `$`
    Root,
    /// Literal value: `"string"`, `42`, `true`, `null`
    Literal(Value),
    /// Path expression: `@.name`
    Path(JSONPath),
    /// Function call: `length(@)`
    Function { name: String, args: Vec<FunctionArg> },
}

/// Function argument types.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionArg {
    Value(ValueExpression),
    Filter(FilterExpression),
    Path(JSONPath),
}

/// Result of JSONPath query evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult<'a> {
    /// The matched values.
    pub values: Vec<&'a Value>,
    /// Normalized paths to the matched values.
    pub paths: Vec<Vec<PathComponent>>,
}

/// A component of a normalized path.
#[derive(Debug, Clone, PartialEq)]
pub enum PathComponent {
    Key(String),
    Index(usize),
}
