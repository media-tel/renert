//! Regex-токенизатор с фиксированным набором правил.
//!
//! Модуль предоставляет:
//! - [`Tokenizer`] — готовый токенизатор с фиксированным порядком regex-правил
//! - [`AnyToken`] — обёртку “любой токен” (обычный или морфологический) с единым API `normalized()/value()/span()/token_type()`
//!
//! ## Идея работы
//!
//! Токенизация происходит одним проходом регулярного выражения, собранного
//! из фиксированного набора правил. Каждое правило становится отдельной
//! capture-группой в объединённом паттерне:
//!
//! - `rules[i]` → `(?P<rule_i>{pattern_i})`
//! - итоговый regex: `(?P<rule_0>...)|(?P<rule_1>...)|...`
//!
//! При каждом совпадении определяем, какая capture-группа сработала первой,
//! и по её индексу берём соответствующий [`TokenType`] из `token_types`.
//!
//! ## Важно про приоритет правил
//!
//! При совпадении альтернатив в regex побеждает **самое левое правило**,
//! которое может сматчиться в данной позиции. Поэтому порядок правил критичен
//! и зафиксирован в модуле [`rules`].
//!
//! ## Ограничения
//!
//! - Токенизатор не вставляет “пробельные” токены: он возвращает только то, что совпало с правилами.
//! - Если паттерн правила может матчить пустую строку, `captures_iter` будет вести себя неожиданно.
//!   Необходимо писать правила так, чтобы матч всегда имел ненулевую длину.
//!
//! ## Связь с морфологией
//!
//! [`Tokenizer`] возвращает базовые [`Token`]. Морфология навешивается отдельным шагом
//! (например, `token.morphed(forms)`), а [`AnyToken`] позволяет унифицировать доступ к полям
//! и нормализации между обычными и морфологическими токенами.

use crate::morph::models::Form;
use regex::Regex;
use std::borrow::Cow;

pub use crate::span::*;
use crate::token::rules::DEFAULT_TOKEN_RULES;
use crate::token::token::{MorphToken, Token, TokenType};

/// Универсальный токен: либо обычный [`Token`], либо токен с морфологией [`MorphToken`].
///
/// Полезно в местах, где на разных этапах пайплайна токен может быть “обогащён” морфологией,
/// но нужен единый API:
/// - `normalized()` — нормализация (лемма, если есть морфология; иначе lowercase)
/// - `value()`, `span()`, `token_type()` — доступ к базовым полям
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnyToken<'a> {
    /// Базовый токен без морфологии.
    Plain(Token<'a>),
    /// Токен с морфологическими разборами.
    Morph(MorphToken<'a>),
}

impl<'a> AnyToken<'a> {
    /// Нормализованное значение:
    /// - для [`AnyToken::Morph`] берётся `forms[0].normalized` (если есть разборы)
    /// - иначе — `value.to_lowercase()`
    pub fn normalized(&self) -> Cow<'static, str> {
        match self {
            AnyToken::Plain(t) => t.normalized_value(),
            AnyToken::Morph(mt) => mt.normalized(),
        }
    }

    /// Тип токена [`TokenType`].
    pub fn token_type(&self) -> TokenType {
        match self {
            AnyToken::Plain(t) => t.token_type,
            AnyToken::Morph(mt) => mt.token_type, // через Deref к Token
        }
    }

    /// Исходное значение токена (срез исходного текста).
    pub fn value(&self) -> &str {
        match self {
            AnyToken::Plain(t) => t.value.as_ref(),
            AnyToken::Morph(mt) => mt.value.as_ref(),
        }
    }

    /// Диапазон [`Span`] в исходной строке.
    pub fn span(&self) -> Span {
        match self {
            AnyToken::Plain(t) => t.span,
            AnyToken::Morph(mt) => mt.span,
        }
    }

    /// Возвращает fully-owned версию токена без привязки к исходному тексту.
    ///
    /// Для морфологического варианта копируются `forms`.
    pub fn to_owned(&self) -> AnyToken<'static> {
        match self {
            AnyToken::Plain(t) => AnyToken::Plain(t.to_owned()),
            AnyToken::Morph(mt) => {
                // Token::to_owned отвязывает value от исходной строки
                let base: Token<'static> = mt.base.to_owned();
                AnyToken::Morph(MorphToken {
                    base,
                    forms: mt.forms.clone(),
                })
            }
        }
    }
}

