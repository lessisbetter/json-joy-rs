//! Utility functions for the JSON CRDT model.
//!
//! Mirrors `json-crdt/model/util.ts`.

/// Generates a random session ID up to 53 bits in size.
///
/// Skips the first `0xFFFF` (65535) values, keeping them reserved for
/// future extensions.  The session ID is bounded by `SESSION.MAX = 2^53 - 1`.
///
/// Mirrors `randomSessionId` in `util.ts`.
///
/// # Implementation note
///
/// The upstream uses `Math.random()` (a 53-bit float).  This implementation
/// derives entropy from the system clock with additional mixing, matching
/// the required range without pulling in an external `rand` crate.
pub fn random_session_id() -> u64 {
    const SESSION_MAX: u64 = 9007199254740991; // 2^53 - 1
    const RESERVED: u64 = 0xFFFF; // 65535
    const DIFF: u64 = SESSION_MAX - RESERVED;

    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Mix seconds and sub-second nanos for entropy.
    let seed = (d.as_secs() << 30) ^ (d.subsec_nanos() as u64);
    let mixed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    // Reduce to the [0, DIFF) range and shift into [RESERVED, SESSION_MAX].
    (mixed % DIFF) + RESERVED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_session_id_in_range() {
        const SESSION_MAX: u64 = 9007199254740991;
        const RESERVED: u64 = 0xFFFF;
        for _ in 0..10 {
            let id = random_session_id();
            assert!(id >= RESERVED, "id {id} should be >= {RESERVED}");
            assert!(id <= SESSION_MAX, "id {id} should be <= {SESSION_MAX}");
        }
    }

    #[test]
    fn random_session_id_never_zero() {
        for _ in 0..20 {
            assert!(random_session_id() > 0);
        }
    }
}
