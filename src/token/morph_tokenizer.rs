//! Морфологический токенизатор (Tokenizer + MorphAnalyzer).
//!
//! Этот модуль объединяет:
//! - [`Tokenizer`] — базовую regex-токенизацию
//! - морфологический анализатор (`MorphAnalyzer` или `CachedMorphAnalyzer`)
//!
//! Результат работы — поток [`AnyToken`], где:
//! - русские слова (`TokenType::Russian`) автоматически превращаются в `MorphToken`
//! - все остальные токены остаются обычными `Token`
//!
//! ## Cached vs Uncached
//!
//! - [`CachedMorphAnalyzer`] — рекомендуемый вариант для production
//! - [`MorphAnalyzer`] — полезен для тестов и отладки

use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::error::MorphError;
use crate::error::TokenizerError;
use crate::morph::models::Form;
use crate::morph::morph::{CachedMorphAnalyzer, MorphAnalyzer};
use crate::token::tokenizer::AnyToken;
use crate::token::{TokenType, Tokenizer};

/// Унифицированный enum для морфологического анализатора.
///
/// Позволяет прозрачно работать:
/// - с обычным [`MorphAnalyzer`]
/// - с кеширующим [`CachedMorphAnalyzer`]
///
/// Используется внутри [`MorphTokenizer`], чтобы не дублировать код.
pub enum Analyser {
    /// Анализатор без кеша.
    MorphAnalyzer(MorphAnalyzer),
    /// Анализатор с LRU-кешем.
    CachedMorphAnalyzer(CachedMorphAnalyzer),
}

impl Analyser {
    /// Подготовка слова к морфологическому анализу.
    ///
    /// Логика:
    /// - если в слове есть заглавные буквы → приводим к lowercase (аллоцируем `String`)
    /// - иначе используем исходный `&str` без аллокаций
    ///
    /// Это уменьшает количество лишних аллокаций и совпадает с типичной
    /// логикой морфоанализаторов, которые ожидают lowercase.
    fn prepare_word(word: &str) -> Cow<'_, str> {
        if word.chars().any(|c| c.is_uppercase()) {
            Cow::Owned(word.to_lowercase())
        } else {
            Cow::Borrowed(word)
        }
    }

    /// Выполняет морфологический разбор слова.
    ///
    /// Возвращает список [`Form`] — вариантов разбора.
    /// Порядок важен: первый элемент обычно считается наиболее вероятным.
    pub fn parse(&self, word: &str) -> Vec<Form> {
        let w = Self::prepare_word(word);
        match self {
            Analyser::MorphAnalyzer(m) => m.parse(&w),
            Analyser::CachedMorphAnalyzer(m) => m.call(&w),
        }
    }

    /// Возвращает множество нормализованных форм слова.
    ///
    /// Используется, когда нужно не один “лучший” вариант,
    /// а все возможные леммы (например, для сопоставления).
    pub fn normalized_set(&self, word: &str) -> std::collections::BTreeSet<String> {
        let w = Self::prepare_word(word);
        match self {
            Analyser::MorphAnalyzer(m) => m.normalized_set(&w),
            Analyser::CachedMorphAnalyzer(m) => m.normalized(&w),
        }
    }

    /// Проверяет, поддерживается ли граммема анализатором.
    ///
    /// Обычно используется при инициализации правил или фильтров,
    /// чтобы упасть раньше, а не во время разбора.
    pub fn check_gram(&self, gram: &str) -> Result<(), MorphError> {
        match self {
            Analyser::MorphAnalyzer(m) => m.check_gram(gram),
            Analyser::CachedMorphAnalyzer(m) => m.check_gram(gram),
        }
    }
}

/// Токенизатор с морфологическим анализом.
///
/// Обёртка над [`Tokenizer`], которая:
/// - сначала токенизирует текст
/// - затем автоматически применяет морфологию ко всем `TokenType::Russian`
///
/// Реализует [`Deref`] к [`Tokenizer`], поэтому можно использовать базовые
/// методы токенизатора вроде [`Tokenizer::split`] вместе с морфологическим API.
pub struct MorphTokenizer {
    /// Базовый regex-токенизатор.
    pub tokenizer: Tokenizer,
    /// Морфологический анализатор.
    pub morph: Analyser,
}

impl Deref for MorphTokenizer {
    type Target = Tokenizer;
    fn deref(&self) -> &Self::Target {
        &self.tokenizer
    }
}

impl MorphTokenizer {
    /// DI-конструктор.
    ///
    /// Позволяет явно передать зависимости:
    /// - готовый [`Tokenizer`]
    /// - любой вариант [`Analyser`]
    ///
    /// Удобно для тестов и кастомных конфигураций.
    pub fn new(tokenizer: Tokenizer, morph: Analyser) -> Self {
        Self { tokenizer, morph }
    }

