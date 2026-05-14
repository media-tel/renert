//! Типы токенов и их обёртки (морфология и теги).
//!
//! Этот модуль описывает базовый токен [`Token`] (часть исходного текста со [`Span`]),
//! а также дополнительные представления:
//! - [`MorphToken`] — токен + список морфологических разборов (`Vec<Form>`)
//! - [`TagToken`] — токен + произвольный тег (`Tag`)
//! - [`MorphTagToken`] — токен + тег + морфология
//!
//! Основная идея: базовый токен можно «обогащать» информацией, не ломая базовую модель.
//! Все обёртки реализуют [`Deref`] к [`Token`], их удобно использовать там,
//! где ожидается `&Token` (например, для доступа к `value`, `span`, `token_type`).

use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, ops::Deref, str};

use crate::morph::models::Form;
pub use crate::span::*;

fn normalized_from_forms(base: &Token<'_>, forms: &[Form]) -> Cow<'static, str> {
    let is_capitalized = base
        .value
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);

    if is_capitalized {
        if let Some(form) = forms.iter().find(|form| form.grams.contains("Surn")) {
            return form.normalized.clone().into();
        }
    }

    forms
        .first()
        .map(|form| form.normalized.clone().into())
        .unwrap_or_else(|| base.normalized_value())
}

/// Тег токена (произвольная строка).
///
/// Используется для пользовательской разметки: например, `"CITY"`, `"STREET"`,
/// `"ORG"`, либо любые внутренние метки pipeline-а.
///
/// Тип — обычный `String`, чтобы тег было удобно хранить/передавать независимо
/// от времени жизни исходного текста.
pub type Tag = String;

/// Тип токена.
///
/// Обычно задаётся токенизатором по правилам (regex / FSM / и т.п.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    /// Слово на кириллице (русский алфавит).
    Russian,
    /// Email-адрес.
    Email,
    /// Номер телефона.
    Phone,
    /// Доменное имя или host.
    Domain,
    /// Слово на латинице.
    Latin,
    /// Целое число.
    Int,
    /// Знаки пунктуации (точка, запятая и т.п.).
    Punct,
    /// Конец строки (end-of-line), если токенизатор выделяет EOL как отдельный токен.
    EOL,
    /// Прочие токены, не попавшие ни в одну категорию.
    Other,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_str = match self {
            TokenType::Russian => "Russian",
            TokenType::Email => "Email",
            TokenType::Phone => "Phone",
            TokenType::Domain => "Domain",
            TokenType::Latin => "Latin",
            TokenType::Int => "Int",
            TokenType::Punct => "Punct",
            TokenType::EOL => "EOL",
            TokenType::Other => "Other",
        };
        write!(f, "{}", type_str)
    }
}

/// Базовый токен.
///
/// Токен — это:
/// - `value`: строка токена (обычно ссылается на исходный текст через [`Cow::Borrowed`])
/// - `span`: диапазон в исходном тексте [`Span`] (формат `[start, stop)`)
/// - `token_type`: классификация токена [`TokenType`]
///
/// ## Время жизни и `Cow`
///
/// `Token<'a>` может хранить значение:
/// - как заимствованную строку (`Cow::Borrowed(&'a str)`) — дёшево, без аллокаций
/// - как `Owned(String)` — когда нужно отделиться от исходного буфера
///
/// ## Нормализация
///
/// Нормализация по умолчанию — `to_lowercase()`.
/// Если требуется лемматизация/морфология — используйте [`Token::morphed`]
/// и методы `normalized()` у морфологических обёрток.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token<'a> {
    /// Исходная строка токена (borrowed/owned).
    pub value: Cow<'a, str>,
    /// Позиция токена в исходном тексте.
    pub span: Span,
    /// Категория токена.
    pub token_type: TokenType,
}

impl<'a> Token<'a> {
    /// Создать токен, заимствуя `value` у исходного текста.
    ///
    /// Обычно используется токенизатором, который режет исходную строку
    /// и создаёт токены без дополнительных аллокаций.
    pub fn new(value: &'a str, span: Span, token_type: TokenType) -> Self {
        Token {
            value: Cow::Borrowed(value),
            span,
            token_type,
        }
    }

