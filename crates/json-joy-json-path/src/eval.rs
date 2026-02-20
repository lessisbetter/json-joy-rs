//! JSONPath evaluator.

use crate::types::*;
use serde_json::Value;

/// JSONPath evaluator.
pub struct JsonPathEval;

impl JsonPathEval {
    /// Evaluate a JSONPath against a JSON document.
    ///
    /// Returns a vector of references to matching values.
    pub fn eval<'a>(path: &JSONPath, doc: &'a Value) -> Vec<&'a Value> {
        let mut results = vec![doc];
        let mut paths: Vec<Vec<PathComponent>> = vec![vec![]];

        for segment in &path.segments {
            let mut new_results = Vec::new();
            let mut new_paths = Vec::new();

            for (i, value) in results.iter().enumerate() {
                let current_path = &paths[i];

                if segment.recursive {
                    // Recursive descent
                    Self::eval_recursive(
                        value,
                        &segment.selectors,
                        current_path,
                        &mut new_results,
                        &mut new_paths,
                    );
                } else {
                    Self::eval_segment(
                        value,
                        segment,
                        current_path,
                        &mut new_results,
                        &mut new_paths,
                    );
                }
            }

            results = new_results;
            paths = new_paths;
        }

        results
    }

    fn eval_segment<'a>(
        value: &'a Value,
        segment: &PathSegment,
        current_path: &[PathComponent],
        results: &mut Vec<&'a Value>,
        paths: &mut Vec<Vec<PathComponent>>,
    ) {
        for selector in &segment.selectors {
            Self::eval_selector(value, selector, current_path, results, paths);
        }
    }

    fn eval_recursive<'a>(
        value: &'a Value,
        selectors: &[Selector],
        current_path: &[PathComponent],
        results: &mut Vec<&'a Value>,
        paths: &mut Vec<Vec<PathComponent>>,
    ) {
        // First, try to match at current level
        for selector in selectors {
            Self::eval_selector(value, selector, current_path, results, paths);
        }

        // Then recurse into children
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let mut new_path = current_path.to_vec();
                    new_path.push(PathComponent::Key(key.clone()));
                    Self::eval_recursive(child, selectors, &new_path, results, paths);
                }
            }
            Value::Array(arr) => {
                for (idx, child) in arr.iter().enumerate() {
                    let mut new_path = current_path.to_vec();
                    new_path.push(PathComponent::Index(idx));
                    Self::eval_recursive(child, selectors, &new_path, results, paths);
                }
            }
            _ => {}
        }
    }

    fn eval_selector<'a>(
        value: &'a Value,
        selector: &Selector,
        current_path: &[PathComponent],
        results: &mut Vec<&'a Value>,
        paths: &mut Vec<Vec<PathComponent>>,
    ) {
        match selector {
            Selector::Name(name) => {
                if let Value::Object(map) = value {
                    if let Some(child) = map.get(name) {
                        let mut new_path = current_path.to_vec();
                        new_path.push(PathComponent::Key(name.clone()));
                        results.push(child);
                        paths.push(new_path);
                    }
                }
            }
            Selector::Index(index) => {
                if let Value::Array(arr) = value {
                    let idx = if *index < 0 {
                        (arr.len() as isize + index) as usize
                    } else {
                        *index as usize
                    };
                    if let Some(child) = arr.get(idx) {
                        let mut new_path = current_path.to_vec();
                        new_path.push(PathComponent::Index(idx));
                        results.push(child);
                        paths.push(new_path);
                    }
                }
            }
            Selector::Wildcard => match value {
                Value::Object(map) => {
                    for (key, child) in map {
                        let mut new_path = current_path.to_vec();
                        new_path.push(PathComponent::Key(key.clone()));
                        results.push(child);
                        paths.push(new_path);
                    }
                }
                Value::Array(arr) => {
                    for (idx, child) in arr.iter().enumerate() {
                        let mut new_path = current_path.to_vec();
                        new_path.push(PathComponent::Index(idx));
                        results.push(child);
                        paths.push(new_path);
                    }
                }
                _ => {}
            },
            Selector::Slice { start, end, step } => {
                if let Value::Array(arr) = value {
                    let len = arr.len();
                    let start_idx = Self::normalize_index(*start, len).unwrap_or(0);
                    let end_idx = Self::normalize_index(*end, len).unwrap_or(len);
                    let step_val = step.unwrap_or(1);

                    if step_val > 0 {
                        let mut i = start_idx;
                        while i < end_idx && i < len {
                            if let Some(child) = arr.get(i) {
                                let mut new_path = current_path.to_vec();
                                new_path.push(PathComponent::Index(i));
                                results.push(child);
                                paths.push(new_path);
                            }
                            i = (i as isize + step_val) as usize;
                        }
                    }
                }
            }
            Selector::Filter(expr) => {
                // Evaluate filter expression against all children
                match value {
                    Value::Object(map) => {
                        for (key, child) in map {
                            if Self::eval_filter(expr, child, value) {
                                let mut new_path = current_path.to_vec();
                                new_path.push(PathComponent::Key(key.clone()));
                                results.push(child);
                                paths.push(new_path);
                            }
                        }
                    }
                    Value::Array(arr) => {
                        for (idx, child) in arr.iter().enumerate() {
                            if Self::eval_filter(expr, child, value) {
                                let mut new_path = current_path.to_vec();
                                new_path.push(PathComponent::Index(idx));
                                results.push(child);
                                paths.push(new_path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn normalize_index(index: Option<isize>, len: usize) -> Option<usize> {
        index.map(|i| {
            if i < 0 {
                ((len as isize) + i).max(0) as usize
            } else {
                i as usize
            }
        })
    }

    fn eval_filter(expr: &FilterExpression, current: &Value, _parent: &Value) -> bool {
        match expr {
            FilterExpression::Existence { path } => {
                // Check if path exists from current node
                let results = Self::eval(path, current);
                !results.is_empty()
            }
            FilterExpression::Comparison {
                operator,
                left,
                right,
            } => {
                let left_val = Self::eval_value_expr(left, current);
                let right_val = Self::eval_value_expr(right, current);
                Self::compare(operator, &left_val, &right_val)
            }
            FilterExpression::Logical {
                operator,
                left,
                right,
            } => {
                let left_result = Self::eval_filter(left, current, _parent);
                let right_result = Self::eval_filter(right, current, _parent);
                match operator {
                    LogicalOperator::And => left_result && right_result,
                    LogicalOperator::Or => left_result || right_result,
                }
            }
            FilterExpression::Negation(expr) => !Self::eval_filter(expr, current, _parent),
            FilterExpression::Paren(expr) => Self::eval_filter(expr, current, _parent),
            FilterExpression::Function { .. } => {
                // Function evaluation not yet implemented
                false
            }
        }
    }

    fn eval_value_expr(expr: &ValueExpression, current: &Value) -> Option<Value> {
        match expr {
            ValueExpression::Current => Some(current.clone()),
            ValueExpression::Root => None, // Root not available in this context
            ValueExpression::Literal(v) => Some(v.clone()),
            ValueExpression::Path(path) => {
                let results = Self::eval(path, current);
                results.first().map(|v| (*v).clone())
            }
            ValueExpression::Function { .. } => None, // Not yet implemented
        }
    }

    fn compare(operator: &ComparisonOperator, left: &Option<Value>, right: &Option<Value>) -> bool {
        match (left, right) {
            (None, None) => match operator {
                ComparisonOperator::Equal => true,
                ComparisonOperator::NotEqual => false,
                _ => false,
            },
            (Some(l), Some(r)) => {
                // Use compare_values for all operators. This handles the case where
                // a numeric literal parsed as f64 (e.g. 1.0) must compare equal to an
                // integer JSON value (e.g. json!(1)), since serde_json's PartialEq
                // distinguishes integer and float representations.
                let ord = Self::compare_values(l, r);
                match operator {
                    ComparisonOperator::Equal => {
                        // For numbers use ordering-based equality; for other types use PartialEq
                        if let (Value::Number(_), Value::Number(_)) = (l, r) {
                            ord == Some(std::cmp::Ordering::Equal)
                        } else {
                            l == r
                        }
                    }
                    ComparisonOperator::NotEqual => {
                        if let (Value::Number(_), Value::Number(_)) = (l, r) {
                            ord != Some(std::cmp::Ordering::Equal)
                        } else {
                            l != r
                        }
                    }
                    ComparisonOperator::Less => ord == Some(std::cmp::Ordering::Less),
                    ComparisonOperator::LessEqual => matches!(
                        ord,
                        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                    ),
                    ComparisonOperator::Greater => ord == Some(std::cmp::Ordering::Greater),
                    ComparisonOperator::GreaterEqual => matches!(
                        ord,
                        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                    ),
                }
            }
            _ => false,
        }
    }

    fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                if let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) {
                    a.partial_cmp(&b)
                } else {
                    None
                }
            }
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}
