/// Options for word wrapping.
#[derive(Debug, Clone)]
pub struct WrapOptions {
    /// Maximum width of each line. Default is 50.
    pub width: usize,
}

impl Default for WrapOptions {
    fn default() -> Self {
        Self { width: 50 }
    }
}

/// Wrap text to a specified width.
///
/// Splits the input string into lines, respecting word boundaries where possible.
///
/// # Examples
///
/// ```
/// use json_joy_util::strings::{word_wrap, WrapOptions};
///
/// let text = "This is a long line of text that should be wrapped.";
/// let lines = word_wrap(text, Some(WrapOptions { width: 20 }));
///
/// assert!(!lines.is_empty());
/// for line in &lines {
///     assert!(line.len() <= 20 || !line.contains(' '));
/// }
/// ```
pub fn word_wrap(s: &str, options: Option<WrapOptions>) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }

    let opts = options.unwrap_or_default();
    let width = opts.width;

    let mut lines = Vec::new();

    // Split by existing newlines first
    for paragraph in s.split('\n') {
        let paragraph = paragraph.trim_end();
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        // Split paragraph into words
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current_line));
                current_line.push_str(word);
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_wrap_empty() {
        let lines = word_wrap("", None);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_word_wrap_short_line() {
        let lines = word_wrap("hello world", Some(WrapOptions { width: 50 }));
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn test_word_wrap_long_line() {
        let text = "This is a long line of text that should be wrapped.";
        let lines = word_wrap(text, Some(WrapOptions { width: 20 }));

        assert!(!lines.is_empty());
        // Each line should be at most 20 chars (unless a single word is longer)
        for line in &lines {
            assert!(line.len() <= 20 || !line.contains(' '));
        }
    }

    #[test]
    fn test_word_wrap_with_newlines() {
        let text = "Line one\nLine two\nLine three";
        let lines = word_wrap(text, Some(WrapOptions { width: 50 }));

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Line one");
        assert_eq!(lines[1], "Line two");
        assert_eq!(lines[2], "Line three");
    }

    #[test]
    fn test_word_wrap_default_options() {
        let lines = word_wrap("hello", None);
        assert_eq!(lines, vec!["hello"]);
    }
}
