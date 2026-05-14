//! Модуль `token`: токены, токенизатор и морфологическое обогащение.
//!
//! Здесь собраны базовые примитивы, которые дальше используются парсером:
//! - [`Token`] / [`TokenType`] — базовая токенизация текста с [`Span`](crate::span::Span)
//! - [`Tokenizer`] — regex-токенизатор с фиксированным набором правил
//! - [`MorphTokenizer`] — обёртка над [`Tokenizer`], автоматически добавляющая морфологию
//! - [`AnyToken`] — унифицированный токен (Plain или Morph)
//! - функции для склейки и работы с последовательностями (`join_*`, `get_*_span`)
//!
//! ## Быстрый старт: токенизация
//!
//! ```rust
//! use renert::token::{Tokenizer, TokenType};
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.tokenize("ул. Ленина д. 10");
//!
//! assert!(!tokens.is_empty());
//! assert_eq!(tokens[0].token_type, TokenType::Russian);
//! assert_eq!(tokens[0].value, "ул");
//! ```
//!
//! Правила токенизации зафиксированы внутри модуля и переиспользуются всеми
//! экземплярами [`Tokenizer`].
//!
//! ## Морфология: MorphTokenizer и AnyToken
//!
//! [`MorphTokenizer`] возвращает `Vec<AnyToken>`:
//! - `TokenType::Russian` → [`AnyToken::Morph`] (с `forms: Vec<Form>`)
//! - остальные → [`AnyToken::Plain`]
//!
//! ```rust,no_run
//! use renert::token::{MorphTokenizer, AnyToken, TokenType};
//!
//! # use renert::error;
//!
//! fn main() -> error::Result<()> {
//! let mt = MorphTokenizer::open()?;
//! let tokens = mt.tokenize("ул. Ленина 10");
//!
//! assert!(matches!(tokens[0], AnyToken::Morph(_))); // "ул"
//! assert!(matches!(tokens[1], AnyToken::Plain(_))); // "."
//! assert!(matches!(tokens[2], AnyToken::Morph(_))); // "Ленина"
//!
//! assert_eq!(tokens[2].token_type(), TokenType::Russian);
//!
//! // Нормализация для Morph зависит от порядка разборов (не всегда однозначно).
//! let n = tokens[2].normalized();
//! assert_eq!(n, n.to_lowercase());
//! # Ok(())
//! # }
//! ```
//!
//! ## Склейка токенов обратно в строку
//!
//! Функции [`join_tokens`] / [`join_normalized_tokens`] вставляют пробел только если между токенами был разрыв
//! в исходном тексте (`next.span.start > prev.span.stop`).
//!
//! ```rust
//! use renert::token::{Tokenizer, join_tokens, join_normalized_tokens};
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.tokenize("ул. Ленина д. 10");
//!
//! let s1 = join_tokens(tokens.iter());
//! let s2 = join_normalized_tokens(tokens.iter());
//!
//! assert_eq!(s1, "ул. Ленина д. 10");
//! assert_eq!(s2, "ул. ленина д. 10");
//! ```
//!
//! ## Получение общего Span для группы токенов
//!
//! ```rust
//! use renert::token::{Tokenizer, get_tokens_span};
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.tokenize("ул. Ленина д. 10");
//!
//! let span = get_tokens_span(&tokens).unwrap();
//! assert!(span.start < span.stop);
//! ```
//!
//! ---
//!
//! Подмодули `tokenizer`, `morph_tokenizer`, `token`, `sequence`, `rules` не объявлены как
//! `pub mod` в API зависимостей: снаружи крейта используйте реэкспорты из `renert::token`.

pub(crate) mod morph_tokenizer;
mod rules;
mod sequence;
#[allow(clippy::module_inception)]
mod token;
mod tokenizer;

pub use morph_tokenizer::{global_morph_tokenizer, Analyser, MorphTokenizer};
pub use sequence::{
    get_any_tokens_span, get_tokens_span, join_any_inflected_tokens, join_any_normalized_tokens,
    join_any_tokens, join_inflected_tokens, join_inflected_tokens_ref, join_normalized_tokens,
    join_tokens,
};
pub use token::{MorphTagToken, MorphToken, TagToken, Token, TokenType};
pub use tokenizer::{AnyToken, AnyTokenOwnedLite, Tokenizer};
