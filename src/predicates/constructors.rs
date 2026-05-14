use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use crate::morph::models::Form;
use crate::predicates::bank::{
    Caseless, Dictionary, Eq, GramPredicate, Gte, In, InCaseless, IsDigit, IsLower, IsNonAlnum,
    IsTitle, IsUpper, IsWord, LengthEq, Lte, Normalized, TokenTypeIs,
};
use crate::token::{
    global_morph_tokenizer, AnyToken, MorphTagToken, MorphToken, TagToken, Token, TokenType,
};

/// Унифицированный доступ к базовому токену и его морфологическим формам.
///
/// Позволяет применять один и тот же предикат к `Token`, `MorphToken`,
/// `AnyToken`, а также к тегированным вариантам.
pub trait TokenView {
    /// Возвращает базовый токен.
    fn token(&self) -> &Token<'_>;

    /// Возвращает морфологические формы, если они есть.
    ///
    /// Для plain-токенов по умолчанию возвращает `None`.
    fn forms(&self) -> Option<&[Form]> {
        None
    }
}

impl TokenView for Token<'_> {
    fn token(&self) -> &Token<'_> {
        self
    }
}

impl TokenView for MorphToken<'_> {
    fn token(&self) -> &Token<'_> {
        &self.base
    }

    fn forms(&self) -> Option<&[Form]> {
        Some(&self.forms)
    }
}

impl TokenView for AnyToken<'_> {
    fn token(&self) -> &Token<'_> {
        match self {
            AnyToken::Plain(t) => t,
            AnyToken::Morph(mt) => &mt.base,
        }
    }

    fn forms(&self) -> Option<&[Form]> {
        match self {
            AnyToken::Plain(_) => None,
            AnyToken::Morph(mt) => Some(&mt.forms),
        }
    }
}

impl TokenView for TagToken<'_> {
    fn token(&self) -> &Token<'_> {
        &self.base
    }
}

impl TokenView for MorphTagToken<'_> {
    fn token(&self) -> &Token<'_> {
        &self.base
    }

    fn forms(&self) -> Option<&[Form]> {
        Some(&self.forms)
    }
}

/// Предикат над токеном.
///
/// Возвращает `true`, если токен удовлетворяет условию.
pub trait Predicate {
    /// Проверяет предикат на токене.
    fn check(&self, token: &dyn TokenView) -> bool;
}

