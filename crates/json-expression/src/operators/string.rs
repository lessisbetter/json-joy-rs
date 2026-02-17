//! String operators — mirrors upstream `operators/string.ts`.

use crate::error::JsError;
use crate::eval_ctx::EvalCtx;
use crate::types::{Arity, JsValue, OperatorDefinition};
use crate::util;
use serde_json::Value;
use std::sync::Arc;

fn cat_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let mut result = String::new();
    for e in &expr[1..] {
        let val = crate::evaluate(e, ctx)?;
        result.push_str(&util::str_val(&val));
    }
    Ok(JsValue::Json(Value::String(result)))
}

fn contains_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let outer = crate::evaluate(&expr[1], ctx)?;
    let inner = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::contains(&outer, &inner))))
}

fn starts_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let outer = crate::evaluate(&expr[1], ctx)?;
    let inner = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::starts(&outer, &inner))))
}

fn ends_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let outer = crate::evaluate(&expr[1], ctx)?;
    let inner = crate::evaluate(&expr[2], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::ends(&outer, &inner))))
}

fn substr_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let s = crate::evaluate(&expr[1], ctx)?;
    let from = crate::evaluate(&expr[2], ctx)?;
    let to = crate::evaluate(&expr[3], ctx)?;
    Ok(util::substr(&s, &from, &to))
}

fn matches_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    // pattern must be a literal string
    let pattern_val = &expr[2];
    let pattern = match util::as_literal(pattern_val) {
        Ok(Value::String(s)) => s.clone(),
        _ => {
            return Err(JsError::Other(
                "\"matches\" second argument should be a regular expression string.".to_string(),
            ))
        }
    };
    let create_pattern = ctx.create_pattern.as_ref().ok_or_else(|| {
        JsError::Other(
            "\"matches\" operator requires \".createPattern()\" option to be implemented.".to_string(),
        )
    })?;
    let matcher = create_pattern(&pattern);
    let outer = crate::evaluate(&expr[1], ctx)?;
    let subject = util::str_val(&outer);
    Ok(JsValue::Json(Value::Bool(matcher(&subject))))
}

fn email_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_email(&val))))
}

fn hostname_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_hostname(&val))))
}

fn ip4_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_ip4(&val))))
}

fn ip6_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_ip6(&val))))
}

fn uuid_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_uuid(&val))))
}

fn uri_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_uri(&val))))
}

fn duration_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_duration(&val))))
}

fn date_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_date(&val))))
}

fn time_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_time(&val))))
}

fn datetime_eval(expr: &[Value], ctx: &mut EvalCtx<'_>) -> Result<JsValue, JsError> {
    let val = crate::evaluate(&expr[1], ctx)?;
    Ok(JsValue::Json(Value::Bool(util::is_datetime(&val))))
}

pub fn operators() -> Vec<Arc<OperatorDefinition>> {
    vec![
        Arc::new(OperatorDefinition { name: ".", aliases: &["cat"], arity: Arity::Variadic, eval_fn: cat_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "contains", aliases: &[], arity: Arity::Fixed(2), eval_fn: contains_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "starts", aliases: &[], arity: Arity::Fixed(2), eval_fn: starts_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "ends", aliases: &[], arity: Arity::Fixed(2), eval_fn: ends_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "substr", aliases: &[], arity: Arity::Fixed(3), eval_fn: substr_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "matches", aliases: &[], arity: Arity::Fixed(2), eval_fn: matches_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "email?", aliases: &[], arity: Arity::Fixed(1), eval_fn: email_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "hostname?", aliases: &[], arity: Arity::Fixed(1), eval_fn: hostname_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "ip4?", aliases: &[], arity: Arity::Fixed(1), eval_fn: ip4_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "ip6?", aliases: &[], arity: Arity::Fixed(1), eval_fn: ip6_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "uuid?", aliases: &[], arity: Arity::Fixed(1), eval_fn: uuid_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "uri?", aliases: &[], arity: Arity::Fixed(1), eval_fn: uri_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "duration?", aliases: &[], arity: Arity::Fixed(1), eval_fn: duration_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "date?", aliases: &[], arity: Arity::Fixed(1), eval_fn: date_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "time?", aliases: &[], arity: Arity::Fixed(1), eval_fn: time_eval, impure: false }),
        Arc::new(OperatorDefinition { name: "dateTime?", aliases: &[], arity: Arity::Fixed(1), eval_fn: datetime_eval, impure: false }),
    ]
}
