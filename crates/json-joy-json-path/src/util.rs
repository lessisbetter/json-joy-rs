//! JSONPath utility helpers.

use crate::types::{JSONPath, PathSegment, Selector};

/// Convert JSONPath AST to string representation.
pub fn json_path_to_string(path: &JSONPath) -> String {
    let mut out = String::from("$");
    for segment in &path.segments {
        out.push_str(&segment_to_string(segment));
    }
    out
}

/// Compare two JSONPath ASTs for structural equality.
pub fn json_path_equals(path1: &JSONPath, path2: &JSONPath) -> bool {
    path1 == path2
}

/// Return property names explicitly referenced by name selectors.
pub fn get_accessed_properties(path: &JSONPath) -> Vec<String> {
    let mut properties = Vec::new();

    for segment in &path.segments {
        for selector in &segment.selectors {
            if let Selector::Name(name) = selector {
                properties.push(name.clone());
            }
        }
    }

    properties
}

fn segment_to_string(segment: &PathSegment) -> String {
    if segment.selectors.len() == 1 {
        let selector = &segment.selectors[0];
        if segment.recursive {
            format!("..{}", selector_to_segment_tail(selector))
        } else {
            selector_to_segment(selector)
        }
    } else {
        let joined = segment
            .selectors
            .iter()
            .map(selector_to_segment)
            .collect::<Vec<_>>()
            .join(",");
        if segment.recursive {
            format!("..[{}]", joined)
        } else {
            format!("[{}]", joined)
        }
    }
}

fn selector_to_segment(selector: &Selector) -> String {
    match selector {
        Selector::Name(name) => {
            if is_identifier(name) {
                format!(".{}", name)
            } else {
                format!("['{}']", escape_single_quoted(name))
            }
        }
        Selector::Index(index) => format!("[{}]", index),
        Selector::Slice { start, end, step } => {
            let mut s = String::from("[");
            if let Some(v) = start {
                s.push_str(&v.to_string());
            }
            s.push(':');
            if let Some(v) = end {
                s.push_str(&v.to_string());
            }
            if let Some(v) = step {
                s.push(':');
                s.push_str(&v.to_string());
            }
            s.push(']');
            s
        }
        Selector::Wildcard => String::from(".*"),
        Selector::Filter(_) => String::from("[?(...)]"),
    }
}

fn selector_to_segment_tail(selector: &Selector) -> String {
    match selector {
        Selector::Name(name) => {
            if is_identifier(name) {
                name.clone()
            } else {
                format!("['{}']", escape_single_quoted(name))
            }
        }
        Selector::Index(index) => format!("[{}]", index),
        Selector::Slice { .. } => selector_to_segment(selector),
        Selector::Wildcard => String::from("*"),
        Selector::Filter(_) => String::from("[?(...)]"),
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn escape_single_quoted(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ComparisonOperator, FilterExpression, LogicalOperator, PathSegment, Selector,
        ValueExpression,
    };

    #[test]
    fn stringify_basic_and_recursive_paths() {
        let path = JSONPath::new(vec![
            PathSegment::new(vec![Selector::Name("store".into())], false),
            PathSegment::new(vec![Selector::Wildcard], false),
            PathSegment::new(vec![Selector::Name("title".into())], true),
        ]);
        assert_eq!(json_path_to_string(&path), "$.store.*..title");
    }

    #[test]
    fn stringify_filter_and_union() {
        let filter = Selector::Filter(FilterExpression::Logical {
            operator: LogicalOperator::And,
            left: Box::new(FilterExpression::Comparison {
                operator: ComparisonOperator::Greater,
                left: ValueExpression::Current,
                right: ValueExpression::Literal(serde_json::json!(1)),
            }),
            right: Box::new(FilterExpression::Existence {
                path: JSONPath::new(vec![]),
            }),
        });

        let path = JSONPath::new(vec![PathSegment::new(
            vec![Selector::Name("a".into()), Selector::Index(1), filter],
            false,
        )]);

        assert_eq!(json_path_to_string(&path), "$[.a,[1],[?(...)]]");
    }

    #[test]
    fn equality_and_accessed_properties() {
        let path1 = JSONPath::new(vec![
            PathSegment::new(vec![Selector::Name("a".into())], false),
            PathSegment::new(vec![Selector::Name("b".into())], true),
        ]);
        let path2 = path1.clone();
        assert!(json_path_equals(&path1, &path2));

        let props = get_accessed_properties(&path1);
        assert_eq!(props, vec!["a", "b"]);
    }
}
