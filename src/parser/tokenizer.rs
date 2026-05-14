//! Абстракции токенизатора и тэггера для парсера.
//!
//! - [`ParserTokenizer`] — интерфейс токенизации текста в `AnyToken`;
//! - [`Tagger`] — интерфейс пост-обработки токенов перед парсингом;
//! - [`PassTagger`] — тэггер по умолчанию (no-op).

use crate::token::{AnyToken, MorphTokenizer, Token, Tokenizer};

/// Абстракция токенизатора для парсера.
pub trait ParserTokenizer: Send + Sync {
    /// Токенизирует текст в последовательность `AnyToken`.
    fn tokenize<'t>(&self, text: &'t str) -> Vec<AnyToken<'t>>;
}

impl ParserTokenizer for Tokenizer {
    fn tokenize<'t>(&self, text: &'t str) -> Vec<AnyToken<'t>> {
        self.tokenize(text)
            .into_iter()
            .map(AnyToken::Plain)
            .collect()
    }
}

impl ParserTokenizer for MorphTokenizer {
    fn tokenize<'t>(&self, text: &'t str) -> Vec<AnyToken<'t>> {
        MorphTokenizer::tokenize(self, text)
    }
}

/// Абстракция тэггера (пост-обработки токенов) перед парсингом.
pub trait Tagger: Send + Sync {
    /// Преобразует/фильтрует входные токены.
    fn tag<'t>(&self, tokens: Vec<AnyToken<'t>>) -> Vec<AnyToken<'t>>;
}

/// Тэггер по умолчанию: не меняет токены.
#[derive(Debug, Default)]
pub struct PassTagger;

impl Tagger for PassTagger {
    fn tag<'t>(&self, tokens: Vec<AnyToken<'t>>) -> Vec<AnyToken<'t>> {
        tokens
    }
}

/// Возвращает "базовый" plain-токен из `AnyToken`.
pub(super) fn anytoken_to_token<'t>(t: &AnyToken<'t>) -> Token<'t> {
    match t {
        AnyToken::Plain(tok) => tok.clone(),
        AnyToken::Morph(mt) => mt.base.clone(),
    }
}
