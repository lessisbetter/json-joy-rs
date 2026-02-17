use std::cmp::Ordering;

/// Compare two strings for object key ordering.
///
/// The comparison is first by length, then by lexicographic order.
/// This is useful for deterministic key ordering when serializing objects.
///
/// Returns:
/// - `Ordering::Less` if `a` should come before `b`
/// - `Ordering::Greater` if `a` should come after `b`
/// - `Ordering::Equal` if both strings are equal
///
/// # Examples
///
/// ```
/// use std::cmp::Ordering;
/// use json_joy_util::obj_key_cmp::obj_key_cmp;
///
/// assert_eq!(obj_key_cmp("a", "b"), Ordering::Less);
/// assert_eq!(obj_key_cmp("aa", "b"), Ordering::Greater); // "aa" is longer
/// assert_eq!(obj_key_cmp("a", "a"), Ordering::Equal);
/// ```
pub fn obj_key_cmp(a: &str, b: &str) -> Ordering {
    let len1 = a.len();
    let len2 = b.len();

    if len1 == len2 {
        a.cmp(b)
    } else {
        len1.cmp(&len2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_length() {
        // Same length: compare lexicographically
        assert_eq!(obj_key_cmp("a", "b"), Ordering::Less);
        assert_eq!(obj_key_cmp("b", "a"), Ordering::Greater);
        assert_eq!(obj_key_cmp("a", "a"), Ordering::Equal);
        assert_eq!(obj_key_cmp("abc", "abd"), Ordering::Less);
    }

    #[test]
    fn test_different_lengths() {
        // Different lengths: shorter comes first
        assert_eq!(obj_key_cmp("a", "aa"), Ordering::Less);
        assert_eq!(obj_key_cmp("aa", "a"), Ordering::Greater);
        assert_eq!(obj_key_cmp("", "a"), Ordering::Less);
        assert_eq!(obj_key_cmp("a", ""), Ordering::Greater);
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(obj_key_cmp("", ""), Ordering::Equal);
        assert_eq!(obj_key_cmp("a", "b"), Ordering::Less);
        assert_eq!(obj_key_cmp("z", "a"), Ordering::Greater);
    }
}
