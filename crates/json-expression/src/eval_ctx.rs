use crate::types::OperatorMap;
use crate::vars::Vars;
use std::sync::Arc;

/// The execution context passed to every operator eval function.
///
/// Mirrors TypeScript's `OperatorEvalCtx` / `JsonExpressionExecutionContext`.
pub struct EvalCtx<'a> {
    /// The variable store (env + named vars).
    pub vars: &'a mut Vars,
    /// The operator map used for recursive evaluation.
    pub operators: Arc<OperatorMap>,
    /// Optional pattern factory for the `matches` operator.
    pub create_pattern: Option<Arc<dyn Fn(&str) -> Box<dyn Fn(&str) -> bool> + Send + Sync>>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(vars: &'a mut Vars, operators: Arc<OperatorMap>) -> Self {
        EvalCtx {
            vars,
            operators,
            create_pattern: None,
        }
    }

    pub fn with_pattern(
        mut self,
        create_pattern: Arc<dyn Fn(&str) -> Box<dyn Fn(&str) -> bool> + Send + Sync>,
    ) -> Self {
        self.create_pattern = Some(create_pattern);
        self
    }
}