    /// Внутренний helper для `open()` / `open_uncached()`.
    fn open_with(analyser: Analyser) -> Result<Self, TokenizerError> {
        Ok(Self::new(Tokenizer::new(), analyser))
    }

    /// Открывает `MorphTokenizer` с кеширующим морфоанализатором
    /// из указанного каталога.
    pub fn open_at(dir: impl AsRef<Path>) -> Result<Self, TokenizerError> {
        Self::open_with(Analyser::CachedMorphAnalyzer(CachedMorphAnalyzer::open_at(
            dir,
        )?))
    }

    /// Открывает `MorphTokenizer` с кеширующим морфоанализатором.
    ///
    /// Рекомендуемый вариант по умолчанию.
    pub fn open() -> Result<Self, TokenizerError> {
        Self::open_with(Analyser::CachedMorphAnalyzer(CachedMorphAnalyzer::open()?))
    }

    /// Открывает `MorphTokenizer` без кеша.
    ///
    /// Полезно для:
    /// - тестов
    /// - отладки
    /// - сравнения производительности
    pub fn open_uncached() -> Result<Self, TokenizerError> {
        Self::open_with(Analyser::MorphAnalyzer(MorphAnalyzer::open()?))
    }

    /// инициализация с опциональными параметрами.
    ///
    /// - если `tokenizer` не задан -> используется `Tokenizer::new()`
    /// - если `morph` не задан -> используется кеширующий анализатор
    pub fn with_optional(
        tokenizer: Option<Tokenizer>,
        morph: Option<Analyser>,
    ) -> Result<Self, TokenizerError> {
        let tokenizer = tokenizer.unwrap_or_default();
        let morph = match morph {
            Some(m) => m,
            None => Analyser::CachedMorphAnalyzer(CachedMorphAnalyzer::open()?),
        };
        Ok(Self::new(tokenizer, morph))
    }

    /// Токенизирует текст с морфологией.
    ///
    /// Алгоритм:
    /// 1. вызывается [`Tokenizer::tokenize`]
    /// 2. каждый токен проверяется:
    ///    - `TokenType::Russian` → морфологический разбор → `AnyToken::Morph`
    ///    - остальные → `AnyToken::Plain`
    ///
    /// Возвращается вектор [`AnyToken`], удобный для дальнейшего пайплайна.
    ///
    /// # Пример
    ///
    /// ```rust,no_run
    /// use renert::token::MorphTokenizer;
    /// use renert::token::AnyToken;
    /// use renert::token::TokenType;
    ///
    /// use renert::error;
    /// fn main() -> error::Result<()> {
    /// let mt = MorphTokenizer::open()?;
    ///
    /// let tokens = mt.tokenize("ул. Ленина 10");
    ///
    /// assert!(matches!(tokens[0], AnyToken::Morph(_))); // "ул"
    /// assert!(matches!(tokens[1], AnyToken::Plain(_))); // "."
    /// assert!(matches!(tokens[2], AnyToken::Morph(_))); // "Ленина"
    ///
    /// assert_eq!(tokens[2].token_type(), TokenType::Russian);
    /// let n = tokens[2].normalized();
    /// assert!(n == "ленин" || n == "ленина");
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn tokenize<'a>(&self, text: &'a str) -> Vec<AnyToken<'a>> {
        let tokens = self.tokenizer.tokenize(text);

        tokens
            .into_iter()
            .map(|t| {
                if t.token_type == TokenType::Russian {
                    let forms = self.morph.parse(t.value.as_ref());
                    AnyToken::Morph(t.morphed(forms))
                } else {
                    AnyToken::Plain(t)
                }
            })
            .collect()
    }
}

/// Глобальный MorphTokenizer, заданный через [`crate::init`].
static MORPH_OVERRIDE: OnceLock<Arc<MorphTokenizer>> = OnceLock::new();

/// Fallback: стандартный MorphTokenizer (dict_dir_default), если `init` не вызывали.
static MORPH_DEFAULT: Lazy<Arc<MorphTokenizer>> = Lazy::new(|| {
    Arc::new(MorphTokenizer::open().unwrap_or_else(|e| {
        panic!(
            "Failed to open MorphTokenizer: {e}\n\
             Hint: call renert::init(xml, cache_dir) or renert::load(dict_dir) before using the parser, \
             or set the YARGY_DICT_DIR environment variable to a directory \
             containing dict.json and dict.fst."
        )
    }))
});