/// Публичное представление предикатов и их логических композиций.
///
/// Это основной тип, который возвращают функции-конструкторы модуля.
#[derive(Debug, Clone)]
pub enum PredicateKind<'a> {
    Eq(Eq<'a>),
    Gte(Gte),
    Lte(Lte),
    Caseless(Caseless<'a>),
    Normalized(Normalized<'a>),
    In(In<'a>),
    IsTitle(IsTitle),
    IsUpper(IsUpper),
    IsLower(IsLower),
    IsDigit(IsDigit),
    IsNonAlnum(IsNonAlnum),
    IsWord(IsWord),
    LengthEq(LengthEq),
    TokenTypeIs(TokenTypeIs),
    Dictionary(Dictionary),
    InCaseless(InCaseless),
    Gram(GramPredicate<'a>),

    And(Vec<PredicateKind<'a>>),
    Or(Vec<PredicateKind<'a>>),
    Not(Box<PredicateKind<'a>>),
}

impl<'a> PredicateKind<'a> {
    /// Проверяет предикат на токене.
    pub fn check(&self, token: &dyn TokenView) -> bool {
        match self {
            PredicateKind::Eq(p) => p.check(token),
            PredicateKind::Gte(p) => p.check(token),
            PredicateKind::Lte(p) => p.check(token),
            PredicateKind::Caseless(p) => p.check(token),
            PredicateKind::Normalized(p) => p.check(token),
            PredicateKind::In(p) => p.check(token),
            PredicateKind::IsTitle(p) => p.check(token),
            PredicateKind::IsUpper(p) => p.check(token),
            PredicateKind::IsLower(p) => p.check(token),
            PredicateKind::IsDigit(p) => p.check(token),
            PredicateKind::IsWord(p) => p.check(token),
            PredicateKind::IsNonAlnum(p) => p.check(token),
            PredicateKind::LengthEq(p) => p.check(token),
            PredicateKind::TokenTypeIs(p) => p.check(token),
            PredicateKind::Dictionary(p) => p.check(token),
            PredicateKind::InCaseless(p) => p.check(token),
            PredicateKind::Gram(p) => p.check(token),

            PredicateKind::And(list) => list.iter().all(|p| p.check(token)),
            PredicateKind::Or(list) => list.iter().any(|p| p.check(token)),
            PredicateKind::Not(inner) => !inner.check(token),
        }
    }

    /// Возвращает суженное подмножество морфоформ токена после успешного `check`.
    ///
    /// Вызывается **только** когда `check` уже вернул `true`.
    ///
    /// Семантика `None`:
    /// - для предикатов без морфологической семантики: "формы не сужаются";
    /// - для морфологических предикатов: "не найдено подходящих форм".
    ///
    /// На практике `None` можно трактовать как отсутствие дополнительного
    /// фильтра по формам на этом шаге.
    pub fn matched_forms(&self, token: &dyn TokenView) -> Option<Arc<[Form]>> {
        let forms = token.forms()?;
        if forms.is_empty() {
            return None;
        }

        match self {
            PredicateKind::Gram(p) => {
                let narrowed: Vec<Form> = forms
                    .iter()
                    .filter(|f| f.grams.contains(p.value))
                    .cloned()
                    .collect();
                if narrowed.is_empty() {
                    None
                } else {
                    Some(narrowed.into())
                }
            }
            PredicateKind::Normalized(p) => {
                let needle = p.value.to_lowercase();
                let narrowed: Vec<Form> = forms
                    .iter()
                    .filter(|f| f.normalized == needle)
                    .cloned()
                    .collect();
                if narrowed.is_empty() {
                    None
                } else {
                    Some(narrowed.into())
                }
            }
            PredicateKind::Dictionary(p) => {
                let narrowed: Vec<Form> = forms
                    .iter()
                    .filter(|f| p.values.contains(&f.normalized))
                    .cloned()
                    .collect();
                if narrowed.is_empty() {
                    None
                } else {
                    Some(narrowed.into())
                }
            }
            PredicateKind::And(list) => {
                let mut result: Option<Vec<&Form>> = None;
                for child in list {
                    if let Some(child_forms) = child.matched_forms(token) {
                        let child_set: HashSet<usize> = child_forms
                            .iter()
                            .filter_map(|cf| forms.iter().position(|f| f == cf))
                            .collect();
                        result = Some(match result {
                            None => forms
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| child_set.contains(i))
                                .map(|(_, f)| f)
                                .collect(),
                            Some(prev) => prev
                                .into_iter()
                                .filter(|f| {
                                    forms
                                        .iter()
                                        .position(|orig| orig == *f)
                                        .is_some_and(|i| child_set.contains(&i))
                                })
                                .collect(),
                        });
                    }
                }
                let narrowed: Vec<Form> = result?.into_iter().cloned().collect();
                if narrowed.is_empty() {
                    None
                } else {
                    Some(narrowed.into())
                }
            }
            PredicateKind::Or(list) => {
                for child in list {
                    if child.check(token) {
                        return child.matched_forms(token);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Возвращает человекочитаемую подпись предиката.
    ///
    /// Удобно для отладки и диагностических сообщений.
    pub fn label(&self) -> String {
        match self {
            PredicateKind::Eq(p) => format!("eq({})", p.value.as_ref()),
            PredicateKind::Gte(p) => format!("gte({})", p.value),
            PredicateKind::Lte(p) => format!("lte({})", p.value),
            PredicateKind::Caseless(p) => format!("caseless({})", p.value.as_ref()),
            PredicateKind::Normalized(p) => format!("normalized({})", p.value),
            PredicateKind::In(_) => "in_(...)".into(),
            PredicateKind::IsTitle(_) => "is_title()".into(),
            PredicateKind::IsUpper(_) => "is_upper()".into(),
            PredicateKind::IsLower(_) => "is_lower()".into(),
            PredicateKind::IsDigit(_) => "is_digit()".into(),
            PredicateKind::IsNonAlnum(_) => "is_non_alnum()".into(),
            PredicateKind::IsWord(_) => "is_word()".into(),
            PredicateKind::LengthEq(n) => format!("length_eq({})", n.0),
            PredicateKind::TokenTypeIs(t) => format!("token_type_is({:?})", t.token_type),
            PredicateKind::Dictionary(_) => "dictionary(...)".into(),
            PredicateKind::InCaseless(_) => "in_caseless(...)".into(),
            PredicateKind::Gram(p) => format!("gram({})", p.value),

            PredicateKind::And(xs) => {
                let inner = xs.iter().map(|p| p.label()).collect::<Vec<_>>().join(" & ");
                format!("and({})", inner)
            }
            PredicateKind::Or(xs) => {
                let inner = xs.iter().map(|p| p.label()).collect::<Vec<_>>().join(" | ");
                format!("or({})", inner)
            }
            PredicateKind::Not(x) => format!("not({})", x.label()),
        }
    }
}

/// Создает предикат точного вхождения в заданный набор значений.
pub fn in_<'a>(values: &[&'a str]) -> PredicateKind<'a> {
    PredicateKind::In(In {
        values: values.iter().copied().map(Cow::Borrowed).collect(),
    })
}

/// Создает предикат точного сравнения со строкой.
///
/// # Examples
///
/// ```rust
/// use renert::predicates::eq;
/// use renert::span::Span;
/// use renert::token::{Token, TokenType};
///
/// let tok = Token::new("Москва", Span::new(0, 12), TokenType::Russian);
/// assert!(eq("Москва").check(&tok));
/// assert!(!eq("Казань").check(&tok));
/// ```
pub fn eq<'a>(value: &'a str) -> PredicateKind<'a> {
    PredicateKind::Eq(Eq {
        value: Cow::Borrowed(value),
    })
}

/// Создает owned-вариант предиката точного сравнения.
pub fn eq_owned<'a>(value: String) -> PredicateKind<'a> {
    PredicateKind::Eq(Eq {
        value: Cow::Owned(value),
    })
}

/// Создает числовой предикат `>= value`.
pub fn gte<'a>(value: i64) -> PredicateKind<'a> {
    PredicateKind::Gte(Gte { value })
}

/// Создает числовой предикат `<= value`.
pub fn lte<'a>(value: i64) -> PredicateKind<'a> {
    PredicateKind::Lte(Lte { value })
}

/// Создает регистронезависимый предикат сравнения.
pub fn caseless<'a>(value: &'a str) -> PredicateKind<'a> {
    PredicateKind::Caseless(Caseless {
        value: Cow::Borrowed(value),
    })
}

/// Создает owned-вариант регистронезависимого предиката сравнения.
pub fn caseless_owned<'a>(value: String) -> PredicateKind<'a> {
    PredicateKind::Caseless(Caseless {
        value: Cow::Owned(value),
    })
}

/// Создает предикат сравнения с нормализованной формой (леммой).
pub fn normalized<'a>(value: &'a str) -> PredicateKind<'a> {
    PredicateKind::Normalized(Normalized { value })
}

/// Проверяет, что токен в Title Case.
pub fn is_title<'a>() -> PredicateKind<'a> {
    PredicateKind::IsTitle(IsTitle)
}

/// Проверяет, что все символы токена верхнего регистра.
pub fn is_upper<'a>() -> PredicateKind<'a> {
    PredicateKind::IsUpper(IsUpper)
}

/// Проверяет, что все символы токена нижнего регистра.
pub fn is_lower<'a>() -> PredicateKind<'a> {
    PredicateKind::IsLower(IsLower)
}

/// Проверяет, что токен состоит только из ASCII-цифр.
pub fn is_digit<'a>() -> PredicateKind<'a> {
    PredicateKind::IsDigit(IsDigit)
}

/// Проверяет, что токен состоит только из буквенных символов.
pub fn is_word<'a>() -> PredicateKind<'a> {
    PredicateKind::IsWord(IsWord)
}

/// Проверяет, что в токене есть хотя бы один не-alphanumeric символ.
pub fn is_non_alnum<'a>() -> PredicateKind<'a> {
    PredicateKind::IsNonAlnum(IsNonAlnum)
}

/// Проверяет длину токена в байтах.
pub fn length_eq<'a>(n: usize) -> PredicateKind<'a> {
    PredicateKind::LengthEq(LengthEq(n))
}

/// Создает регистронезависимый предикат вхождения в набор значений.
pub fn in_caseless<'a>(values: &[&'a str]) -> PredicateKind<'a> {
    PredicateKind::InCaseless(InCaseless {
        values: values.iter().map(|v| v.to_lowercase()).collect(),
    })
}