    /// Нормализованное значение токена (нижний регистр).
    ///
    /// Возвращает `Cow<'static, str>`, т.к. `to_lowercase()` создаёт новый `String`.
    ///
    /// Если нужна морфологическая нормализация (лемма), используйте [`Token::morphed`]
    /// и далее [`MorphToken::normalized`] / [`MorphTagToken::normalized`].
    pub fn normalized_value(&self) -> Cow<'static, str> {
        self.value.to_lowercase().into()
    }

    /// Создать нормализованный токен (нижний регистр), сохраняя `span` и `token_type`.
    ///
    /// Возвращаемый токен имеет время жизни `'static`, т.к. значение будет `Owned`.
    pub fn normalized_token(&self) -> Token<'static> {
        Token {
            value: self.normalized_value(),
            span: self.span,
            token_type: self.token_type,
        }
    }

    /// Превратить токен в полностью `Owned` (отвязать от исходного буфера).
    ///
    /// Полезно, если исходная строка скоро будет освобождена, а токены нужно хранить.
    pub fn to_owned(&self) -> Token<'static> {
        Token {
            value: Cow::Owned(self.value.to_string()),
            span: self.span,
            token_type: self.token_type,
        }
    }

    /// Обогатить токен морфологией.
    ///
    /// `forms` — список морфологических разборов (например, из словаря/анализатора).
    /// Порядок обычно важен: первый разбор (`forms[0]`) считается предпочтительным.
    ///
    /// См. [`MorphToken::normalized`].
    pub fn morphed(self, forms: Vec<Form>) -> MorphToken<'a> {
        MorphToken { base: self, forms }
    }

    /// Добавить к токену произвольный тег.
    ///
    /// Используйте теги, чтобы помечать токены на этапах пайплайна
    /// (правила, NER, пользовательские классы и т.п.).
    pub fn tagged(self, tag: Tag) -> TagToken<'a> {
        TagToken { base: self, tag }
    }
}

/// Токен с морфологической информацией.
///
/// Содержит:
/// - `base`: исходный [`Token`]
/// - `forms`: варианты морфологического разбора
///
/// Реализует [`Deref`] к [`Token`], поэтому можно писать `morph_token.value`,
/// `morph_token.span`, `morph_token.token_type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MorphToken<'a> {
    /// Базовый токен.
    pub base: Token<'a>,
    /// Морфологические разборы.
    pub forms: Vec<Form>,
}

impl<'a> Deref for MorphToken<'a> {
    type Target = Token<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<'a> MorphToken<'a> {
    /// Нормализованная форма токена с учётом морфологии.
    ///
    /// Логика:
    /// - если есть `forms.first()`, берём `Form.normalized`
    /// - иначе fallback на [`Token::normalized_value`]
    ///
    /// Это удобно для «лемматизированного сравнения» и построения ключей.
    pub fn normalized(&self) -> Cow<'static, str> {
        normalized_from_forms(&self.base, &self.forms)
    }

    /// Ограничить (заменить) набор морфологических разборов.
    ///
    /// Обычно вызывается после фильтрации разборов по граммемам/условиям.
    pub fn constrained(self, forms: Vec<Form>) -> Self {
        Self { forms, ..self }
    }

    /// Добавить тег к морфологическому токену, получив [`MorphTagToken`].
    pub fn tagged(self, tag: Tag) -> MorphTagToken<'a> {
        MorphTagToken {
            base: self.base,
            tag,
            forms: self.forms,
        }
    }
}

/// Токен с тегом.
///
/// Реализует [`Deref`] к [`Token`], чтобы базовые поля оставались доступны напрямую.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagToken<'a> {
    /// Базовый токен.
    pub base: Token<'a>,
    /// Тег токена.
    pub tag: Tag,
}

