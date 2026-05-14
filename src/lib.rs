//! парсер, морфология и интерпретации.
//!
//! Разбор текста по правилам (алгоритм Earley в [`parser`]), словарь и морфология morph-rs
//! ([`morph`], [`token`]), построение грамматик ([`mod@rule`], [`predicates`]), согласование и отношения
//! ([`relations`]), извлечение структурированных фактов ([`interpretation`]), газеттиры ([`pipeline`]),
//! деревья разбора ([`tree`]). Ошибки публичного API — [`error`].
//!
//! Перед первым использованием [`Parser`], [`pipeline::morph_pipeline`] или [`token::MorphTokenizer`]
//! вызовите [`init`] или [`load`] (см. раздел **Быстрый старт**).
//!
//! # Быстрый старт
//!
//! 1. Подготовьте каталог со словарём: [`init`] (XML → кэш) или [`load`] (уже есть `dict.json` + `dict.fst`).
//! 2. Соберите [`RuleRegistry`] и [`Parser`], разбирайте текст (`find`, `findall`, `r#match` и т.д.).
//! 3. При необходимости добавьте интерпретации и читайте факты из матчей (см. [`interpretation`]).
//!
//! Примеры грамматик и дат — в rustdoc модуля [`interpretation`]; сопоставление с Python API — ниже.
//!
//! # Основные модули
//!
//! - [`error`] — единый тип [`error::Error`] и алиас [`error::Result`].
//! - [`interpretation`] — атрибуты, нормализаторы, факты, интерпретаторы.
//! - [`morph`] — загрузка словаря и морфологический анализ.
//! - [`parser`] — [`Parser`] и чарт Earley.
//! - [`pipeline`] — газеттиры (готовые словари фраз).
//! - [`predicates`] — предикаты для терминов (`eq`, `gram`, …).
//! - [`relations`] — согласование форм между токенами.
//! - [`mod@rule`] — правила, продукции, реестр [`RuleRegistry`].
//! - [`span`] — символьные диапазоны.
//! - [`token`] — токены и токенизаторы.
//! - [`tree`] — дерево разбора и нормализация / отношения на дереве.
//!
//! # Соответствие Python [yargy](https://github.com/natasha/yargy)
//!
//! В Python `and_`, `or_`, `not_` перегружены по типам аргументов. В Rust уровни разделены
//! именами: дизъюнкция **правил** — [`or_`], дизъюнкция **предикатов** — [`or`], для
//! **отношений** — [`or_relation`] (и аналогично [`and`] / [`and_relation`], [`not`] /
//! [`not_relation`]).
//!
//! | Python `api.py` | С корня `renert` |
//! |-----------------|----------------------|
//! | `rule(...)` | [`rule()`] |
//! | `empty` | [`empty`] (алиас [`eps`](rule::builder::eps)) |
//! | `forward` | [`forward`] |
//! | `and_` (предикаты) | [`and`] |
//! | `and_` (отношения) | [`and_relation`] |
//! | `or_` (предикаты) | [`or`] |
//! | `or_` (отношения) | [`or_relation`] |
//! | `or_` (правила) | [`or_`] |
//! | `not_` (предикат) | [`not`] |
//! | `not_` (отношение) | [`not_relation`] |
//!
//! Литералы в продукциях: [`term`] или [`pred`]`(`[`eq`]`("…"))` вместо скрытого
//! `prepare_production_item` из Python.
//!
//! ```rust
//! use renert::{and, empty, eq, forward, not, or, or_, pred, rule, term};
//!
//! let _ = rule([term("a"), pred(eq("b"))]);
//! let _ = or_([rule([term("x")]), rule([term("y")])]);
//! let _ = and(vec![eq("a"), or(vec![eq("b"), eq("c")])]);
//! let _ = not(eq("z"));
//! let _ = empty();
//! let _ = forward();
//! ```
//!
//! Минимальный набор имён с корня:
//!
//! ```rust
//! use renert::{eq, not, or, or_, rule, term};
//!
//! let _ = rule([term("a")]);
//! let _ = or_([rule([term("b")]), rule([term("c")])]);
//! let _ = not(eq("x"));
//! let _ = or(vec![eq("y"), eq("z")]);
//! ```

use std::path::Path;

