//! Предикаты терминалов и их композиция.
//!
//! ## Логические операции API
//! Комбинаторы `and`, `or`, `not` и `eq` реэкспортируются с **корня** крейта
//! (`renert::and`, …). Остальные конструкторы предикатов — из этого модуля, например
//! `use renert::predicates::{gram, dictionary, is_title};`.
//!
//! Эти же комбинаторы также доступны как `renert::predicates::{and, or, not, eq}`,
//! но для единообразия публичного API рекомендуется импортировать их с корня.
//!
//! Подмодуль [`constructors`] по-прежнему содержит полную реализацию; реэкспорты ниже —
//! короткий фасад над ним.
//!
//! ## Когда импортировать с корня, а когда из `predicates`
//! - С корня (`renert::{and, or, not, eq}`): когда комбинируете правила и предикаты в одном месте.
//! - Из `predicates`: когда собираете набор только терминальных предикатов.
//!
//! Пример:
//! ```rust
//! use renert::predicates::{dictionary, gram, is_title};
//! use renert::{and, or};
//!
//! let city = and(vec![gram("NOUN"), is_title(), dictionary(&["москва", "казань"])]);
//! let alt = or(vec![gram("NOUN"), gram("ADJF")]);
//! ```

pub mod bank;
pub mod constructors;

#[doc(inline)]
pub use constructors::{
    and, caseless, caseless_owned, dictionary, eq, eq_owned, gram, gte, in_, in_caseless, is_digit,
    is_lower, is_non_alnum, is_title, is_token_type, is_upper, is_word, length_eq, lte, normalized,
    not, or,
};
