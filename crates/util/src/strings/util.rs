/// A predicate function for checking character properties.
pub type CharPredicate = fn(char) -> bool;

/// Check if a character is a letter or digit.
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::is_letter;
///
/// assert!(is_letter('a'));
/// assert!(is_letter('Z'));
/// assert!(is_letter('5'));
/// assert!(!is_letter(' '));
/// assert!(!is_letter('!'));
/// ```
pub fn is_letter(ch: char) -> bool {
    ch.is_alphanumeric()
}

/// Check if a character is whitespace.
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::is_whitespace;
///
/// assert!(is_whitespace(' '));
/// assert!(is_whitespace('\t'));
/// assert!(is_whitespace('\n'));
/// assert!(!is_whitespace('a'));
/// ```
pub fn is_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}

/// Check if a character is punctuation (not a letter or whitespace).
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::is_punctuation;
///
/// assert!(is_punctuation('!'));
/// assert!(is_punctuation('.'));
/// assert!(is_punctuation(','));
/// assert!(!is_punctuation('a'));
/// assert!(!is_punctuation(' '));
/// ```
pub fn is_punctuation(ch: char) -> bool {
    !is_letter(ch) && !is_whitespace(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_letter_ascii() {
        assert!(is_letter('a'));
        assert!(is_letter('Z'));
        assert!(is_letter('0'));
        assert!(is_letter('9'));
    }

    #[test]
    fn test_is_letter_unicode() {
        assert!(is_letter('日'));
        assert!(is_letter('α'));
    }

    #[test]
    fn test_is_letter_not() {
        assert!(!is_letter(' '));
        assert!(!is_letter('!'));
        assert!(!is_letter('\t'));
    }

    #[test]
    fn test_is_whitespace() {
        assert!(is_whitespace(' '));
        assert!(is_whitespace('\t'));
        assert!(is_whitespace('\n'));
        assert!(is_whitespace('\r'));
    }

    #[test]
    fn test_is_whitespace_not() {
        assert!(!is_whitespace('a'));
        assert!(!is_whitespace('1'));
        assert!(!is_whitespace('!'));
    }

    #[test]
    fn test_is_punctuation() {
        assert!(is_punctuation('!'));
        assert!(is_punctuation('.'));
        assert!(is_punctuation(','));
        assert!(is_punctuation('?'));
        assert!(is_punctuation(':'));
    }

    #[test]
    fn test_is_punctuation_not() {
        assert!(!is_punctuation('a'));
        assert!(!is_punctuation('1'));
        assert!(!is_punctuation(' '));
    }
}
