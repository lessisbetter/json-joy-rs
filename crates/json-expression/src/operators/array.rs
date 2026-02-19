//! Array operators — mirrors upstream `operators/array.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
fn concat_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let arrays: Result<Vec<JsValue>, JsError> =
        expr[1..].iter().map(|e| crate::evaluate(e, ctx)).collect();
    util::concat_arrays(&arrays?)
}

fn push_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let mut arr = util::as_arr(&operand1)?.clone();
    for e in &expr[2..] {
        let val = crate::evaluate(e, ctx)?;
        arr.push(util::jsvalue_to_json(val));
    }
    Ok(JsValue::Json(Value::Array(arr)))
}

fn head_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let operand2 = crate::evaluate(&expr[2], ctx)?;
    util::head(&operand1, &operand2)
}

fn sort_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let mut arr = util::as_arr(&operand1)?.clone();
    // JS default sort is lexicographic string comparison
    arr.sort_by(|a, b| {
        let sa = util::str_val(&JsValue::Json(a.clone()));
        let sb = util::str_val(&JsValue::Json(b.clone()));
        sa.cmp(&sb)
    });
    Ok(JsValue::Json(Value::Array(arr)))
}

fn reverse_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let mut arr = util::as_arr(&operand1)?.clone();
    arr.reverse();
    Ok(JsValue::Json(Value::Array(arr)))
}

fn in_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let arr = crate::evaluate(&expr[1], ctx)?;
    let val = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_in_arr(&arr, &val)?)))
}

fn from_entries_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    util::from_entries(&operand1)
}

fn index_of_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let container = crate::evaluate(&expr[1], ctx)?;
    let item = crate::evaluate(&expr[2], ctx)?;
    util::index_of(&container, &item)
}

fn normalize_slice_index(idx: i32, len: i32) -> usize {
    if idx < 0 {
        (len + idx).max(0) as usize
    } else {
        idx.min(len) as usize
    }
}

fn slice_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let operand2 = crate::evaluate(&expr[2], ctx)?;
    let operand3 = crate::evaluate(&expr[3], ctx)?;
    let arr = util::as_arr(&operand1)?.clone();
    let len = arr.len() as i32;
    let start = normalize_slice_index(util::int(&operand2), len);
    let end = normalize_slice_index(util::int(&operand3), len);
    Ok(JsValue::Json(Value::Array(arr[start..end].to_vec())))
}

fn zip_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let operand2 = crate::evaluate(&expr[2], ctx)?;
    util::zip(&operand1, &operand2)
}

fn get_literal_str(val: &Value) -> Result<String, JsError> {
    match util::as_literal(val)? {
        Value::String(s) => Ok(s.clone()),
        _ => Err(JsError::NotString),
    }
}

fn filter_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let arr = util::as_arr(&operand1)?.clone();
    let varname = get_literal_str(&expr[2])?;
    let sub_expr = expr[3].clone();
    let operators = Arc::clone(&ctx.operators);
    let create_pattern = ctx.create_pattern.clone();
    util::filter_arr(&arr, &varname, ctx.vars, &mut |vars| {
        let mut inner_ctx = EvalCtx {
            vars,
            operators: Arc::clone(&operators),
            create_pattern: create_pattern.clone(),
        };
        crate::evaluate(&sub_expr, &mut inner_ctx)
    })
}

fn map_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let arr = util::as_arr(&operand1)?.clone();
    let varname = get_literal_str(&expr[2])?;
    let sub_expr = expr[3].clone();
    let operators = Arc::clone(&ctx.operators);
    let create_pattern = ctx.create_pattern.clone();
    util::map_arr(&arr, &varname, ctx.vars, &mut |vars| {
        let mut inner_ctx = EvalCtx {
            vars,
            operators: Arc::clone(&operators),
            create_pattern: create_pattern.clone(),
        };
        crate::evaluate(&sub_expr, &mut inner_ctx)
    })
}

fn reduce_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let operand1 = crate::evaluate(&expr[1], ctx)?;
    let arr = util::as_arr(&operand1)?.clone();
    let initial_value = crate::evaluate(&expr[2], ctx)?;
    let accname = get_literal_str(&expr[3])?;
    let varname = get_literal_str(&expr[4])?;
    let sub_expr = expr[5].clone();
    let operators = Arc::clone(&ctx.operators);
    let create_pattern = ctx.create_pattern.clone();
    util::reduce_arr(
        &arr,
        initial_value,
        &accname,
        &varname,
        ctx.vars,
        &mut |vars| {
            let mut inner_ctx = EvalCtx {
                vars,
                operators: Arc::clone(&operators),
                create_pattern: create_pattern.clone(),
            };
            crate::evaluate(&sub_expr, &mut inner_ctx)
        },
    )
}

use std::sync::Arc;

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition {
            name: "concat",
            aliases: &["++"],
            arity: Arity::Variadic,
            eval_fn: concat_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "push",
            aliases: &[],
            arity: Arity::Variadic,
            eval_fn: push_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "head",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: head_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "sort",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: sort_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "reverse",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: reverse_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "in",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: in_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "fromEntries",
            aliases: &[],
            arity: Arity::Fixed(1),
            eval_fn: from_entries_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "indexOf",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: index_of_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "slice",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: slice_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "zip",
            aliases: &[],
            arity: Arity::Fixed(2),
            eval_fn: zip_eval,
            impure: false,
        }),
        Arc::new(OperatorDefinition {
            name: "filter",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: filter_eval,
            impure: true,
        }),
        Arc::new(OperatorDefinition {
            name: "map",
            aliases: &[],
            arity: Arity::Fixed(3),
            eval_fn: map_eval,
            impure: true,
        }),
        Arc::new(OperatorDefinition {
            name: "reduce",
            aliases: &[],
            arity: Arity::Fixed(5),
            eval_fn: reduce_eval,
            impure: true,
        }),
    ]
}