impl<'a> Deref for TagToken<'a> {
    type Target = Token<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// Токен с тегом и морфологией.
///
/// Это объединение возможностей [`MorphToken`] и [`TagToken`].
/// Реализует [`Deref`] к [`Token`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MorphTagToken<'a> {
    /// Базовый токен.
    pub base: Token<'a>,
    /// Тег.
    pub tag: Tag,
    /// Морфологические разборы.
    pub forms: Vec<Form>,
}

impl<'a> Deref for MorphTagToken<'a> {
    type Target = Token<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<'a> MorphTagToken<'a> {
    /// Нормализованная форма токена с учётом морфологии.
    ///
    /// Совпадает по логике с [`MorphToken::normalized`].
    pub fn normalized(&self) -> Cow<'static, str> {
        normalized_from_forms(&self.base, &self.forms)
    }

    /// Ограничить (заменить) набор морфологических разборов.
    pub fn constrained(self, forms: Vec<Form>) -> Self {
        Self { forms, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Grams;

    fn form(norm: &str) -> Form {
        Form::new(norm.to_string(), Grams::default(), None)
    }

    fn surn_form(norm: &str) -> Form {
        let mut grams = Grams::default();
        grams.values.insert("Surn".to_string());
        Form::new(norm.to_string(), grams, None)
    }

    #[test]
    fn test_token_creation() {
        let text = "\n";
        let span = Span::new(0, text.len());
        let t = Token::new(text, span, TokenType::EOL);

        assert_eq!(t.value, "\n");
        assert_eq!(t.span, span);
        assert_eq!(t.token_type, TokenType::EOL);
    }

    #[test]
    fn test_token_normalized_value() {
        let text = "НовГоРоД";
        let span = Span::new(0, text.len());
        let t = Token::new(text, span, TokenType::Russian);

        let norm = t.normalized_value();

        assert_eq!(&*norm, "новгород");
        // normalized_value должен возвращать новую (Owned) строку
        assert!(matches!(norm, std::borrow::Cow::Owned(_)));
    }

    #[test]
    fn test_token_normalized_token() {
        let text = "LoNdon";
        let span = Span::new(0, text.len());
        let t = Token::new(text, span, TokenType::Latin);

        let norm_t = t.normalized_token();

        assert_eq!(norm_t.value, "london");
        assert_eq!(norm_t.span, span);
        assert_eq!(norm_t.token_type, TokenType::Latin);

        // normalized_token должен возвращать Owned
        assert!(matches!(norm_t.value, std::borrow::Cow::Owned(_)));
    }

    #[test]
    fn test_borrowed_token_works() {
        let text = "Москва";
        let span = Span::new(0, 6);

        let t = Token::new(text, span, TokenType::Russian);

        // Проверка корректного заимствования токена
        match t.value {
            Cow::Borrowed(s) => assert_eq!(s, "Москва"),
            _ => panic!("Expected Borrowed"),
        }

        assert_eq!(t.span, span);
        assert_eq!(t.token_type, TokenType::Russian);
    }

    #[test]
    fn test_to_owned_converts_to_owned() {
        let text = "Питер";
        let span = Span::new(0, 5);

        let borrowed = Token::new(text, span, TokenType::Russian);
        let owned = borrowed.to_owned();

        // Проверяет конвертацию Borrowed в Owned
        match owned.value {
            Cow::Owned(ref s) => assert_eq!(s, "Питер"),
            _ => panic!("Expected Owned"),
        }

        assert_eq!(owned.span, span);
        assert_eq!(owned.token_type, TokenType::Russian);
    }

    #[test]
    fn test_owned_token_can_be_cloned() {
        // корректность копирования токена
        let t = Token {
            value: Cow::Owned("123".to_string()),
            span: Span::new(0, 3),
            token_type: TokenType::Int,
        };

        let c = t.clone();

        match c.value {
            Cow::Owned(ref s) => assert_eq!(s, "123"),
            _ => panic!("Expected Owned"),
        }
    }

    #[test]
    fn test_display_token_type() {
        assert_eq!(TokenType::Int.to_string(), "Int");
        assert_eq!(TokenType::Russian.to_string(), "Russian");
        assert_eq!(TokenType::Email.to_string(), "Email");
        assert_eq!(TokenType::Phone.to_string(), "Phone");
        assert_eq!(TokenType::Domain.to_string(), "Domain");
        assert_eq!(TokenType::EOL.to_string(), "EOL");
    }

    #[test]
    fn test_morph_token_normalized_fallback_to_lowercase_when_no_forms() {
        let text = "НоВгОрОд";
        let span = Span::new(0, text.len());

        let token = Token::new(text, span, TokenType::Russian);
        let morph_token = token.morphed(vec![]);

        // Нет форм => fallback на lowercase базового токена
        assert_eq!(&*morph_token.normalized(), "новгород");
    }

    #[test]
    fn test_morph_token_normalized_uses_first_form_when_forms_present() {
        let text = "НоВгОрОда";
        let span = Span::new(0, text.len());

        let token = Token::new(text, span, TokenType::Russian);
        let morph_token = token.morphed(vec![form("новгород")]);

        // forms есть => normalized берётся из forms[0].normalized
        assert_eq!(&*morph_token.normalized(), "новгород");
    }

    #[test]
    fn test_morph_normalized_prefers_surn_form_for_capitalized_tokens() {
        let text = "Иванова";
        let span = Span::new(0, text.len());
        let token = Token::new(text, span, TokenType::Russian);

        let morph = token
            .clone()
            .morphed(vec![form("иванов"), surn_form("иванова")]);
        assert_eq!(&*morph.normalized(), "иванова");

        let tagged = token
            .morphed(vec![form("иванов"), surn_form("иванова")])
            .tagged("SURNAME".to_string());
        assert_eq!(&*tagged.normalized(), "иванова");
    }

    #[test]
    fn test_morph_token_constrained_replaces_forms() {
        let text = "НОВГОРОД";
        let span = Span::new(0, text.len());

        let base = Token::new(text, span, TokenType::Russian);
        let morph = base.morphed(vec![form("лемма1"), form("лемма2")]);

        let constrained = morph.constrained(vec![form("только1")]);

        assert_eq!(constrained.forms.len(), 1);
        assert_eq!(constrained.forms[0].normalized, "только1");
        assert_eq!(&*constrained.normalized(), "только1");
    }

    #[test]
    fn test_tag_token_holds_tag_and_derefs_to_token() {
        let text = "Москва";
        let span = Span::new(0, text.len());

        let base = Token::new(text, span, TokenType::Russian);
        let tagged = base.tagged("CITY".to_string());

        assert_eq!(tagged.tag, "CITY");
        // Deref: доступ к полям Token
        assert_eq!(tagged.value, "Москва");
        assert_eq!(tagged.span, span);
        assert_eq!(tagged.token_type, TokenType::Russian);
    }

    #[test]
    fn test_morph_tag_token_has_both_tag_and_forms() {
        let text = "НОВГОРОДА";
        let span = Span::new(0, text.len());

        let base = Token::new(text, span, TokenType::Russian);
        let morph = base.morphed(vec![form("новгород")]);
        let mt = morph.tagged("LOC".to_string());

        assert_eq!(mt.tag, "LOC");
        assert_eq!(mt.forms.len(), 1);
        assert_eq!(&*mt.normalized(), "новгород");

        // constrained должен сохранять tag
        let mt2 = mt.constrained(vec![form("новгород_2")]);
        assert_eq!(mt2.tag, "LOC");
        assert_eq!(&*mt2.normalized(), "новгород_2");
    }

    #[test]
    fn test_morph_token_tagged_carries_tag_and_preserves_base_fields() {
        let text = "НОВГОРОДА";
        let span = Span::new(10, 10 + text.len());

        let token = Token::new(text, span, TokenType::Russian);
        let morph = token.morphed(vec![form("новгород")]);

        let mt = morph.tagged("LOC".to_string());

        // tag должен проставиться
        assert_eq!(mt.tag, "LOC");

        // base поля должны сохраниться
        assert_eq!(mt.value, text);
        assert_eq!(mt.span, span);
        assert_eq!(mt.token_type, TokenType::Russian);

        // forms должны перенестись без потерь
        assert_eq!(mt.forms.len(), 1);
        assert_eq!(mt.forms[0].normalized, "новгород");

        // normalized() должен брать из forms[0]
        assert_eq!(&*mt.normalized(), "новгород");
    }

    #[test]
    fn test_morph_token_tagged_preserves_all_forms_order() {
        let text = "ТЕСТ";
        let span = Span::new(0, text.len());

        let token = Token::new(text, span, TokenType::Russian);
        let morph = token.morphed(vec![form("лемма1"), form("лемма2"), form("лемма3")]);

        let mt = morph.tagged("X".to_string());

        assert_eq!(mt.tag, "X");
        assert_eq!(mt.forms.len(), 3);
        assert_eq!(mt.forms[0].normalized, "лемма1");
        assert_eq!(mt.forms[1].normalized, "лемма2");
        assert_eq!(mt.forms[2].normalized, "лемма3");

        // normalized должен использовать первую форму
        assert_eq!(&*mt.normalized(), "лемма1");
    }

    #[test]
    fn test_morph_token_tagged_with_empty_forms_falls_back_to_lowercase() {
        let text = "НоВгОрОд";
        let span = Span::new(0, text.len());

        let token = Token::new(text, span, TokenType::Russian);
        let morph = token.morphed(vec![]);

        let mt = morph.tagged("CITY".to_string());

        assert_eq!(mt.tag, "CITY");
        assert!(mt.forms.is_empty());

        // forms пусты => fallback на lowercase базового токена
        assert_eq!(&*mt.normalized(), "новгород");
    }

    #[test]
    fn test_morph_tag_token_constrained_preserves_tag_and_base() {
        let text = "НОВГОРОДА";
        let span = Span::new(5, 5 + text.len());

        let base = Token::new(text, span, TokenType::Russian);
        let morph = base.morphed(vec![form("новгород")]);
        let mt = morph.tagged("LOC".to_string());

        let mt2 = mt.constrained(vec![form("новгород_2")]);

        // tag должен сохраниться
        assert_eq!(mt2.tag, "LOC");

        // base поля должны сохраниться
        assert_eq!(mt2.value, text);
        assert_eq!(mt2.span, span);
        assert_eq!(mt2.token_type, TokenType::Russian);

        // forms заменились
        assert_eq!(mt2.forms.len(), 1);
        assert_eq!(mt2.forms[0].normalized, "новгород_2");
        assert_eq!(&*mt2.normalized(), "новгород_2");
    }

    #[test]
    fn test_morph_tag_token_deref_exposes_token_fields() {
        let text = "Москву";
        let span = Span::new(0, text.len());

        // Собираем MorphTagToken
        let base = Token::new(text, span, TokenType::Russian);
        let morph = base.morphed(vec![form("москва")]);
        let mt = morph.tagged("CITY".to_string());

        // Deref -> Token: доступ к полям как у Token
        assert_eq!(&*mt.normalized(), "москва");
        assert_eq!(mt.span, span);
        assert_eq!(mt.token_type, TokenType::Russian);
    }

    #[test]
    fn test_tag_token_tagged_sets_tag_and_preserves_base_fields() {
        let text = "123";
        let span = Span::new(2, 2 + text.len());

        let token = Token::new(text, span, TokenType::Int);
        let tagged = token.tagged("NUMBER".to_string());

        // tag должен проставиться
        assert_eq!(tagged.tag, "NUMBER");

        // base поля должны сохраниться
        assert_eq!(tagged.value, text);
        assert_eq!(tagged.span, span);
        assert_eq!(tagged.token_type, TokenType::Int);
    }

    #[test]
    fn test_tag_token_deref_exposes_token_fields() {
        let text = "!";
        let span = Span::new(0, text.len());

        let token = Token::new(text, span, TokenType::Punct);
        let tagged = token.tagged("PUNCT".to_string());

        // Deref -> Token
        assert_eq!(tagged.value, "!");
        assert_eq!(tagged.span, span);
        assert_eq!(tagged.token_type, TokenType::Punct);
    }
}
