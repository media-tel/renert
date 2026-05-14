use crate::predicates::constructors::{Predicate, TokenView};
use crate::token::TokenType;

use std::borrow::Cow;
use std::collections::HashSet;

/// Предикат точного совпадения значения токена.
#[derive(Debug, Clone)]
pub struct Eq<'a> {
    pub value: Cow<'a, str>,
}

impl<'a> Predicate for Eq<'a> {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value == self.value.as_ref()
    }
}

/// Числовой предикат `>= value`.
///
/// Если токен не парсится в `i64`, возвращает `false`.
#[derive(Debug, Clone)]
pub struct Gte {
    pub value: i64,
}

impl Predicate for Gte {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        match token_view.token().value.parse::<i64>() {
            Ok(n) => n >= self.value,
            Err(_) => false,
        }
    }
}

/// Числовой предикат `<= value`.
///
/// Если токен не парсится в `i64`, возвращает `false`.
#[derive(Debug, Clone)]
pub struct Lte {
    pub value: i64,
}

impl Predicate for Lte {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        match token_view.token().value.parse::<i64>() {
            Ok(n) => n <= self.value,
            Err(_) => false,
        }
    }
}

/// Регистронезависимое сравнение со строкой.
#[derive(Debug, Clone)]
pub struct Caseless<'a> {
    pub value: Cow<'a, str>,
}

impl<'a> Predicate for Caseless<'a> {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.to_lowercase() == self.value.to_lowercase()
    }
}

/// Сравнение с нормализованной формой (леммой).
///
/// Для морфологических токенов проверяет `Form.normalized` у всех разборов.
/// Для plain-токенов сравнивает `value.to_lowercase()`.
#[derive(Debug, Clone)]
pub struct Normalized<'a> {
    pub value: &'a str,
}

impl<'a> Predicate for Normalized<'a> {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        let needle = self.value.to_lowercase();

        if let Some(forms) = token_view.forms() {
            // Вариант 1: проверяем любую лемму среди разборов
            return forms.iter().any(|f| f.normalized == needle);
        }

        // для non-morph токенов просто сравнение в нижнем регистре
        token_view.token().value.to_lowercase() == needle
    }
}

/// Регистрозависимое вхождение в набор значений.
#[derive(Debug, Clone)]
pub struct In<'a> {
    pub values: HashSet<Cow<'a, str>>,
}

impl<'a> Predicate for In<'a> {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        self.values.contains(&token_view.token().value)
    }
}

/// Регистронезависимое вхождение в набор значений.
#[derive(Debug, Clone)]
pub struct InCaseless {
    pub values: HashSet<String>,
}

impl Predicate for InCaseless {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        self.values
            .contains(&token_view.token().value.to_lowercase())
    }
}

/// Проверяет формат Title Case: первый символ заглавный, остальные строчные.
#[derive(Debug, Clone)]
pub struct IsTitle;

impl Predicate for IsTitle {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        let s = token_view.token().value.as_ref();

        let mut chars = s.chars();
        let first = match chars.next() {
            Some(c) => c,
            None => return false, // пустой токен — не Title
        };

        if !first.is_uppercase() {
            return false;
        }

        chars.all(|c| c.is_lowercase())
    }
}

/// Проверяет, что все символы токена верхнего регистра.
#[derive(Debug, Clone)]
pub struct IsUpper;

impl Predicate for IsUpper {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.chars().all(|c| c.is_uppercase())
    }
}

/// Проверяет, что все символы токена нижнего регистра.
#[derive(Debug, Clone)]
pub struct IsLower;

impl Predicate for IsLower {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.chars().all(|c| c.is_lowercase())
    }
}

/// Проверяет, что токен состоит только из буквенных символов.
#[derive(Debug, Clone)]
pub struct IsWord;

impl Predicate for IsWord {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.chars().all(|c| c.is_alphabetic())
    }
}

/// Проверяет, что токен состоит только из ASCII-цифр.
#[derive(Debug, Clone)]
pub struct IsDigit;

impl Predicate for IsDigit {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.chars().all(|c| c.is_ascii_digit())
    }
}

/// Проверяет, что в токене есть хотя бы один не-alphanumeric символ.
///
/// Эквивалентно отрицанию условия `chars().all(is_alphanumeric)`.
#[derive(Debug, Clone)]
pub struct IsNonAlnum;

impl Predicate for IsNonAlnum {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        !token_view
            .token()
            .value
            .chars()
            .all(|c| c.is_alphanumeric())
    }
}

/// Проверка длины токена в байтах.
///
/// Использует `str::len()`, то есть считает байты UTF-8, а не количество `char`.
#[derive(Debug, Clone)]
pub struct LengthEq(pub usize);

impl Predicate for LengthEq {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().value.len() == self.0
    }
}

/// Проверка точного типа токена.
#[derive(Debug, Clone)]
pub struct TokenTypeIs {
    pub token_type: TokenType,
}

impl Predicate for TokenTypeIs {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view.token().token_type == self.token_type
    }
}

/// Словарный предикат по множеству нормализованных слов.
///
/// Токен проходит проверку, если:
/// - он состоит только из букв;
/// - его normalized-форма (или одна из морфоформ) содержится в `values`.
#[derive(Debug, Clone)]
pub struct Dictionary {
    pub values: HashSet<String>,
}

impl Predicate for Dictionary {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        if !token_view.token().value.chars().all(|c| c.is_alphabetic()) {
            return false;
        }

        if let Some(forms) = token_view.forms() {
            return forms.iter().any(|f| self.values.contains(&f.normalized));
        }

        // plain token -> token.normalized (lowercase)
        let v = token_view.token().value.to_lowercase();
        self.values.contains(&v)
    }
}

/// Проверка наличия граммемы в морфологических разборах токена.
#[derive(Debug, Clone)]
pub struct GramPredicate<'a> {
    pub value: &'a str,
}

impl<'a> Predicate for GramPredicate<'a> {
    fn check(&self, token_view: &dyn TokenView) -> bool {
        token_view
            .forms()
            .is_some_and(|forms| forms.iter().any(|f| f.grams.contains(self.value)))
    }
}
