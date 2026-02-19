//! Substring occurrence count.
//!
//! Mirrors `packages/json-joy/src/util/strCnt.ts`.

/// Counts the number of non-overlapping occurrences of `needle` in `haystack`,
/// starting from byte offset `offset`.
pub fn str_cnt(needle: &str, haystack: &str, offset: usize) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut pos = offset;
    while let Some(found) = haystack[pos..].find(needle) {
        count += 1;
        pos += found + needle.len();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_occurrences() {
        assert_eq!(str_cnt("a", "banana", 0), 3);
        assert_eq!(str_cnt("na", "banana", 0), 2);
        assert_eq!(str_cnt("x", "banana", 0), 0);
        assert_eq!(str_cnt("a", "banana", 2), 2); // skip first 'a'
    }

    #[test]
    fn empty_needle_returns_zero() {
        assert_eq!(str_cnt("", "hello", 0), 0);
    }
}
