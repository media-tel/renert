//! Полиморфный вход для методов `Parser::find` / `Parser::findall`.
//!
//! Трейт [`FindInput`] позволяет передавать в парсер как строку, так и
//! готовый срез токенов, сохраняя единый API.

use crate::token::Token;

use super::engine::Parser;
use super::match_result::{Match, MatchBorrowed};

/// Полиморфный вход для методов `Parser::find` / `Parser::findall`.
///
/// Позволяет единый API для строки и для среза токенов.
pub trait FindInput<'a> {
    /// Тип результата `find`.
    type One;
    /// Тип результата `findall`.
    type Many;

    /// Реализация поиска одного совпадения.
    fn parser_find(self, parser: &Parser<'_>) -> Self::One;
    /// Реализация поиска всех совпадений.
    fn parser_findall(self, parser: &Parser<'_>) -> Self::Many;
}

impl<'a> FindInput<'a> for &'a str {
    type One = Option<Match>;
    type Many = Vec<Match>;

    fn parser_find(self, parser: &Parser<'_>) -> Self::One {
        parser.find_match(self)
    }

    fn parser_findall(self, parser: &Parser<'_>) -> Self::Many {
        parser.findall_match(self)
    }
}

impl<'a> FindInput<'a> for &'a [Token<'a>] {
    type One = Option<MatchBorrowed<'a>>;
    type Many = Vec<MatchBorrowed<'a>>;

    fn parser_find(self, parser: &Parser<'_>) -> Self::One {
        parser.find_tokens(self)
    }

    fn parser_findall(self, parser: &Parser<'_>) -> Self::Many {
        parser.findall_tokens(self)
    }
}

impl<'a> FindInput<'a> for &'a Vec<Token<'a>> {
    type One = Option<MatchBorrowed<'a>>;
    type Many = Vec<MatchBorrowed<'a>>;

    fn parser_find(self, parser: &Parser<'_>) -> Self::One {
        parser.find_tokens(self.as_slice())
    }

    fn parser_findall(self, parser: &Parser<'_>) -> Self::Many {
        parser.findall_tokens(self.as_slice())
    }
}