/// Компактный fully-owned токен без хранения исходного `value`.
///
/// Используется в сценариях, где нужно хранить только:
/// - позицию [`Span`]
/// - [`TokenType`]
/// - (опционально) морфологические формы
///
/// Значение токена при необходимости восстанавливается из исходного текста через [`AnyTokenOwnedLite::value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyTokenOwnedLite {
    /// Обычный токен без морфологии.
    Plain {
        /// Диапазон токена в исходной строке.
        span: Span,
        /// Тип токена.
        token_type: TokenType,
    },
    /// Морфологический токен с сохраненными разборами.
    Morph {
        /// Диапазон токена в исходной строке.
        span: Span,
        /// Тип токена.
        token_type: TokenType,
        /// Морфологические разборы.
        forms: Vec<Form>,
    },
}

impl AnyTokenOwnedLite {
    /// Возвращает диапазон токена в исходной строке.
    pub fn span(&self) -> Span {
        match self {
            AnyTokenOwnedLite::Plain { span, .. } => *span,
            AnyTokenOwnedLite::Morph { span, .. } => *span,
        }
    }

    /// Возвращает тип токена.
    pub fn token_type(&self) -> TokenType {
        match self {
            AnyTokenOwnedLite::Plain { token_type, .. } => *token_type,
            AnyTokenOwnedLite::Morph { token_type, .. } => *token_type,
        }
    }

    /// Вырезает исходное значение токена из `text` по сохраненному [`Span`].
    ///
    /// # Panics
    ///
    /// Паникует, если `Span` выходит за границы `text` или не попадает на границы UTF-8 символов.
    pub fn value<'t>(&self, text: &'t str) -> &'t str {
        let s = self.span();
        &text[s.start..s.stop]
    }

    /// Возвращает нормализованное значение токена.
    ///
    /// - для `Morph` используется `forms[0].normalized`, если формы есть;
    /// - иначе используется `to_lowercase()` по срезу `text[span]`.
    ///
    /// # Panics
    ///
    /// Паникует, если `Span` выходит за границы `text` или не попадает на границы UTF-8 символов.
    pub fn normalized(&self, text: &str) -> Cow<'static, str> {
        match self {
            AnyTokenOwnedLite::Morph { forms, span, .. } => forms
                .first()
                .map(|f| f.normalized.clone().into())
                .unwrap_or_else(|| text[span.start..span.stop].to_lowercase().into()),
            AnyTokenOwnedLite::Plain { span, .. } => {
                text[span.start..span.stop].to_lowercase().into()
            }
        }
    }
}

impl<'a> From<&crate::token::AnyToken<'a>> for AnyTokenOwnedLite {
    fn from(t: &crate::token::AnyToken<'a>) -> Self {
        match t {
            crate::token::AnyToken::Plain(tok) => AnyTokenOwnedLite::Plain {
                span: tok.span,
                token_type: tok.token_type,
            },
            crate::token::AnyToken::Morph(mt) => AnyTokenOwnedLite::Morph {
                span: mt.span,
                token_type: mt.token_type,
                forms: mt.forms.clone(),
            },
        }
    }
}

/// Regex-токенизатор.
///
/// Хранит скомпилированное регулярное выражение и порядок типов токенов,
/// соответствующий capture-группам.
/// При tokenization выполняется один `captures_iter` по входному тексту.
///
/// ### Приоритет
/// Порядок фиксированных правил определяет приоритет: чем ближе к началу, тем выше.
pub struct Tokenizer {
    regex: Regex,
    /// `token_types[i]` соответствует capture-группе `(i+1)`,
    /// потому что `0` — это весь матч.
    token_types: Vec<TokenType>,
}

impl Tokenizer {
    /// Создаёт токенизатор с фиксированным набором правил.
    pub fn new() -> Self {
        let (regex, token_types) = Self::compile_default();
        Self { regex, token_types }
    }

