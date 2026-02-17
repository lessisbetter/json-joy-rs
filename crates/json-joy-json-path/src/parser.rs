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
        // Simplified filter expression parsing
        // For now, just handle existence checks like @.name
        self.skip_whitespace();

        if self.peek() != Some('@') {
            return Err(ParseError::InvalidSelector);
        }
        self.advance();

        // Parse the path from current node
        let mut segments = Vec::new();
        while self.peek() == Some('.') || self.peek() == Some('[') {
            if self.peek() == Some('.') {
                self.advance();
                let name = self.parse_identifier()?;
                segments.push(PathSegment::new(vec![Selector::Name(name)], false));
            } else if self.peek() == Some('[') {
                let selectors = self.parse_bracket_selectors()?;
                segments.push(PathSegment::new(selectors, false));
            }
        }

        // For now, just return an existence expression
        Ok(FilterExpression::Existence {
            path: JSONPath::new(segments),
        })
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
