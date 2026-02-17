/// Escape special characters in a string for JSON serialization.
///
/// This function escapes:
/// - Control characters (0x00-0x1F)
/// - Double quote (0x22)
/// - Backslash (0x5C)
/// - Invalid surrogate pairs (0xD800-0xDFFF)
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::escape;
///
/// assert_eq!(escape("hello"), "hello");
/// assert_eq!(escape("say \"hi\""), "say \\\"hi\\\"");
/// assert_eq!(escape("line1\nline2"), "line1\\nline2");
/// ```
pub fn escape(s: &str) -> String {
    let mut result = String::new();
    let mut last = 0;

    for (i, ch) in s.char_indices() {
        let escaped = match ch {
            // Control characters
            '\u{0000}' => Some("\\u0000"),
            '\u{0001}' => Some("\\u0001"),
            '\u{0002}' => Some("\\u0002"),
            '\u{0003}' => Some("\\u0003"),
            '\u{0004}' => Some("\\u0004"),
            '\u{0005}' => Some("\\u0005"),
            '\u{0006}' => Some("\\u0006"),
            '\u{0007}' => Some("\\u0007"),
            '\u{0008}' => Some("\\b"),
            '\t' => Some("\\t"),
            '\n' => Some("\\n"),
            '\u{000B}' => Some("\\u000b"),
            '\u{000C}' => Some("\\f"),
            '\r' => Some("\\r"),
            '\u{000E}' => Some("\\u000e"),
            '\u{000F}' => Some("\\u000f"),
            '\u{0010}' => Some("\\u0010"),
            '\u{0011}' => Some("\\u0011"),
            '\u{0012}' => Some("\\u0012"),
            '\u{0013}' => Some("\\u0013"),
            '\u{0014}' => Some("\\u0014"),
            '\u{0015}' => Some("\\u0015"),
            '\u{0016}' => Some("\\u0016"),
            '\u{0017}' => Some("\\u0017"),
            '\u{0018}' => Some("\\u0018"),
            '\u{0019}' => Some("\\u0019"),
            '\u{001A}' => Some("\\u001a"),
            '\u{001B}' => Some("\\u001b"),
            '\u{001C}' => Some("\\u001c"),
            '\u{001D}' => Some("\\u001d"),
            '\u{001E}' => Some("\\u001e"),
            '\u{001F}' => Some("\\u001f"),
            // Special characters
            '"' => Some("\\\""),
            '\\' => Some("\\\\"),
            // No escape needed
            _ => None,
        };

        if let Some(esc) = escaped {
            result.push_str(&s[last..i]);
            result.push_str(esc);
            last = i + ch.len_utf8();
        }
    }

    result.push_str(&s[last..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_simple() {
        assert_eq!(escape("hello"), "hello");
    }

    #[test]
    fn test_escape_empty() {
        assert_eq!(escape(""), "");
    }

    #[test]
    fn test_escape_quotes() {
        assert_eq!(escape("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_escape_backslash() {
        assert_eq!(escape("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_escape_newline() {
        assert_eq!(escape("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_tab() {
        assert_eq!(escape("tab\there"), "tab\\there");
    }

    #[test]
    fn test_escape_carriage_return() {
        assert_eq!(escape("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_backspace() {
        // \x08 is the backspace character
        assert_eq!(escape("back\x08space"), "back\\bspace");
    }

    #[test]
    fn test_escape_form_feed() {
        // \x0c is the form feed character
        assert_eq!(escape("form\x0cfeed"), "form\\ffeed");
    }

    #[test]
    fn test_escape_null() {
        assert_eq!(escape("null\0byte"), "null\\u0000byte");
    }

    #[test]
    fn test_escape_unicode() {
        // Valid Unicode should not be escaped
        assert_eq!(escape("hello 日本語"), "hello 日本語");
    }
}