    /// Компилирует объединённый regex из фиксированного набора правил.
    ///
    /// Каждое правило становится именованной группой `(?P<rule_i>...)`.
    /// Возвращает:
    /// - `Regex` — объединённый паттерн
    /// - `Vec<TokenType>` — соответствие индексу capture-группы (i+1) → тип токена
    fn compile_default() -> (Regex, Vec<TokenType>) {
        let mut parts = Vec::with_capacity(DEFAULT_TOKEN_RULES.len());
        let mut token_types = Vec::with_capacity(DEFAULT_TOKEN_RULES.len());

        for (i, (token_type, pattern)) in DEFAULT_TOKEN_RULES.iter().enumerate() {
            let name = format!("rule_{}", i);
            let part = format!(r"(?P<{}>{})", name, pattern);
            parts.push(part);
            token_types.push(*token_type);
        }

        let pattern = parts.join("|");
        let regex = Regex::new(&pattern).expect("Invalid combined regex");
        (regex, token_types)
    }

    /// Токенизирует входной текст и возвращает список [`Token`].
    ///
    /// Для каждого совпадения объединённого regex:
    /// 1. находим первую сработавшую capture-группу (индексы 1..)
    /// 2. определяем тип токена по `token_types[i-1]`
    /// 3. создаём [`Token`] со срезом исходной строки (`Cow::Borrowed`)
    ///
    /// Возвращаемые токены ссылаются на `text`, поэтому имеют lifetime `'a`.
    pub fn tokenize<'a>(&self, text: &'a str) -> Vec<Token<'a>> {
        let mut out = Vec::new();

        for caps in self.regex.captures_iter(text) {
            let mut found: Option<(usize, usize, TokenType)> = None;

            // caps[0] — весь матч, поэтому начинаем с 1
            for (i, m) in caps.iter().enumerate().skip(1) {
                if let Some(m) = m {
                    let token_type = self.token_types[i - 1];
                    found = Some((m.start(), m.end(), token_type));
                    break;
                }
            }

            let (start, stop, token_type) =
                found.expect("Combined regex matched but no capture group matched");

            out.push(Token {
                value: Cow::Borrowed(&text[start..stop]),
                span: Span { start, stop },
                token_type,
            });
        }

        out
    }

    /// Разбивает строку на части и возвращает только значения токенов.
    ///
    /// Это упрощённый вариант [`Tokenizer::tokenize`], когда `span` и `token_type`
    /// не нужны.
    pub fn split<'a>(&self, text: &'a str) -> Vec<Cow<'a, str>> {
        self.tokenize(text).into_iter().map(|t| t.value).collect()
    }
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Form;
    use crate::morph::models::Grams;

    fn form(norm: &str) -> Form {
        Form::new(norm.to_string(), Grams::default(), None)
    }

    #[test]
    fn test_basic_tokenization() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("ул. Ленина д. 10");

        assert_eq!(tokens.len(), 6);

        assert_eq!(tokens[0].value, "ул");
        assert_eq!(tokens[0].token_type, TokenType::Russian);

        assert_eq!(tokens[1].value, ".");
        assert_eq!(tokens[1].token_type, TokenType::Punct);

        assert_eq!(tokens[2].value, "Ленина");
        assert_eq!(tokens[2].token_type, TokenType::Russian);

        assert_eq!(tokens[3].value, "д");
        assert_eq!(tokens[3].token_type, TokenType::Russian);

        assert_eq!(tokens[4].value, ".");
        assert_eq!(tokens[4].token_type, TokenType::Punct);

        assert_eq!(tokens[5].value, "10");
        assert_eq!(tokens[5].token_type, TokenType::Int);
    }

    #[test]
    fn test_numbers() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("10 25 300");

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[2].value, "300");
        assert_eq!(tokens[1].token_type, TokenType::Int);
    }

    #[test]
    fn test_latin() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("London, Baker St. 221B");

        assert_eq!(tokens.len(), 7);

        assert_eq!(tokens[0].value, "London");
        assert_eq!(tokens[0].token_type, TokenType::Latin);

        assert_eq!(tokens[1].value, ",");
        assert_eq!(tokens[1].token_type, TokenType::Punct);

        assert_eq!(tokens[2].value, "Baker");
        assert_eq!(tokens[2].token_type, TokenType::Latin);

        assert_eq!(tokens[3].value, "St");
        assert_eq!(tokens[3].token_type, TokenType::Latin);

        assert_eq!(tokens[4].value, ".");
        assert_eq!(tokens[4].token_type, TokenType::Punct);

        assert_eq!(tokens[5].value, "221");
        assert_eq!(tokens[5].token_type, TokenType::Int);

        assert_eq!(tokens[6].value, "B");
        assert_eq!(tokens[6].token_type, TokenType::Latin);
    }

    #[test]
    fn test_punctuation() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("Привет, мир!");

        assert_eq!(tokens.len(), 4);

        assert_eq!(tokens[0].value, "Привет");
        assert_eq!(tokens[1].token_type, TokenType::Punct);
        assert_eq!(tokens[2].value, "мир");
        assert_eq!(tokens[3].token_type, TokenType::Punct);
    }

    #[test]
    fn test_address() {
        let tokenizer = Tokenizer::new();

        let text = "межсел.тер. Межселенные территории Алеутского района, остров Беринга";

        let tokens = tokenizer.tokenize(text);

        assert!(tokens.len() > 6);
        assert_eq!(tokens[0].value, "межсел");
        assert_eq!(tokens[1].value, ".");

        assert_eq!(tokens[2].token_type, TokenType::Russian);
        assert_eq!(tokens[7].token_type, TokenType::Russian);
    }

    #[test]
    fn test_empty_string() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("");

        assert!(tokens.is_empty());
    }

    #[test]
    fn test_span_positions() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Привет,мир");

        // "Привет" занимает 12 байт
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.stop, 12);

        // "," - 1 байт
        assert_eq!(tokens[1].span.start, 12);
        assert_eq!(tokens[1].span.stop, 13);

        // "мир" - 6 байт
        assert_eq!(tokens[2].span.start, 13);
        assert_eq!(tokens[2].span.stop, 19);
    }

    #[test]
    fn test_newline() {
        let tokenizer = Tokenizer::new();

        let tokens_1 = tokenizer.tokenize("\n");
        assert_eq!(tokens_1.len(), 1);

        assert_eq!(tokens_1[0].value, "\n");
        assert_eq!(tokens_1[0].token_type, TokenType::EOL);

        let tokens_2 = tokenizer.tokenize("\n\n\r");
        assert_eq!(tokens_2.len(), 1);

        assert_eq!(tokens_2[0].value, "\n\n\r");
        assert_eq!(tokens_2[0].token_type, TokenType::EOL);
    }

    #[test]
    fn test_mixed_text() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("ул@12");

        assert_eq!(tokens.len(), 3);

        assert_eq!(tokens[0].value, "ул");
        assert_eq!(tokens[0].token_type, TokenType::Russian);

        assert_eq!(tokens[1].value, "@");
        assert_eq!(tokens[1].token_type, TokenType::Punct);

        assert_eq!(tokens[2].value, "12");
        assert_eq!(tokens[2].token_type, TokenType::Int);
    }

    #[test]
    fn test_other_special_characters() {
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize("$%=#");

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].value, "$");
        assert_eq!(tokens[1].value, "%");
        assert_eq!(tokens[2].value, "=");
        assert_eq!(tokens[3].value, "#");
    }

    fn spans_from_pieces(s: &str, pieces: &[&str]) -> Vec<Span> {
        let mut spans = Vec::with_capacity(pieces.len());
        let mut pos = 0usize;
        for &p in pieces {
            // ищем p начиная с текущей позиции (чтобы работало для повторов)
            let rel = s[pos..].find(p).expect("piece not found");
            let start = pos + rel;
            let stop = start + p.len(); // len() в байтах — именно то, что нужно
            spans.push(Span::new(start, stop));
            pos = stop;
        }
        spans
    }

    #[test]
    fn test_types_like_yargy_bytespan() {
        let tokenizer = Tokenizer::new();
        let s = "Ростов-на-Дону";

        let spans = spans_from_pieces(s, &["Ростов", "-", "на", "-", "Дону"]);
        let tokens = tokenizer.tokenize(s);

        assert_eq!(
            tokens,
            vec![
                Token::new("Ростов", spans[0], TokenType::Russian),
                Token::new("-", spans[1], TokenType::Punct),
                Token::new("на", spans[2], TokenType::Russian),
                Token::new("-", spans[3], TokenType::Punct),
                Token::new("Дону", spans[4], TokenType::Russian),
            ]
        );
    }

    #[test]
    fn test_tokenizer_split_returns_cow_and_matches_tokenize() {
        let tokenizer = Tokenizer::new();
        let text = "pi = 3.14";

        let split = tokenizer.split(text);
        let split_refs: Vec<&str> = split.iter().map(|c| c.as_ref()).collect();

        let toks = tokenizer.tokenize(text);
        let tok_refs: Vec<&str> = toks.iter().map(|t| t.value.as_ref()).collect();

        assert_eq!(split_refs, tok_refs);
    }

    #[test]
    fn test_fixed_rules_cover_core_token_types() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("A\nБ#1");
        let token_types: Vec<TokenType> = tokens.iter().map(|t| t.token_type).collect();

        assert_eq!(
            token_types,
            vec![
                TokenType::Latin,
                TokenType::EOL,
                TokenType::Russian,
                TokenType::Punct,
                TokenType::Int,
            ]
        );
    }

    #[test]
    fn test_domain_rule_has_priority_over_latin_and_punct() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("на сайте www.VKontakte.ru или www.google.com");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].value, "на");
        assert_eq!(tokens[0].token_type, TokenType::Russian);
        assert_eq!(tokens[1].value, "сайте");
        assert_eq!(tokens[1].token_type, TokenType::Russian);
        assert_eq!(tokens[2].value, "www.VKontakte.ru");
        assert_eq!(tokens[2].token_type, TokenType::Domain);
        assert_eq!(tokens[3].value, "или");
        assert_eq!(tokens[3].token_type, TokenType::Russian);
        assert_eq!(tokens[4].value, "www.google.com");
        assert_eq!(tokens[4].token_type, TokenType::Domain);
    }

    #[test]
    fn test_email_and_phone_rules_match_as_single_tokens() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("mail me@test.com or call +7 (999) 123-45-67 89991234567");

        assert!(tokens
            .iter()
            .any(|t| t.value == "me@test.com" && t.token_type == TokenType::Email));
        assert!(tokens
            .iter()
            .any(|t| t.value == "+7 (999) 123-45-67" && t.token_type == TokenType::Phone));
        assert!(tokens
            .iter()
            .any(|t| t.value == "89991234567" && t.token_type == TokenType::Phone));
    }

    #[test]
    fn test_domain_rule_does_not_capture_decimal_numbers() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("3.14");

        assert_eq!(
            tokens,
            vec![
                Token::new("3", Span::new(0, 1), TokenType::Int),
                Token::new(".", Span::new(1, 2), TokenType::Punct),
                Token::new("14", Span::new(2, 4), TokenType::Int),
            ]
        );
    }

    #[test]
    fn test_anytoken_accessors_plain_and_morph() {
        let text = "НОВГОРОДА";
        let span = Span::new(5, 5 + text.len());
        let base = Token::new(text, span, TokenType::Russian);

        let plain = AnyToken::Plain(base.clone());
        assert_eq!(plain.value(), "НОВГОРОДА");
        assert_eq!(plain.token_type(), TokenType::Russian);
        assert_eq!(plain.span(), span);
        assert_eq!(&*plain.normalized(), "новгорода"); // lowercase fallback

        let morph = AnyToken::Morph(base.morphed(vec![form("новгород")]));
        assert_eq!(morph.value(), "НОВГОРОДА");
        assert_eq!(morph.token_type(), TokenType::Russian);
        assert_eq!(morph.span(), span);
        assert_eq!(&*morph.normalized(), "новгород"); // из forms[0]
    }
}
