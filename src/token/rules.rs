//! Fixed tokenization rules used by [`Tokenizer`].

use crate::token::TokenType;

/// Фиксированный порядок правил токенизации.
///
/// Порядок важен: при совпадении альтернатив в объединённом regex выигрывает самое
/// левое правило, поэтому здесь зафиксирован текущий приоритет токенов.
pub(crate) const DEFAULT_TOKEN_RULES: &[(TokenType, &str)] = &[
    (TokenType::Russian, r"[А-Яа-яЁё]+"),
    (
        TokenType::Email,
        r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+",
    ),
    (
        TokenType::Phone,
        r"(?:\+?\d{10,14}|\+?\d[\s_-]?\(?\d{3}\)?(?:[\s_-]?\d{2,4}){3})",
    ),
    (
        TokenType::Domain,
        r"(?:[a-zA-Z0-9-]+\.)+[a-zA-Z][a-zA-Z0-9-]*",
    ),
    (TokenType::Latin, r"[a-zA-Z]+"),
    (TokenType::Int, r"\d+"),
    (
        TokenType::Punct,
        r#"[-\\/!#$%&()\[\]\*\+,\.:;<=>?@^_`{|}~#…"\'«»„“ʼʻ”]"#,
    ),
    (TokenType::EOL, r"[\n\r]+"),
    (TokenType::Other, r"\S"),
];
