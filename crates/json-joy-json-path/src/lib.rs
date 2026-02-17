//! JSONPath (RFC 9535) implementation.
//!
//! This crate provides parsing and evaluation of JSONPath expressions
//! as specified in [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html).
//!
//! # Example
//!
//! ```
//! use json_joy_json_path::{JsonPathParser, JsonPathEval};
//! use serde_json::json;
//!
//! // Parse a JSONPath expression
//! let path = JsonPathParser::parse("$.store.books[*].author").unwrap();
//!
//! // Evaluate against a JSON document
//! let doc = json!({
//!     "store": {
//!         "books": [
//!             {"author": "Nigel Rees", "title": "Sayings of the Century"},
//!             {"author": "Evelyn Waugh", "title": "Sword of Honour"}
//!         ]
//!     }
//! });
//!
//! let results = JsonPathEval::eval(&path, &doc);
//! assert_eq!(results.len(), 2);
//! ```

mod types;
pub use types::*;

mod ast;
pub use ast::Ast;

mod parser;
pub use parser::{JsonPathParser, ParseError};

mod eval;
pub use eval::JsonPathEval;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_root() {
        let path = JsonPathParser::parse("$").unwrap();
        assert_eq!(path.segments.len(), 0);
    }

    #[test]
    fn test_parse_dot_notation() {
        let path = JsonPathParser::parse("$.store.books").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_bracket_notation() {
        let path = JsonPathParser::parse("$['store']['books']").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_wildcard() {
        let path = JsonPathParser::parse("$.store.*").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_index() {
        let path = JsonPathParser::parse("$.books[0]").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_slice() {
        let path = JsonPathParser::parse("$.books[1:3]").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_eval_root() {
        let doc = json!({"a": 1});
        let path = JsonPathParser::parse("$").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &doc);
    }

    #[test]
    fn test_eval_dot_notation() {
        let doc = json!({"a": {"b": 42}});
        let path = JsonPathParser::parse("$.a.b").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &json!(42));
    }

    #[test]
    fn test_eval_wildcard() {
        let doc = json!({"a": 1, "b": 2});
        let path = JsonPathParser::parse("$.*").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_array_index() {
        let doc = json!([1, 2, 3, 4, 5]);
        let path = JsonPathParser::parse("$[2]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &json!(3));
    }

    #[test]
    fn test_eval_array_slice() {
        let doc = json!([1, 2, 3, 4, 5]);
        let path = JsonPathParser::parse("$[1:3]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], &json!(2));
        assert_eq!(results[1], &json!(3));
    }
}
