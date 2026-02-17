//! AST construction helpers.

use crate::types::*;

/// Helper functions for constructing JSONPath AST nodes.
pub struct Ast;

impl Ast {
    /// Create a JSONPath from segments.
    pub fn path(segments: Vec<PathSegment>) -> JSONPath {
        JSONPath::new(segments)
    }

    /// Create a path segment from selectors.
    pub fn segment(selectors: Vec<Selector>, recursive: bool) -> PathSegment {
        PathSegment::new(selectors, recursive)
    }

    /// Create a named selector.
    pub fn name(name: impl Into<String>) -> Selector {
        Selector::Name(name.into())
    }

    /// Create an index selector.
    pub fn index(index: isize) -> Selector {
        Selector::Index(index)
    }

    /// Create a slice selector.
    pub fn slice(start: Option<isize>, end: Option<isize>, step: Option<isize>) -> Selector {
        Selector::Slice { start, end, step }
    }

    /// Create a wildcard selector.
    pub fn wildcard() -> Selector {
        Selector::Wildcard
    }

    /// Create a filter selector.
    pub fn filter(expr: FilterExpression) -> Selector {
        Selector::Filter(expr)
    }

    /// Create a comparison expression.
    pub fn comparison(
        operator: ComparisonOperator,
        left: ValueExpression,
        right: ValueExpression,
    ) -> FilterExpression {
        FilterExpression::Comparison { operator, left, right }
    }

    /// Create a logical expression.
    pub fn logical(
        operator: LogicalOperator,
        left: FilterExpression,
        right: FilterExpression,
    ) -> FilterExpression {
        FilterExpression::Logical {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create a current node expression.
    pub fn current() -> ValueExpression {
        ValueExpression::Current
    }

    /// Create a root node expression.
    pub fn root() -> ValueExpression {
        ValueExpression::Root
    }

    /// Create a literal expression.
    pub fn literal(value: serde_json::Value) -> ValueExpression {
        ValueExpression::Literal(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_construction() {
        let path = Ast::path(vec![
            Ast::segment(vec![Ast::name("store")], false),
            Ast::segment(vec![Ast::name("books")], false),
            Ast::segment(vec![Ast::wildcard()], false),
        ]);

        assert_eq!(path.segments.len(), 3);
        assert!(!path.segments[0].recursive);
    }
}
