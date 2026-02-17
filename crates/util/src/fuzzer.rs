use rand::{rngs::OsRng, Rng, RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use std::sync::{Arc, Mutex};

/// A fuzzer for generating random test data.
///
/// Uses the xoshiro256** PRNG for reproducible random sequences when seeded.
///
/// # Examples
///
/// ```
/// use json_joy_util::fuzzer::Fuzzer;
///
/// // Create a fuzzer with a random seed
/// let fuzzer = Fuzzer::new(None);
///
/// // Generate random integers
/// let n = fuzzer.random_int(1, 10);
/// assert!(n >= 1 && n <= 10);
///
/// // Pick a random element from a slice
/// let choices = vec!["a", "b", "c"];
/// let picked = fuzzer.pick(&choices);
/// assert!(choices.contains(&picked));
/// ```
pub struct Fuzzer {
    /// The seed used to initialize the PRNG.
    pub seed: [u8; 32],
    rng: Arc<Mutex<Xoshiro256StarStar>>,
}

impl Fuzzer {
    /// Create a new fuzzer with an optional seed.
    ///
    /// If no seed is provided, a random seed will be generated using `OsRng`.
    pub fn new(seed: Option<[u8; 32]>) -> Self {
        let seed = seed.unwrap_or_else(|| {
            let mut bytes = [0u8; 32];
            OsRng.fill_bytes(&mut bytes);
            bytes
        });

        let rng = Xoshiro256StarStar::from_seed(seed);

        Self {
            seed,
            rng: Arc::new(Mutex::new(rng)),
        }
    }

    /// Generate a random integer in the range [min, max] (inclusive).
    pub fn random_int(&self, min: i64, max: i64) -> i64 {
        let mut rng = self.rng.lock().unwrap();
        rng.gen_range(min..=max)
    }

    /// Generate a random integer in the range specified by a tuple [min, max] (inclusive).
    pub fn random_int_range(&self, range: (i64, i64)) -> i64 {
        self.random_int(range.0, range.1)
    }

    /// Pick a random element from a slice.
    pub fn pick<'a, T>(&self, elements: &'a [T]) -> &'a T {
        let mut rng = self.rng.lock().unwrap();
        let idx = rng.gen_range(0..elements.len());
        &elements[idx]
    }

    /// Repeat a callback `times` times and collect results.
    pub fn repeat<T, F>(&self, times: usize, mut callback: F) -> Vec<T>
    where
        F: FnMut() -> T,
    {
        (0..times).map(|_| callback()).collect()
    }

    /// Generate a random f64 in the range [0, 1).
    pub fn random(&self) -> f64 {
        let mut rng = self.rng.lock().unwrap();
        rng.gen::<f64>()
    }

    /// Generate a random byte array of the specified length.
    pub fn random_bytes(&self, len: usize) -> Vec<u8> {
        let mut rng = self.rng.lock().unwrap();
        let mut bytes = vec![0u8; len];
        rng.fill_bytes(&mut bytes);
        bytes
    }

    /// Generate a random boolean with the given probability of being true.
    pub fn random_bool(&self, probability: f64) -> bool {
        let mut rng = self.rng.lock().unwrap();
        rng.gen_bool(probability)
    }

    /// Generate a random string of the specified length from the given characters.
    pub fn random_string(&self, len: usize, chars: &str) -> String {
        let chars: Vec<char> = chars.chars().collect();
        let mut rng = self.rng.lock().unwrap();
        (0..len)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}

/// Static helper methods for random generation (uses thread-local RNG).
pub struct Random;

impl Random {
    /// Generate a random integer in the range [min, max] (inclusive).
    pub fn random_int(min: i64, max: i64) -> i64 {
        rand::thread_rng().gen_range(min..=max)
    }

    /// Pick a random element from a slice.
    pub fn pick<'a, T>(elements: &'a [T]) -> &'a T {
        let idx = rand::thread_rng().gen_range(0..elements.len());
        &elements[idx]
    }

    /// Repeat a callback `times` times and collect results.
    pub fn repeat<T, F>(times: usize, mut callback: F) -> Vec<T>
    where
        F: FnMut() -> T,
    {
        (0..times).map(|_| callback()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzer_random_int() {
        let fuzzer = Fuzzer::new(None);

        // Test range
        for _ in 0..100 {
            let n = fuzzer.random_int(1, 10);
            assert!(n >= 1 && n <= 10);
        }
    }

    #[test]
    fn test_fuzzer_random_int_range() {
        let fuzzer = Fuzzer::new(None);

        for _ in 0..100 {
            let n = fuzzer.random_int_range((5, 15));
            assert!(n >= 5 && n <= 15);
        }
    }

    #[test]
    fn test_fuzzer_pick() {
        let fuzzer = Fuzzer::new(None);
        let choices = vec!["a", "b", "c"];

        for _ in 0..100 {
            let picked = fuzzer.pick(&choices);
            assert!(choices.contains(picked));
        }
    }

    #[test]
    fn test_fuzzer_repeat() {
        let fuzzer = Fuzzer::new(None);

        let results: Vec<i32> = fuzzer.repeat(5, || 42);
        assert_eq!(results, vec![42, 42, 42, 42, 42]);
    }

    #[test]
    fn test_fuzzer_reproducible() {
        let seed = [1u8; 32];

        let fuzzer1 = Fuzzer::new(Some(seed));
        let fuzzer2 = Fuzzer::new(Some(seed));

        // Same seed should produce same sequence
        for _ in 0..10 {
            assert_eq!(fuzzer1.random_int(0, 1000), fuzzer2.random_int(0, 1000));
        }
    }

    #[test]
    fn test_fuzzer_random() {
        let fuzzer = Fuzzer::new(None);

        for _ in 0..100 {
            let r = fuzzer.random();
            assert!(r >= 0.0 && r < 1.0);
        }
    }

    #[test]
    fn test_fuzzer_random_bytes() {
        let fuzzer = Fuzzer::new(None);

        let bytes = fuzzer.random_bytes(16);
        assert_eq!(bytes.len(), 16);
    }

    #[test]
    fn test_fuzzer_random_bool() {
        let fuzzer = Fuzzer::new(None);

        // Test that both values can be produced
        let mut has_true = false;
        let mut has_false = false;

        for _ in 0..100 {
            if fuzzer.random_bool(0.5) {
                has_true = true;
            } else {
                has_false = true;
            }
        }

        assert!(has_true && has_false);
    }

    #[test]
    fn test_fuzzer_random_string() {
        let fuzzer = Fuzzer::new(None);

        let s = fuzzer.random_string(10, "abc");
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| "abc".contains(c)));
    }

    #[test]
    fn test_random_static() {
        // Test static methods
        let n = Random::random_int(1, 10);
        assert!(n >= 1 && n <= 10);

        let choices = vec![1, 2, 3];
        let picked = Random::pick(&choices);
        assert!(choices.contains(picked));

        let results: Vec<i32> = Random::repeat(3, || 42);
        assert_eq!(results, vec![42, 42, 42]);
    }
}
