use serde_json::Value;

/// Deterministic pseudo-random number generator equivalent to upstream `rnd(seed)`.
pub fn rnd(seed: i64) -> impl FnMut() -> f64 {
    let mut seed = if seed == 0 { 1 } else { seed };
    move || {
        seed = (seed * 48_271) % 2_147_483_647;
        (seed - 1) as f64 / 2_147_483_646_f64
    }
}

/// Executes a callback deterministically.
///
/// Rust divergence: upstream temporarily monkey-patches `Math.random()` for
/// all code in the callback. Rust does not provide a global hook for `rand`
/// crate consumers, so this helper currently executes the callback directly.
pub fn deterministic<T, F>(_rnd_seed: i64, code: F) -> T
where
    F: FnOnce() -> T,
{
    code()
}

/// Deep clone for JSON values.
pub fn clone_json(value: &Value) -> Value {
    value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rnd_generates_unit_interval() {
        let mut next = rnd(123456789);
        for _ in 0..10 {
            let n = next();
            assert!((0.0..1.0).contains(&n));
        }
    }

    #[test]
    fn clone_json_deep_clones() {
        let input = serde_json::json!({"a": [1, 2, {"b": true}]});
        let clone = clone_json(&input);
        assert_eq!(clone, input);
    }
}
