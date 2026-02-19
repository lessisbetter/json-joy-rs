use crate::string::Token;

use super::types::Template;

/// Mirrors upstream `nil` template.
pub fn nil() -> Template {
    Template::nil()
}

/// Mirrors upstream `tokensHelloWorld`.
pub fn tokens_hello_world() -> Token {
    Token::list(vec![
        Token::pick(vec![
            Token::literal("hello"),
            Token::literal("Hello"),
            Token::literal("Halo"),
            Token::literal("Hi"),
            Token::literal("Hey"),
            Token::literal("Greetings"),
            Token::literal("Salutations"),
        ]),
        Token::pick(vec![Token::literal(""), Token::literal(",")]),
        Token::literal(" "),
        Token::pick(vec![
            Token::literal("world"),
            Token::literal("World"),
            Token::literal("Earth"),
            Token::literal("Globe"),
            Token::literal("Planet"),
        ]),
        Token::pick(vec![Token::literal(""), Token::literal("!")]),
    ])
}

/// Mirrors upstream `tokensObjectKey`.
pub fn tokens_object_key() -> Token {
    Token::pick(vec![
        Token::pick(vec![
            Token::literal("id"),
            Token::literal("name"),
            Token::literal("type"),
            Token::literal("tags"),
            Token::literal("_id"),
            Token::literal(".git"),
            Token::literal("__proto__"),
            Token::literal(""),
        ]),
        Token::list(vec![
            Token::pick(vec![
                Token::literal("user"),
                Token::literal("group"),
                Token::literal("__system__"),
            ]),
            Token::pick(vec![
                Token::literal("."),
                Token::literal(":"),
                Token::literal("_"),
                Token::literal("$"),
            ]),
            Token::pick(vec![
                Token::literal("id"),
                Token::literal("$namespace"),
                Token::literal("$"),
            ]),
        ]),
    ])
}

/// Mirrors upstream `str` template.
pub fn str_template() -> Template {
    Template::str(Some(tokens_hello_world()))
}