/// Проверяет соответствие типа токена.
///
/// Поддерживаемые строковые значения:
/// `Int`, `Russian`, `Email`, `Phone`, `Domain`, `Latin`, `Punct`, `EOL`.
/// Неизвестные значения сопоставляются с [`TokenType::Other`].
///
/// # Examples
///
/// ```rust
/// use renert::predicates::is_token_type;
/// use renert::span::Span;
/// use renert::token::{Token, TokenType};
///
/// let tok = Token::new("42", Span::new(0, 2), TokenType::Int);
/// assert!(is_token_type("Int").check(&tok));
/// assert!(!is_token_type("Russian").check(&tok));
/// ```
pub fn is_token_type<'a>(_type: &'a str) -> PredicateKind<'a> {
    let tt = match _type {
        "Int" => TokenType::Int,
        "Russian" => TokenType::Russian,
        "Email" => TokenType::Email,
        "Phone" => TokenType::Phone,
        "Domain" => TokenType::Domain,
        "Latin" => TokenType::Latin,
        "Punct" => TokenType::Punct,
        "EOL" => TokenType::EOL,
        _ => TokenType::Other,
    };

    PredicateKind::TokenTypeIs(TokenTypeIs { token_type: tt })
}

/// Создает словарный предикат по набору слов.
///
/// Для каждого элемента словаря берутся все нормализованные формы через
/// глобальный морфоанализатор (`global_morph_tokenizer()`), после чего
/// проверка выполняется по леммам токена.
///
/// # Examples
///
/// ```rust,no_run
/// use renert::predicates::dictionary;
/// use renert::token::global_morph_tokenizer;
///
/// let tokens = global_morph_tokenizer().tokenize("улицей");
/// assert!(dictionary(&["улица"]).check(&tokens[0]));
/// ```
pub fn dictionary<'a>(values: &[&'a str]) -> PredicateKind<'a> {
    let mt = global_morph_tokenizer();
    let mut norm_set: HashSet<String> = HashSet::new();

    for &item in values {
        norm_set.extend(mt.morph.normalized_set(item)); // все леммы
    }

    PredicateKind::Dictionary(Dictionary { values: norm_set })
}

