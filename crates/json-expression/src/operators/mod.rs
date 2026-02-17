//! Operator definitions — mirrors upstream `operators/index.ts`.

pub mod arithmetic;
pub mod array;
pub mod binary;
pub mod bitwise;
pub mod branching;
pub mod comparison;
pub mod container;
pub mod input;
pub mod logical;
pub mod object;
pub mod patch;
pub mod string;
pub mod type_ops;

use crate::types::{OperatorDefinition, OperatorMap, operators_to_map};
use std::sync::Arc;

/// All operators combined — mirrors upstream `operators` array.
pub fn all_operators() -> Vec<Arc<OperatorDefinition>> {
    let mut ops = Vec::new();
    ops.extend(arithmetic::operators());
    ops.extend(comparison::operators());
    ops.extend(logical::operators());
    ops.extend(type_ops::operators());
    ops.extend(container::operators());
    ops.extend(string::operators());
    ops.extend(binary::operators());
    ops.extend(array::operators());
    ops.extend(object::operators());
    ops.extend(branching::operators());
    ops.extend(input::operators());
    ops.extend(bitwise::operators());
    ops.extend(patch::operators());
    ops
}

/// Build the operator map from all operators.
///
/// Mirrors upstream `operatorsMap`.
pub fn operators_map() -> OperatorMap {
    operators_to_map(all_operators())
}
