use rand::Rng;

/// Token grammar for template-driven random string generation.
///
/// Rust divergence: upstream models tokens as tuple unions in TypeScript;
/// here we use an enum for type safety.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Literal(String),
    Pick(Vec<Token>),
    Repeat {
        min: usize,
        max: usize,
        pattern: Box<Token>,
    },
    Char {
        min: u32,
        max: u32,
        count: usize,
    },
    List(Vec<Token>),
}

impl Token {
    pub fn literal<S: Into<String>>(value: S) -> Self {
        Self::Literal(value.into())
    }

    pub fn pick(from: Vec<Token>) -> Self {
        Self::Pick(from)
    }

    pub fn repeat(min: usize, max: usize, pattern: Token) -> Self {
        Self::Repeat {
            min,
            max,
            pattern: Box::new(pattern),
        }
    }

    pub fn char_range(min: u32, max: u32, count: Option<usize>) -> Self {
        Self::Char {
            min,
            max,
            count: count.unwrap_or(1),
        }
    }

    pub fn list(every: Vec<Token>) -> Self {
        Self::List(every)
    }
}

/// Mirrors upstream `randomString(token)`.
pub fn random_string(token: &Token) -> String {
    let mut rng = rand::thread_rng();
    match token {
        Token::Literal(s) => s.clone(),
        Token::Pick(from) => {
            if from.is_empty() {
                return String::new();
            }
            let idx = rng.gen_range(0..from.len());
            random_string(&from[idx])
        }
        Token::Repeat { min, max, pattern } => {
            let (lo, hi) = if min <= max {
                (*min, *max)
            } else {
                (*max, *min)
            };
            let count = rng.gen_range(lo..=hi);
            let mut out = String::new();
            for _ in 0..count {
                out.push_str(&random_string(pattern));
            }
            out
        }
        Token::Char { min, max, count } => {
            let (lo, hi) = if min <= max {
                (*min, *max)
            } else {
                (*max, *min)
            };
            let mut out = String::new();
            for _ in 0..*count {
                let code_point = rng.gen_range(lo..=hi);
                match char::from_u32(code_point) {
                    Some(c) => out.push(c),
                    None => out.push('\u{FFFD}'),
                }
            }
            out
        }
        Token::List(every) => {
            let mut out = String::new();
            for t in every {
                out.push_str(&random_string(t));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_string_pick() {
        let token = Token::pick(vec![
            Token::literal("apple"),
            Token::literal("banana"),
            Token::literal("cherry"),
        ]);
        let result = random_string(&token);
        assert!(["apple", "banana", "cherry"].contains(&result.as_str()));
    }

    #[test]
    fn random_string_repeat() {
        let token = Token::repeat(2, 5, Token::literal("x"));
        let result = random_string(&token);
        assert!((2..=5).contains(&result.len()));
    }

    #[test]
    fn random_string_char_range() {
        let token = Token::char_range(65, 90, Some(3));
        let result = random_string(&token);
        assert_eq!(result.chars().count(), 3);
        assert!(result.chars().all(|c| c.is_ascii_uppercase()));
    }
}