/// Создает предикат наличия граммемы в морфологических разборах токена.
///
/// # Examples
///
/// ```rust,no_run
/// use renert::predicates::gram;
/// use renert::token::global_morph_tokenizer;
///
/// let tokens = global_morph_tokenizer().tokenize("стали");
/// assert!(gram("NOUN").check(&tokens[0]));
/// ```
pub fn gram<'a>(g: &'a str) -> PredicateKind<'a> {
    PredicateKind::Gram(GramPredicate { value: g })
}

/// Логическое `AND` для списка предикатов.
///
/// # Examples
///
/// ```rust
/// use renert::predicates::{eq, is_word, and};
/// use renert::span::Span;
/// use renert::token::{Token, TokenType};
///
/// let tok = Token::new("дом", Span::new(0, 6), TokenType::Russian);
/// let p = and(vec![is_word(), eq("дом")]);
/// assert!(p.check(&tok));
/// ```
pub fn and<'a>(preds: Vec<PredicateKind<'a>>) -> PredicateKind<'a> {
    PredicateKind::And(preds)
}

/// Логическое `OR` для списка предикатов.
///
/// # Examples
///
/// ```rust
/// use renert::predicates::{eq, or};
/// use renert::span::Span;
/// use renert::token::{Token, TokenType};
///
/// let tok = Token::new("дом", Span::new(0, 6), TokenType::Russian);
/// let p = or(vec![eq("улица"), eq("дом")]);
/// assert!(p.check(&tok));
/// ```
pub fn or<'a>(preds: Vec<PredicateKind<'a>>) -> PredicateKind<'a> {
    PredicateKind::Or(preds)
}

