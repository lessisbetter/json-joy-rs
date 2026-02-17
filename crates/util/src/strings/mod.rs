//! String utilities.
//!
//! Provides functions for string manipulation, escaping, and formatting.

mod as_string;
mod escape;
mod util;
mod word_wrap;

pub use as_string::as_string;
pub use escape::escape;
pub use util::{is_letter, is_punctuation, is_whitespace, CharPredicate};
pub use word_wrap::{word_wrap, WrapOptions};