/// Возвращает глобальный [`MorphTokenizer`].
///
/// Если ранее был вызван [`crate::init`], используется словарь оттуда.
/// Иначе — fallback через [`dict_dir_default`](crate::morph::dict_loader::dict_dir_default).
pub fn global_morph_tokenizer() -> Arc<MorphTokenizer> {
    MORPH_OVERRIDE
        .get()
        .cloned()
        .unwrap_or_else(|| MORPH_DEFAULT.clone())
}

/// Устанавливает глобальный [`MorphTokenizer`].
///
/// Вызывается из [`crate::init`] или [`crate::load`]. Повторный вызов возвращает `Err`.
pub(crate) fn set_global_morph_tokenizer(mt: MorphTokenizer) -> Result<(), TokenizerError> {
    MORPH_OVERRIDE
        .set(Arc::new(mt))
        .map_err(|_| TokenizerError::AlreadyInitialized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;
    use crate::token::global_morph_tokenizer;

    #[test]
    fn test_morph_tokenizer_newline() {
        let mt = MorphTokenizer::open().unwrap();

        let tokens_1 = mt.tokenize("\n");
        assert_eq!(tokens_1.len(), 1);

        assert_eq!(tokens_1[0].value(), "\n");
        assert_eq!(tokens_1[0].token_type(), TokenType::EOL);

        let tokens_2 = mt.tokenize("\n\n\r");
        assert_eq!(tokens_2.len(), 1);

        assert_eq!(tokens_2[0].value(), "\n\n\r");
        assert_eq!(tokens_2[0].token_type(), TokenType::EOL);
    }

    #[test]
    fn test_morph_tokenizer_punctuation_is_plain() {
        let mt = MorphTokenizer::open().unwrap();

        let tokens = mt.tokenize("дом,улица");

        assert!(matches!(tokens[0], AnyToken::Morph(_))); // "дом"
        assert!(matches!(tokens[1], AnyToken::Plain(_))); // ","
        assert_eq!(tokens[1].token_type(), TokenType::Punct);
        assert!(matches!(tokens[2], AnyToken::Morph(_))); // "улица"
    }

    #[test]
    fn test_morph_tokenizer_values_match_tokenizer() {
        let base = Tokenizer::new();
        let mt = MorphTokenizer::open().unwrap();

        let text = "дом\nулица, 10";

        let base_tokens = base.tokenize(text);
        let base_vals: Vec<&str> = base_tokens.iter().map(|t| t.value.as_ref()).collect();

        let mt_tokens = mt.tokenize(text);
        let mt_vals: Vec<&str> = mt_tokens.iter().map(|t| t.value()).collect();

        assert_eq!(base_vals, mt_vals);
    }

    #[test]
    fn test_morph_tokenizer_only_russian_is_morphed() {
        let mt = global_morph_tokenizer();

        let tokens = mt.tokenize("A-12, Б");

        assert!(matches!(tokens[0], AnyToken::Plain(_))); // A
        assert!(matches!(tokens[1], AnyToken::Plain(_))); // -
        assert!(matches!(tokens[2], AnyToken::Plain(_))); // 12
        assert!(matches!(tokens[3], AnyToken::Plain(_))); // ,
        assert!(matches!(tokens[4], AnyToken::Morph(_))); // Б
    }

    #[test]
    fn test_morph_tokenizer_normalized_accessor() {
        let mt = global_morph_tokenizer();
        let tokens = mt.tokenize("LoNdon ДОМА");

        assert_eq!(tokens[0].normalized().as_ref(), "london");

        // Для русского токена: normalized должен быть согласован со словарём
        let n = tokens[1].normalized().into_owned();
        let set = mt.morph.normalized_set("ДОМА");
        assert!(
            set.contains(&n),
            "Expected normalized() to be in normalized_set(). normalized={n:?}, set={set:?}"
        );
    }

    #[test]
    fn test_morph_tokenizer_with_optional_none_none_equals_open_behavior() {
        let mt = MorphTokenizer::with_optional(None, None).unwrap();
        let tokens = mt.tokenize("дом");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], AnyToken::Morph(_)));
    }

    #[test]
    fn test_morph_tokenizer_with_optional_supplied_tokenizer() {
        let mt = MorphTokenizer::with_optional(Some(Tokenizer::new()), None).unwrap();
        let tokens = mt.tokenize("дом,улица");

        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], AnyToken::Morph(_)));
        assert!(matches!(tokens[1], AnyToken::Plain(_)));
        assert!(matches!(tokens[2], AnyToken::Morph(_)));
    }

    #[test]
    fn test_morph_tokenizer_open_uncached_works() {
        let mt = MorphTokenizer::open_uncached().unwrap();
        let tokens = mt.tokenize("дом");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], AnyToken::Morph(_)));
    }

    #[test]
    fn test_morph_like_yargy() {
        let tokenizer = MorphTokenizer::open().unwrap();
        let tokens = tokenizer.tokenize("dvd-диски");

        assert_eq!(tokens.len(), 3);

        assert_eq!(tokens[0].value(), "dvd");
        assert_eq!(tokens[0].span(), Span::new(0, 3));
        assert_eq!(tokens[0].token_type(), TokenType::Latin);

        assert_eq!(tokens[1].value(), "-");
        assert_eq!(tokens[1].span(), Span::new(3, 4));
        assert_eq!(tokens[1].token_type(), TokenType::Punct);

        assert_eq!(tokens[2].value(), "диски");
        // В Rust Span в байтах: "диски" = 10 байт, значит stop = 4 + 10 = 14
        assert_eq!(tokens[2].span(), Span::new(4, 14));
        assert_eq!(tokens[2].token_type(), TokenType::Russian);

        let mt = match &tokens[2] {
            AnyToken::Morph(mt) => mt,
            _ => panic!("Expected Morph token for 'диски'"),
        };

        assert!(
            !mt.forms.is_empty(),
            "Expected non-empty forms from dict for 'диски'"
        );

        // Как в yargy: среди разборов должна встречаться нормальная форма "диск"
        assert!(
            mt.forms.iter().any(|f| f.normalized == "диск"),
            "Expected at least one form normalized == 'диск'"
        );
    }

    #[test]
    fn test_morph_tokenizer_puts_opencorpora_grams_into_forms() {
        use crate::token::tokenizer::AnyToken;
        use crate::token::TokenType;

        let mt = global_morph_tokenizer();

        // - "стали" (Russian) должно стать Morph и иметь граммемы (NOUN и VERB у разных разборов)
        // - "," (Punct) должен остаться Plain
        // - "dvd" (Latin) должен остаться Plain
        let tokens = mt.tokenize("стали, dvd");
        assert_eq!(tokens.len(), 3);

        // 1) "стали" => Russian => MorphToken => forms заполнены граммемами
        let t0 = &tokens[0];
        assert_eq!(t0.value(), "стали");
        assert_eq!(t0.token_type(), TokenType::Russian);

        let mt0 = match t0 {
            AnyToken::Morph(m) => m,
            _ => panic!("Expected AnyToken::Morph for Russian word 'стали'"),
        };

        assert!(
            !mt0.forms.is_empty(),
            "Expected non-empty forms for 'стали' from morph analyzer"
        );

        // 2) Проверяем, что граммемы реально попали в Form.grams.values как строки
        // и что среди разборов встречаются "NOUN" и "VERB" (омонимия 'стали').
        let has_noun = mt0.forms.iter().any(|f| f.grams.contains("NOUN"));
        let has_verb = mt0.forms.iter().any(|f| f.grams.contains("VERB"));

        assert!(
            has_noun,
            "Expected at least one parse of 'стали' to contain grammeme 'NOUN' in grams.values"
        );
        assert!(
            has_verb,
            "Expected at least one parse of 'стали' to contain grammeme 'VERB' in grams.values"
        );

        // 3) "," => Punct => Plain (не морфится)
        let t1 = &tokens[1];
        assert_eq!(t1.value(), ",");
        assert_eq!(t1.token_type(), TokenType::Punct);
        assert!(
            matches!(t1, AnyToken::Plain(_)),
            "Expected punctuation to remain AnyToken::Plain"
        );

        // 4) "dvd" => Latin => Plain (не морфится)
        let t2 = &tokens[2];
        assert_eq!(t2.value(), "dvd");
        assert_eq!(t2.token_type(), TokenType::Latin);
        assert!(
            matches!(t2, AnyToken::Plain(_)),
            "Expected Latin token to remain AnyToken::Plain"
        );
    }

    #[test]
    fn test_email_and_phone_rules_match_as_single_tokens() {
        let tokenizer = global_morph_tokenizer();
        let tokens = tokenizer
            .tokenize("me@test.com or call +7 (999) 123-45-67 89991234567 +7-999-123-45-67");

        assert!(tokens
            .iter()
            .any(|t| t.value() == "me@test.com" && t.token_type() == TokenType::Email));
        assert!(tokens
            .iter()
            .any(|t| t.value() == "+7 (999) 123-45-67" && t.token_type() == TokenType::Phone));
        assert!(tokens
            .iter()
            .any(|t| t.value() == "89991234567" && t.token_type() == TokenType::Phone));
        assert!(tokens
            .iter()
            .any(|t| t.value() == "+7-999-123-45-67" && t.token_type() == TokenType::Phone));
    }
}
