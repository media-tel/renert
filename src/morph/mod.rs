//! Модуль `morph`: морфологический анализ и модели данных.
//!
//! Этот модуль предоставляет слой морфологии поверх `morph-rs`.
//!
//! Содержимое:
//! - [`morph`] — адаптеры анализатора: [`morph::MorphAnalyzer`], [`morph::CachedMorphAnalyzer`]
//! - [`models`] — структуры результата морфологического разбора: [`models::Form`], [`models::Grams`]
//! - [`dict_loader`] — подготовка и поиск словаря (`dict.json` + `dict.fst`)
//!
//! ## Инициализация словаря
//!
//! Рекомендуемый способ — [`crate::init`] (подготовка из XML при необходимости) или
//! [`crate::load`] (только каталог с готовыми `dict.json` + `dict.fst`): один вызов в начале `main`
//! настраивает глобальный морфологический слой, используемый токенизацией и предикатами.
//!
//! ```rust,no_run
//! use renert::error;
//!
//! fn main() -> error::Result<()> {
//!     renert::init("dict.opcorpora.xml", "data/dict")?;
//!     // дальше Parser, morph_pipeline и т.д.
//!     Ok(())
//! }
//! ```
//!
//! Если [`crate::init`] не вызывался, используется fallback
//! через [`dict_loader::dict_dir_default`] (переменная окружения
//! `YARGY_DICT_DIR` или стандартные пути).
//!
//! ## Использование в пайплайне
//!
//! Морфология подключается через [`crate::token::MorphTokenizer`]:
//!
//! ```text
//! text → Tokenizer → MorphTokenizer → Vec<AnyToken> (с forms/grams) → predicates/grammar
//! ```
//!
//! `morph`-модуль **не выбирает “лучший” разбор** и не интерпретирует контекст.
//! Он возвращает все разборы, а выбор делается предикатами грамматики.
//!
//! ## Публичный API
//!
//! - Анализатор: [`morph::MorphAnalyzer::open`], [`morph::MorphAnalyzer::open_at`],
//!   [`morph::MorphAnalyzer::parse`], [`morph::MorphAnalyzer::normalized_set`],
//!   [`morph::MorphAnalyzer::check_gram`]
//! - Кеширующий анализатор: [`morph::CachedMorphAnalyzer::open`],
//!   [`morph::CachedMorphAnalyzer::parse`]
//! - Работа со словарем: [`dict_loader::prepare_dictionary`], [`dict_loader::dict_dir_default`]
//!
//! ## Пример: открыть анализатор и разобрать слово
//!
//! ```rust,no_run
//! use renert::morph::morph::MorphAnalyzer;
//!
//! use renert::error;
//!
//! # fn main() -> error::Result<()> {
//! let m = MorphAnalyzer::open()?;
//!
//! let forms = m.parse("Новгорода");
//! assert!(!forms.is_empty());
//!
//! // Часто первый разбор оказывается практичным рабочим вариантом,
//! // но это не строгий контракт: грамматика должна уметь работать со всеми.
//! let f0 = &forms[0];
//! println!("lemma: {}", f0.normalized);
//! println!("grams: {:?}", f0.grams.values);
//! # Ok(())
//! # }
//! ```
//!
//! ## Пример: получить множество возможных лемм
//!
//! ```rust,no_run
//! use renert::morph::morph::MorphAnalyzer;
//!
//! use renert::error;
//!
//! # fn main() -> error::Result<()> {
//! let m = MorphAnalyzer::open()?;
//! let norms = m.normalized_set("Ленина");
//!
//! // Неоднозначные слова могут иметь несколько нормальных форм.
//! assert!(!norms.is_empty());
//! println!("normalized_set = {:?}", norms);
//! # Ok(())
//! # }
//! ```
//!
//! ## Пример: склонение через `Form::inflect_str`
//!
//! Инфлекция возможна только если `Form.raw` заполнен.
//!
//! ```rust,no_run
//! use renert::morph::morph::CachedMorphAnalyzer;
//!
//! use renert::error;
//!
//! # fn main() -> error::Result<()> {
//! let m = CachedMorphAnalyzer::open()?;
//! let forms = m.parse("улица");
//!
//! let f0 = &forms[0];
//! // Приведём к именительному множественного (nomn+plur)
//! let out = f0.inflect_str(m.analyzer(), &["nomn", "plur"]);
//! println!("inflected = {}", out);
//! # Ok(())
//! # }
//! ```
//!
//! ## Производительность: кеширующий анализатор
//!
//! Для production обычно используется [`morph::CachedMorphAnalyzer`]:
//!
//! ```rust,no_run
//! use renert::morph::morph::CachedMorphAnalyzer;
//!
//! use renert::error;
//!
//! fn main() -> error::Result<()> {
//! let m = CachedMorphAnalyzer::open()?;
//!
//! // При повторных вызовах одинаковых слов будет попадание в LRU-cache.
//! let _ = m.parse("Новгорода");
//! let _ = m.parse("Новгорода");
//! # Ok(())
//! # }
//! ```
//!
//! ---
//!
//! Если словарь не найден, `open()` вернёт ошибку с путями,
//! где ожидались `dict.json` и `dict.fst`.

pub mod dict_loader;
pub mod models;
#[allow(clippy::module_inception)]
pub mod morph;