/// Логическое отрицание предиката.
///
/// # Examples
///
/// ```rust
/// use renert::predicates::{eq, not};
/// use renert::span::Span;
/// use renert::token::{Token, TokenType};
///
/// let tok = Token::new("дом", Span::new(0, 6), TokenType::Russian);
/// assert!(not(eq("улица")).check(&tok));
/// ```
pub fn not<'a>(pred: PredicateKind<'a>) -> PredicateKind<'a> {
    PredicateKind::Not(Box::new(pred))
}

#[cfg(test)]
mod tests {
    use super::*;
    pub use crate::span::*;
    use crate::token::AnyToken;

    fn tokenize_all<'a>(input: &'a str) -> Vec<AnyToken<'a>> {
        let tokenizer = global_morph_tokenizer();
        tokenizer.tokenize(input)
    }

    #[test]
    fn test_eq() {
        let tokens = tokenize_all("ул. Ленина д. 10");
        let predicate = eq("ул");

        let tokens_2 = tokenize_all("Проспект Горького 32А");
        let predicate_2 = eq("32");

        assert!(predicate.check(&tokens[0]));
        assert!(!predicate.check(&tokens[1]));
        assert!(!predicate.check(&tokens[4]));

        assert!(predicate_2.check(&tokens_2[2]));
        assert!(!predicate_2.check(&tokens_2[3]));
    }

    #[test]
    fn test_gte() {
        let tokens = tokenize_all("дом 15");
        let predicate = gte(10);

        assert!(!predicate.check(&tokens[0]));
        assert!(predicate.check(&tokens[1]));
    }

    #[test]
    fn test_lte() {
        let tokens = tokenize_all("7 корпус");
        let predicate = lte(10);
        assert!(predicate.check(&tokens[0]));
        assert!(!predicate.check(&tokens[1]));
    }

    #[test]
    fn test_caseless() {
        let tokens = tokenize_all("МоСкВа");
        let predicate_1 = caseless("москва");
        let predicate_2 = caseless("Петербург");

        assert!(predicate_1.check(&tokens[0]));
        assert!(!predicate_2.check(&tokens[0]));
    }

    #[test]
    fn test_normalized_lemmatization() {
        let tokens = tokenize_all("улица улицу улицей улице");

        let p = normalized("улица");

        assert!(p.check(&tokens[0]));
        assert!(p.check(&tokens[1]));
        assert!(p.check(&tokens[2]));
        assert!(p.check(&tokens[3]));
    }

    #[test]
    fn test_in() {
        // Регистрозависимое вхождение: строка токена должна совпадать с элементом множества.
        let tokens = tokenize_all("москва питер Казань");
        let predicate = in_(&["москва", "питер"]);

        assert!(predicate.check(&tokens[0]));
        assert!(predicate.check(&tokens[1]));
        assert!(!predicate.check(&tokens[2]));

        let predicate = in_(&["S", "M", "L"]);
        let tokens = tokenize_all("S 1");
        assert!(predicate.check(&tokens[0]));
        assert!(!predicate.check(&tokens[1]));
    }

    #[test]
    fn test_in_caseless() {
        let tokens = tokenize_all("Москва МОСКВА москва Казань");
        let predicate = in_caseless(&["москва", "питер"]);

        assert!(predicate.check(&tokens[0]));
        assert!(predicate.check(&tokens[1]));
        assert!(predicate.check(&tokens[2]));
        assert!(!predicate.check(&tokens[3]));
    }

    #[test]
    fn test_is_title() {
        let tokens_1 = tokenize_all("Москва");
        let tokens_2 = tokenize_all("улица");
        let predicate = is_title();

        assert!(predicate.check(&tokens_1[0]));
        assert!(!predicate.check(&tokens_2[0]));
    }

    #[test]
    fn test_is_upper() {
        let tokens_1 = tokenize_all("ТВЕРЬ");
        let tokens_2 = tokenize_all("еда");
        let predicate = is_upper();

        assert!(predicate.check(&tokens_1[0]));
        assert!(!predicate.check(&tokens_2[0]));
    }

    #[test]
    fn test_is_lower() {
        let tokens_1 = tokenize_all("тверь");
        let tokens_2 = tokenize_all("ТВЕРЬ");
        let predicate = is_lower();

        assert!(predicate.check(&tokens_1[0]));
        assert!(!predicate.check(&tokens_2[0]));
    }

    #[test]
    fn test_is_word() {
        let tokens = tokenize_all("улица 2");
        let predicate = is_word();

        assert!(predicate.check(&tokens[0]));
        assert!(!predicate.check(&tokens[1]));
    }

    #[test]
    fn test_is_digit() {
        let tokens = tokenize_all("12345 Маркса");
        let predicate = is_digit();

        assert!(predicate.check(&tokens[0]));
        assert!(!predicate.check(&tokens[1]));
    }

    #[test]
    fn test_is_alnum() {
        let tokens = tokenize_all("Проспект Горького *####**");
        let predicate = is_non_alnum();

        assert!(predicate.check(&tokens[2]));
        assert!(!predicate.check(&tokens[1]));
    }

    #[test]
    fn test_length_eq() {
        let tokens = tokenize_all("дом");
        let predicate = LengthEq(6);
        println!("{:?}", tokens);

        assert!(predicate.check(&tokens[0]));
    }

    #[test]
    fn test_token_type_is() {
        let tokens = tokenize_all("улица 12 ,");

        // ожидаем:
        // "улица" -> Russian
        // "12"    -> Int
        // ","     -> Punct

        let is_ru = is_token_type("Russian");
        let is_int = is_token_type("Int");
        let is_punct = is_token_type("Punct");

        assert!(is_ru.check(&tokens[0]));
        assert!(!is_ru.check(&tokens[1]));

        assert!(is_int.check(&tokens[1]));
        assert!(!is_int.check(&tokens[0]));

        assert!(is_punct.check(&tokens[2]));
    }

    #[test]
    fn test_dictionary_caseless_basic() {
        let tokens = tokenize_all("Москва МОСКВА москва Казань");

        let predicate = dictionary(&["москва", "питер"]);

        assert!(predicate.check(&tokens[0]));
        assert!(predicate.check(&tokens[1]));
        assert!(predicate.check(&tokens[2]));
        assert!(!predicate.check(&tokens[3]));
    }

    #[test]
    fn test_dictionary_check_punct() {
        let tokens = tokenize_all(",");

        let predicate = dictionary(&[".", ",", "!"]);

        assert!(!predicate.check(&tokens[0])); // токен не является словом, поэтому не проходит проверку
    }

    #[test]
    fn test_dictionary_with_other_predicates() {
        let token_1 = tokenize_all("Москва");
        let city_1 = and(vec![
            is_word(),
            is_title(),
            dictionary(&["москва", "казань"]),
        ]);

        assert!(city_1.check(&token_1[0]));

        let token_2 = tokenize_all("г. Москва");
        let city_2 = and(vec![is_title(), dictionary(&["СПБ", "МОСКВА", "Москва"])]);

        assert!(city_2.check(&token_2[2])); // учитывается регистр
    }

    #[test]
    fn test_dictionary_lemmatization() {
        let tokens = tokenize_all("улицу улицей улице улица");
        let d = dictionary(&["улица"]);

        assert!(d.check(&tokens[0]));
        assert!(d.check(&tokens[1]));
        assert!(d.check(&tokens[2]));
        assert!(d.check(&tokens[3]));
    }

    #[test]
    fn test_gram() {
        let tokens = tokenize_all("стали");
        let predicate_noun = gram("NOUN");
        let predicate_verb = gram("VERB");

        let t0 = &tokens[0];
        assert!(matches!(t0, AnyToken::Morph(_)));

        assert!(predicate_noun.check(&tokens[0]));
        assert!(predicate_verb.check(&tokens[0]));
    }

    #[test]
    fn test_gram_i_is_not_verb() {
        let tokens = tokenize_all("и");

        let conj = gram("CONJ");
        let verb = gram("VERB");

        assert!(conj.check(&tokens[0])); // союз должен быть
        assert!(!verb.check(&tokens[0])); // глагол — нет
    }

    #[test]
    fn test_tagtoken_basic_predicates_work() {
        let base = Token {
            value: "Москва".into(),
            span: Span::new(0, 10),
            token_type: TokenType::Russian,
        };

        let tagged = base.tagged("CITY".to_string());

        assert!(eq("Москва").check(&tagged));
        assert!(caseless("москва").check(&tagged));
        assert!(is_title().check(&tagged));
        assert!(is_word().check(&tagged));
        assert!(is_token_type("Russian").check(&tagged));

        // Морфо-предикаты на TagToken:
        // - gram() -> false (forms отсутствуют)
        // - normalized() -> fallback на lowercase сравнение value
        assert!(!gram("NOUN").check(&tagged));
        assert!(normalized("москва").check(&tagged));
    }

    #[test]
    fn test_morphtagtoken_supports_gram_and_normalized() {
        // Морфология нужна, поэтому берём MorphTokenizer и достаём MorphToken,
        // а затем добавляем тег -> MorphTagToken.
        let tokens = tokenize_all("улицу");

        let morph = match &tokens[0] {
            AnyToken::Morph(m) => m.clone(),
            _ => panic!("Expected morph token for 'улицу'"),
        };

        let tagged = morph.tagged("OBJ".to_string()); // MorphTagToken

        // normalized("улица") должно матчить "улицу" через лемму
        assert!(normalized("улица").check(&tagged));

        // И граммема NOUN обычно присутствует у "улицу"
        assert!(gram("NOUN").check(&tagged));
        // А VERB быть не должен (в нормальном словаре)
        assert!(!gram("VERB").check(&tagged));
    }

    #[test]
    fn test_morphtagtoken_ambiguous_word_has_multiple_grams() {
        // "стали" омонимично -> NOUN и VERB true
        let tokens = tokenize_all("стали");

        let morph = match &tokens[0] {
            AnyToken::Morph(m) => m.clone(),
            _ => panic!("Expected morph token for 'стали'"),
        };

        let tagged = morph.tagged("AMB".to_string());

        assert!(gram("NOUN").check(&tagged));
        assert!(gram("VERB").check(&tagged));
    }
}
