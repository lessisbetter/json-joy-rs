//! JSONPath parser (RFC 9535).

use crate::types::*;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("Expected root identifier '$' at start")]
    ExpectedRoot,
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unexpected end of input")]
    UnexpectedEnd,
    #[error("Invalid escape sequence")]
    InvalidEscape,
    #[error("Invalid number")]
    InvalidNumber,
    #[error("Unclosed string")]
    UnclosedString,
    #[error("Invalid selector")]
    InvalidSelector,
}

/// Helper struct returned by `peek_comparison_operator`.
struct ComparisonToken {
    operator: ComparisonOperator,
    len: usize,
}

/// JSONPath parser.
pub struct JsonPathParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonPathParser<'a> {
    /// Parse a JSONPath expression.
    pub fn parse(input: &'a str) -> Result<JSONPath, ParseError> {
        let mut parser = Self { input, pos: 0 };
        parser.parse_path()
    }

    fn parse_path(&mut self) -> Result<JSONPath, ParseError> {
        // Must start with $
        if self.peek() != Some('$') {
            return Err(ParseError::ExpectedRoot);
        }
        self.advance();

        let mut segments = Vec::new();

        while !self.is_at_end() {
            if self.peek() == Some('.') {
                self.advance();
                // Check for .. (recursive descent)
                if self.peek() == Some('.') {
                    self.advance();
                    // This is recursive descent, parse the selector
                    let selector = self.parse_recursive_selector()?;
                    segments.push(PathSegment::new(vec![selector], true));
                } else if self.peek() == Some('*') {
                    // Wildcard: .*
                    self.advance();
                    segments.push(PathSegment::new(vec![Selector::Wildcard], false));
                } else {
                    // Single dot notation: .name
                    let name = self.parse_identifier()?;
                    segments.push(PathSegment::new(vec![Selector::Name(name)], false));
                }
            } else if self.peek() == Some('[') {
                // Bracket notation
                let selectors = self.parse_bracket_selectors()?;
                segments.push(PathSegment::new(selectors, false));
            } else {
                break;
            }
        }

        Ok(JSONPath::new(segments))
    }

    fn parse_recursive_selector(&mut self) -> Result<Selector, ParseError> {
        // After .. we expect either:
        // - An identifier: ..name
        // - A bracket: ..[...]
        // - A wildcard: ..*

        if self.peek() == Some('*') {
            self.advance();
            return Ok(Selector::Wildcard);
        }

        if self.peek() == Some('[') {
            let selectors = self.parse_bracket_selectors()?;
            // For recursive descent, we typically only have one selector
            return Ok(selectors.into_iter().next().unwrap_or(Selector::Wildcard));
        }

        // Parse identifier
        let name = self.parse_identifier()?;
        Ok(Selector::Name(name))
    }

    fn parse_bracket_selectors(&mut self) -> Result<Vec<Selector>, ParseError> {
        self.expect('[')?;
        let mut selectors = Vec::new();

        loop {
            self.skip_whitespace();

            if self.peek() == Some(']') {
                self.advance();
                break;
            }

            let selector = self.parse_bracket_selector()?;
            selectors.push(selector);

            self.skip_whitespace();
            if self.peek() == Some(',') {
                self.advance();
            } else if self.peek() == Some(']') {
                self.advance();
                break;
            } else {
                return Err(ParseError::UnexpectedChar(self.peek().unwrap_or('\0')));
            }
        }

        Ok(selectors)
    }

    fn parse_bracket_selector(&mut self) -> Result<Selector, ParseError> {
        match self.peek() {
            Some('\'') | Some('"') => {
                // String literal (named selector)
                let name = self.parse_string()?;
                Ok(Selector::Name(name))
            }
            Some('*') => {
                self.advance();
                Ok(Selector::Wildcard)
            }
            Some(':') | Some('-') | Some('0'..='9') => {
                // Could be index or slice
                self.parse_index_or_slice()
            }
            Some('?') => {
                self.advance();
                self.skip_whitespace();
                self.expect('(')?;
                let expr = self.parse_filter_expression()?;
                self.expect(')')?;
                Ok(Selector::Filter(expr))
            }
            _ => Err(ParseError::InvalidSelector),
        }
    }

    fn parse_index_or_slice(&mut self) -> Result<Selector, ParseError> {
        let start = self.parse_optional_number()?;

        if self.peek() == Some(':') {
            self.advance();
            let end = self.parse_optional_number()?;

            let step = if self.peek() == Some(':') {
                self.advance();
                Some(self.parse_number()?)
            } else {
                None
            };

            Ok(Selector::Slice {
                start,
                end,
                step,
            })
        } else {
            Ok(Selector::Index(start.unwrap_or(0)))
        }
    }

    fn parse_optional_number(&mut self) -> Result<Option<isize>, ParseError> {
        self.skip_whitespace();
        if matches!(self.peek(), Some('0'..='9') | Some('-')) {
            Ok(Some(self.parse_number()?))
        } else {
            Ok(None)
        }
    }

    fn parse_number(&mut self) -> Result<isize, ParseError> {
        let start = self.pos;
        let negative = self.peek() == Some('-');
        if negative {
            self.advance();
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        let num_str = &self.input[start..self.pos];
        num_str.parse::<isize>().map_err(|_| ParseError::InvalidNumber)
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        let start = self.pos;

        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                self.advance();
            } else {
                break;
            }
        }

        if self.pos == start {
            return Err(ParseError::UnexpectedEnd);
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        let quote = self.peek().unwrap();
        self.advance();

        let mut result = String::new();

        loop {
            match self.peek() {
                None => return Err(ParseError::UnclosedString),
                Some(c) if c == quote => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('n') => result.push('\n'),
                        Some('t') => result.push('\t'),
                        Some('r') => result.push('\r'),
                        Some('\\') => result.push('\\'),
                        Some('\'') => result.push('\''),
                        Some('"') => result.push('"'),
                        _ => return Err(ParseError::InvalidEscape),
                    }
                    self.advance();
                }
                Some(c) => {
                    result.push(c);
                    self.advance();
                }
            }
        }

        Ok(result)
    }

    fn parse_filter_expression(&mut self) -> Result<FilterExpression, ParseError> {
        self.parse_logical_or_expression()
    }

    fn parse_logical_or_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_logical_and_expression()?;
        self.skip_whitespace();

        while self.peek_str("||") {
            self.advance();
            self.advance();
            self.skip_whitespace();
            let right = self.parse_logical_and_expression()?;
            left = FilterExpression::Logical {
                operator: LogicalOperator::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
            self.skip_whitespace();
        }

        Ok(left)
    }

    fn parse_logical_and_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_unary_expression()?;
        self.skip_whitespace();

        while self.peek_str("&&") {
            self.advance();
            self.advance();
            self.skip_whitespace();
            let right = self.parse_unary_expression()?;
            left = FilterExpression::Logical {
                operator: LogicalOperator::And,
                left: Box::new(left),
                right: Box::new(right),
            };
            self.skip_whitespace();
        }

        Ok(left)
    }

    fn parse_unary_expression(&mut self) -> Result<FilterExpression, ParseError> {
        self.skip_whitespace();

        if self.peek() == Some('!') {
            self.advance();
            self.skip_whitespace();
            let expr = self.parse_unary_expression()?;
            return Ok(FilterExpression::Negation(Box::new(expr)));
        }

        if self.peek() == Some('(') {
            self.advance();
            self.skip_whitespace();
            let expr = self.parse_filter_expression()?;
            self.skip_whitespace();
            self.expect(')')?;
            return Ok(FilterExpression::Paren(Box::new(expr)));
        }

        self.parse_primary_expression()
    }

    fn parse_primary_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let left = self.parse_value_expression()?;
        self.skip_whitespace();

        if let Some(op) = self.peek_comparison_operator() {
            self.advance_by(op.len);
            self.skip_whitespace();
            let right = self.parse_value_expression()?;
            return Ok(FilterExpression::Comparison {
                operator: op.operator,
                left,
                right,
            });
        }

        // No comparison operator — treat as existence test
        match &left {
            ValueExpression::Path(path) => {
                Ok(FilterExpression::Existence { path: path.clone() })
            }
            ValueExpression::Current => {
                Ok(FilterExpression::Existence { path: JSONPath::new(vec![]) })
            }
            ValueExpression::Function { name, args } => {
                Ok(FilterExpression::Function { name: name.clone(), args: args.clone() })
            }
            _ => Err(ParseError::InvalidSelector),
        }
    }

    fn parse_value_expression(&mut self) -> Result<ValueExpression, ParseError> {
        self.skip_whitespace();

        if self.peek() == Some('@') {
            self.advance();
            if self.peek() == Some('.') || self.peek() == Some('[') {
                let segments = self.parse_filter_path_segments()?;
                return Ok(ValueExpression::Path(JSONPath::new(segments)));
            }
            return Ok(ValueExpression::Current);
        }

        if self.peek() == Some('$') {
            self.advance();
            let segments = self.parse_filter_path_segments()?;
            return Ok(ValueExpression::Path(JSONPath::new(segments)));
        }

        if self.peek() == Some('\'') || self.peek() == Some('"') {
            let s = self.parse_string()?;
            return Ok(ValueExpression::Literal(serde_json::Value::String(s)));
        }

        if matches!(self.peek(), Some('0'..='9') | Some('-')) {
            let n = self.parse_float_number()?;
            return Ok(ValueExpression::Literal(
                serde_json::Number::from_f64(n)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
            ));
        }

        if self.peek_str("true") {
            self.advance_by(4);
            return Ok(ValueExpression::Literal(serde_json::Value::Bool(true)));
        }

        if self.peek_str("false") {
            self.advance_by(5);
            return Ok(ValueExpression::Literal(serde_json::Value::Bool(false)));
        }

        if self.peek_str("null") {
            self.advance_by(4);
            return Ok(ValueExpression::Literal(serde_json::Value::Null));
        }

        // Function call: starts with a lowercase letter
        if matches!(self.peek(), Some('a'..='z')) {
            let name = self.parse_function_name()?;
            self.skip_whitespace();
            if self.peek() == Some('(') {
                self.advance();
                self.skip_whitespace();
                let mut args = Vec::new();
                if self.peek() != Some(')') {
                    loop {
                        self.skip_whitespace();
                        let arg_val = self.parse_value_expression()?;
                        args.push(FunctionArg::Value(arg_val));
                        self.skip_whitespace();
                        if self.peek() == Some(',') {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.skip_whitespace();
                self.expect(')')?;
                return Ok(ValueExpression::Function { name, args });
            }
            return Err(ParseError::InvalidSelector);
        }

        Err(ParseError::InvalidSelector)
    }

    /// Parse path segments in a filter context.
    /// Stops when it encounters `)`, `,`, `&&`, `||`, a comparison operator, or `]`.
    fn parse_filter_path_segments(&mut self) -> Result<Vec<PathSegment>, ParseError> {
        let mut segments = Vec::new();

        loop {
            self.skip_whitespace();
            if self.is_filter_path_terminator() {
                break;
            }
            if self.peek() == Some('.') {
                self.advance();
                // Check for recursive descent (..)
                if self.peek() == Some('.') {
                    self.advance();
                    let selector = self.parse_recursive_selector()?;
                    segments.push(PathSegment::new(vec![selector], true));
                } else if self.peek() == Some('*') {
                    self.advance();
                    segments.push(PathSegment::new(vec![Selector::Wildcard], false));
                } else {
                    let name = self.parse_identifier()?;
                    segments.push(PathSegment::new(vec![Selector::Name(name)], false));
                }
            } else if self.peek() == Some('[') {
                let selectors = self.parse_bracket_selectors()?;
                segments.push(PathSegment::new(selectors, false));
            } else {
                break;
            }
        }

        Ok(segments)
    }

    fn is_filter_path_terminator(&self) -> bool {
        match self.peek() {
            None => true,
            Some(')') | Some(',') | Some(']') => true,
            Some('&') => self.peek_str("&&"),
            Some('|') => self.peek_str("||"),
            Some('=') => self.peek_str("=="),
            Some('!') => self.peek_str("!="),
            Some('<') | Some('>') => true,
            _ => false,
        }
    }

    fn peek_str(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn advance_by(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    fn parse_function_name(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        // First char must be lowercase letter
        match self.peek() {
            Some(c) if c.is_ascii_lowercase() => self.advance(),
            _ => return Err(ParseError::InvalidSelector),
        }
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_float_number(&mut self) -> Result<f64, ParseError> {
        let start = self.pos;

        // Optional minus
        if self.peek() == Some('-') {
            self.advance();
        }

        // Integer part
        if !matches!(self.peek(), Some('0'..='9')) {
            return Err(ParseError::InvalidNumber);
        }
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }

        // Optional decimal
        if self.peek() == Some('.') {
            self.advance();
            if !matches!(self.peek(), Some('0'..='9')) {
                return Err(ParseError::InvalidNumber);
            }
            while matches!(self.peek(), Some('0'..='9')) {
                self.advance();
            }
        }

        // Optional exponent
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.advance();
            }
            if !matches!(self.peek(), Some('0'..='9')) {
                return Err(ParseError::InvalidNumber);
            }
            while matches!(self.peek(), Some('0'..='9')) {
                self.advance();
            }
        }

        let num_str = &self.input[start..self.pos];
        num_str.parse::<f64>().map_err(|_| ParseError::InvalidNumber)
    }

    /// Peek at the next comparison operator without consuming input.
    /// Returns the operator token length and enum value, or None.
    fn peek_comparison_operator(&self) -> Option<ComparisonToken> {
        if self.peek_str("==") {
            Some(ComparisonToken { operator: ComparisonOperator::Equal, len: 2 })
        } else if self.peek_str("!=") {
            Some(ComparisonToken { operator: ComparisonOperator::NotEqual, len: 2 })
        } else if self.peek_str("<=") {
            Some(ComparisonToken { operator: ComparisonOperator::LessEqual, len: 2 })
        } else if self.peek_str(">=") {
            Some(ComparisonToken { operator: ComparisonOperator::GreaterEqual, len: 2 })
        } else if self.peek_str("<") {
            Some(ComparisonToken { operator: ComparisonOperator::Less, len: 1 })
        } else if self.peek_str(">") {
            Some(ComparisonToken { operator: ComparisonOperator::Greater, len: 1 })
        } else {
            None
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += self.peek().unwrap().len_utf8();
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn expect(&mut self, expected: char) -> Result<(), ParseError> {
        if self.peek() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedChar(self.peek().unwrap_or('\0')))
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }
}
