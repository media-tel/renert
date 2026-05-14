//! Парсер на базе алгоритма Earley.
//!
//! Модуль объединяет низкоуровневые структуры чарта/состояний и
//! высокоуровневый API поиска совпадений.
//!
//! Ключевые сущности:
//! - [`Parser`] — основной вход в API парсинга;
//! - [`Chart`] и [`State`] — внутреннее представление процесса разбора;
//! - [`MatchBorrowed`] / [`MatchOwned`] — результаты разбора.
//!
//! Типичный конвейер:
//! `tokens -> chart -> states -> trees -> matches`.
//!
//! # Что использовать в прикладном коде
//!
//! - Для строки: [`Parser::find`] / [`Parser::findall`] возвращают `Match` (owned).
//! - Только значения токенов как `String` (без дерева): [`Parser::find_text`] /
//!   [`Parser::findall_text`].
//! - Для готовых токенов: [`Parser::parse`] и `find/findall` по срезу токенов
//!   возвращают `MatchBorrowed`.
//! - Для низкого уровня: [`Parser::chart`], [`Parser::matches`], [`Parser::extract`].
//!
//! # Примеры API
//!
//! ## `findall` и `findall_text` (все непересекающиеся совпадения)
//!
//! ```rust
//! use renert::predicates::{and, dictionary, gram, is_title};
//! use renert::{pred, Parser, RuleRegistry};
//!
//! let text = r#"
//!                В Чеченской республике на день рождения ...
//!                Донецкая народная республика провозгласила ...
//!                Башня Федерация — одна из самых высоких ...
//!                "#;
//!
//! let geo_rule = (
//!   pred(and(vec![
//!         gram("ADJF"),
//!         is_title(), // аналог is_capitalized()
//!     ]))
//!     + pred(gram("ADJF")).optional().repeatable()
//!     + pred(dictionary(&["федерация", "республика"]))
//! ).build(());
//!
//! let mut registry = RuleRegistry::new();
//! let geo_id = registry.add(geo_rule);
//!
//! let parser = Parser::new(&registry, geo_id);
//!
//! for m in parser.findall(text) {
//!     let tokens = m.tokens();
//!     let values: Vec<&str> = tokens.iter().map(|t| t.value.as_ref()).collect();
//!     println!("{:?}", values);
//! }
//! // Сразу Vec<String> на каждый матч:
//! for values in parser.findall_text(text) {
//!     println!("{:?}", values);
//! }
//! ```
//! Output:
//! ```text
//! ["Чеченской", "республике"]
//! ["Донецкая", "народная", "республика"]
//! ```
//!
//! ## `find` и `find_text` (первое совпадение в тексте)
//!
//! ```rust
//! use renert::{term, Parser, RuleRegistry};
//!
//! let phrase = "Russian Federation";
//! let mut registry = RuleRegistry::new();
//! let rid = registry.add((term("Russian") + term("Federation")).build(()));
//! let parser = Parser::new(&registry, rid);
//!
//! let m = parser.find(phrase).expect("expected match");
//! // Значения токенов из `Match`:
//! let from_match: Vec<String> = m
//!     .tokens()
//!     .into_iter()
//!     .map(|t| t.value.into_owned())
//!     .collect();
//!
//! // Тот же набор строк через `find_text` (`Option<Vec<String>>`):
//! let from_text = parser.find_text(phrase).expect("expected match");
//! assert_eq!(from_match, from_text);
//! assert_eq!(
//!     from_text,
//!     vec!["Russian".to_string(), "Federation".to_string()]
//! );
//! ```
//! Output: оба пути дают `["Russian", "Federation"]` (как `Vec<String>` внутри `Some` у `find_text`).
//! ```text
//! ["Russian", "Federation"]
//! ```
//!
//! ## `r#match` (разбор всей строки с начала)
//!
//! Успех только если корневое правило покрывает **все** токены входа; лишний хвост даёт `None`
//!
//! ```rust
//! use renert::{term, Parser, RuleRegistry};
//!
//! let mut registry = RuleRegistry::new();
//! let rid = registry.add((term("Russian") + term("Federation")).build(()));
//! let parser = Parser::new(&registry, rid);
//!
//! assert!(parser.r#match("Russian Federation").is_some());
//! assert!(parser.r#match("Russian Federation extra").is_none());
//! ```
//!
//! ## `as_json` и `to_json_value` (извлечённый факт)
//!
//! Если на правиле задана интерпретация факта, из [`Match`] / [`MatchBorrowed`] читают
//! [`ExtractedFact`] через `fact` / `facts` (см. реестр [`RuleRegistry`](crate::RuleRegistry)).
//! У записи одинаковая по смыслу проекция полей в двух формах:
//!
//! - **`as_json`**: [`OrderedFactMap`](crate::interpretation::OrderedFactMap) (по сути
//!   `Vec<(String, FactValue)>`) — как у [`FactRecord::as_json`](crate::interpretation::FactRecord::as_json)
//!   и у [`ExtractedFact::as_json`](crate::parser::ExtractedFact::as_json). Упорядоченные пары
//!   «имя поля + значение» во **внутренних типах** крейта; удобно для логики в Rust (сопоставления,
//!   ветвления по `FactValue`, `Opaque` остаётся отдельным вариантом enum, а не `null`).
//! - **`to_json_value`**: [`serde_json::Value`](https://docs.rs/serde_json/latest/serde_json/enum.Value.html) —
//!   **готовый JSON** для `serde_json::to_string`, HTTP, JSONB: у [`ExtractedFact`]
//!   это объект с полями `name`, `fields`, `spans` (тот же набор полей факта, что в `as_json`, но
//!   уже в JSON, плюс имя и диапазоны). [`FactRecord::to_json_value`](crate::interpretation::FactRecord::to_json_value)
//!   возвращает **только объект полей `fields`**, без `name`/`spans`. Значения
//!   [`FactValue::Opaque`](crate::interpretation::FactValue::Opaque) на wire кодируются как JSON `null`.
//!
//! Поле `spans` у [`ExtractedFact::to_json_value`](crate::parser::ExtractedFact::to_json_value) —
//! это **снимок** диапазонов, зафиксированный при создании [`ExtractedFact`]; при ручном
//! редактировании полей факта он может не совпадать с [`FactRecord::spans`](crate::interpretation::FactRecord::spans).
//!
//! **Пример** (для одного из совпадений, одно поле `noun` — нормализованное слово из словаря; числа
//! `start`/`stop` зависят от входного текста):
//!
//! - Для `as_json` (отладка через `Debug` / ручной разбор `FactValue`):
//!
//! ```text
//! as_json: [("noun", Str("республика"))]
//! ```
//!
//! - Для `to_json_value` (то, что уходит в БД/JSON-API):
//!
//! ```text
//! {
//!   "name": "Geo",
//!   "fields": {
//!     "noun": "республика"
//!   },
//!   "spans": [
//!     { "start": 39, "stop": 59 }
//!   ]
//! }
//! ```
//!
//! Итог: **в слоях с БД и внешним JSON** предпочтительно **`to_json_value`**;  
//! **`as_json`** — нативный путь, когда нужны **типизированные** [`FactValue`](crate::interpretation::FactValue) в коде, без
//! `serde_json::Value`.

mod chart;
mod engine;
mod fact_extract;
mod find_input;
mod item;
mod match_result;
mod tokenizer;

pub use chart::Chart;
pub use engine::Parser;
pub use item::State;
pub use match_result::{
    prepare_match, prepare_matches, prepare_resolved_matches, prepare_trees, ExtractedFact, Match,
    MatchBorrowed, MatchOwned,
};
pub use tokenizer::{ParserTokenizer, PassTagger, Tagger};

#[cfg(test)]
mod tests;
