/// Serialize text as a JSON string value.
///
/// This function wraps a string in double quotes and escapes special characters
/// as needed for JSON serialization.
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::as_string;
///
/// assert_eq!(as_string("hello"), "\"hello\"");
/// assert_eq!(as_string("say \"hi\""), "\"say \\\"hi\\\"\"");
/// assert_eq!(as_string("back\\slash"), "\"back\\\\slash\"");
/// ```
pub fn as_string(s: &str) -> String {
    // String serialization cannot fail - serde_json always successfully serializes strings
    serde_json::to_string(s).expect("string serialization is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_string_simple() {
        assert_eq!(as_string("hello"), "\"hello\"");
    }

    #[test]
    fn test_as_string_empty() {
        assert_eq!(as_string(""), "\"\"");
    }

    #[test]
    fn test_as_string_with_quotes() {
        assert_eq!(as_string("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn test_as_string_with_backslash() {
        assert_eq!(as_string("back\\slash"), "\"back\\\\slash\"");
    }

    #[test]
    fn test_as_string_with_newline() {
        assert_eq!(as_string("line1\nline2"), "\"line1\\nline2\"");
    }

    #[test]
    fn test_as_string_with_tab() {
        assert_eq!(as_string("tab\there"), "\"tab\\there\"");
    }

    #[test]
    fn test_as_string_with_unicode() {
        assert_eq!(as_string("hello 日本語"), "\"hello 日本語\"");
    }
}