pub mod error;
pub(crate) mod internal;
pub mod interpretation;
pub mod morph;
pub mod parser;
pub mod pipeline;
pub mod predicates;
pub mod relations;
pub mod rule;
pub mod span;
pub mod token;
pub mod tree;

pub use parser::Parser;
pub use predicates::constructors::{and, eq, not, or};
pub use relations::{and_relation, not_relation, or_relation};
pub use rule::builder::eps as empty;
pub use rule::builder::{forward, main_term, or_, pred, rule, term, RuleBuilder, RuleId};
pub use rule::registry::RuleRegistry;

/// Инициализирует RENERT: подготавливает словарь и настраивает глобальный морфоанализатор.
///
/// Если словарь уже собран и лежит на диске, можно вызвать только [`load`] без XML.
///
/// Если `cache_dir` уже содержит `dict.json` и `dict.fst`, сборка из XML пропускается.
/// Иначе словарь собирается из `xml` в `cache_dir` (первый запуск может быть долгим, не более 10 минут).
///
/// Если вы хотите собрать словарь заново, то можете удалить файлы словаря в cache_dir.
///
/// Инициализация должна быть вызвана **до** первого обращения к [`parser::Parser`],
/// [`pipeline::morph_pipeline`] или [`token::MorphTokenizer`].
/// Повторный вызов возвращает ошибку.
///
/// # Ошибки
///
/// Возвращает [`error::Error`]: сбой подготовки словаря ([`error::DictError`]), открытия морфологии
/// ([`error::MorphError`]) или повторной инициализации ([`error::TokenizerError::AlreadyInitialized`]).
///
/// # Пример
///
/// ```rust,no_run
/// use renert::error;
///
/// fn main() -> error::Result<()> {
///     renert::init("dict.opcorpora.xml", "data/dict")?;
///     // дальше Parser, morph_pipeline и т.д.
///     Ok(())
/// }
/// ```
///
/// Для локальной сборки словаря при разработке можно использовать бинарь `add_dictionary`:
///
/// ```text
/// cargo run --bin add_dictionary -- dict.opcorpora.xml --out data/dict
/// ```
///
/// Если `--out` не указан, по умолчанию используется `data/dict`.
pub fn init(xml: impl AsRef<Path>, cache_dir: impl AsRef<Path>) -> error::Result<()> {
    let cache_dir = cache_dir.as_ref();
    morph::dict_loader::prepare_dictionary(&xml, cache_dir)?;
    let mt = token::MorphTokenizer::open_at(cache_dir)?;
    token::morph_tokenizer::set_global_morph_tokenizer(mt)?;
    Ok(())
}

/// Загружает RENERT из каталога с уже собранным словарём morph-rs (`dict.json` + `dict.fst`).
///
/// В отличие от [`init`], **не** читает XML и **не** запускает сборку словаря — только открывает
/// готовые файлы. Удобно на машинах с ограниченной RAM, если бинарники собраны заранее
/// (например, `cargo run --bin add_dictionary -- …` на другом компьютере).
///
/// Если в `dict_dir` нет обоих файлов, возвращается [`error::DictError::DictDirIncomplete`].
///
/// Повторный вызов (после успешного [`init`] или [`load`]) возвращает ошибку, как у [`init`].
///
/// # Ошибки
///
/// [`error::DictError::DictDirIncomplete`], если в каталоге нет обоих файлов словаря; иначе ошибки
/// морфологии или [`error::TokenizerError::AlreadyInitialized`] при повторном вызове.
///
/// # Пример
///
/// ```rust,no_run
/// use renert::error;
///
/// fn main() -> error::Result<()> {
///     renert::load("/path/to/dir/with/dict_json_and_fst")?;
///     Ok(())
/// }
/// ```
pub fn load(dict_dir: impl AsRef<Path>) -> error::Result<()> {
    let dict_dir = dict_dir.as_ref();
    if !morph::dict_loader::has_dict_files(dict_dir) {
        return Err(error::DictError::DictDirIncomplete {
            path: dict_dir.to_path_buf(),
        }
        .into());
    }
    let mt = token::MorphTokenizer::open_at(dict_dir)?;
    token::morph_tokenizer::set_global_morph_tokenizer(mt)?;
    Ok(())
}
