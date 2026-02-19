use rand::Rng;

/// Mirrors upstream `int(min, max)`.
pub fn int(min: i64, max: i64) -> i64 {
    if min == max {
        return min;
    }
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    rand::thread_rng().gen_range(lo..=hi)
}

/// Mirrors upstream `int64(min, max)`.
///
/// Rust divergence: upstream returns JavaScript `bigint`, while this crate
/// uses `i64` as the native 64-bit integer type.
pub fn int64(min: i64, max: i64) -> i64 {
    int(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_respects_bounds() {
        for _ in 0..100 {
            let n = int(-10, 10);
            assert!((-10..=10).contains(&n));
        }
    }

    #[test]
    fn int64_respects_bounds() {
        for _ in 0..100 {
            let n = int64(-100, 100);
            assert!((-100..=100).contains(&n));
        }
    }
}
