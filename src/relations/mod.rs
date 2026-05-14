//! Модуль отношений для ограничения морфологических форм.
//!
//! Содержит граф отношений между элементами и специализированный граф
//! для токенов с морфологическими разборами, а также композиционные
//! конструкторы (`And`, `Or`, `Not`) для построения составных отношений.
//! Конкретные морфологические отношения (род, число, падеж, ГЧП) находятся
//! в [`bank`].
//!
//! ## Логические операции API
//! Для [`Relation`] доступны композиционные функции:
//! - [`and_relation`] — конъюнкция отношений;
//! - [`or_relation`] — дизъюнкция отношений;
//! - [`not_relation`] — отрицание отношения.
//!
//! Они используются как обычные комбинаторы над `Arc<dyn Relation>` и могут
//! вкладываться друг в друга.
//!
//! ## Примеры использования согласования
//! В библиотеке реализовано четыре типа согласований:
//! [`gender_relation`] — согласование по роду,
//! [`number_relation`] — по числу,
//! [`case_relation`] — по падежу,
//! [`gnc_relation`] — по роду, числу и падежу.
//! Метод [`match_relation`](crate::relations::RuleBuilderRelationExt::match_relation) указывает согласование:
//! ```rust
//! use renert::fact;
//! use renert::interpretation::RuleInterpretation;
//! use renert::predicates::gram;
//! use renert::relations::{gnc_relation, RuleBuilderRelationExt};
//! use renert::{pred, rule, Parser, RuleRegistry};
//!
//! fact!(Name => [first, last]);
//!
//! static FORMS: &[&str] = &["nomn", "sing"];
//!
//! let gnc = gnc_relation();
//!
//! let name_rule = rule([
//!     pred(gram("Name"))
//!         .interpretation(Name::first.inflected(FORMS))
//!         .match_relation(gnc.clone()),
//!     pred(gram("Surn"))
//!         .interpretation(Name::last.inflected(FORMS))
//!         .match_relation(gnc),
//! ])
//! .interpretation_fact::<Name>();
//!
//! let mut registry = RuleRegistry::new();
//! let rid = registry.add(name_rule.build(()));
//! registry.validate().expect("registry must be valid");
//!
//! let parser = Parser::new(&registry, rid);
//!
//! if let Some(m) = parser.r#match("Сашу Иванову") {
//!     if let Some(fact) = m.fact(&registry) {
//!         println!("{}", fact);
//!     }
//! }
//! ```
//!
//! Output:
//! ```text
//! Name(
//!     first='Саша',
//!     last='Иванова'
//! )
//! ```
//!
//! Без использования согласования правила не будут, например, учитываться морфологические формы имен и фамилий:
//! ```rust
//! use renert::fact;
//! use renert::interpretation::RuleInterpretation;
//! use renert::predicates::gram;
//! use renert::relations::{gnc_relation, RuleBuilderRelationExt};
//! use renert::{pred, rule, Parser, RuleRegistry};
//!
//! fact!(Name => [first, last]);
//! static FORMS: &[&str] = &["nomn", "sing"];
//!
//! let gnc = gnc_relation();
//!
//! let name_rule = rule([
//!     pred(gram("Name"))
//!         .interpretation(Name::first.inflected(FORMS))
//!         .match_relation(gnc.clone()),
//!     pred(gram("Surn"))
//!         .interpretation(Name::last.inflected(FORMS))
//!         .match_relation(gnc),
//! ])
//! .interpretation_fact::<Name>();
//!
//! let mut registry = RuleRegistry::new();
//! let rid = registry.add(name_rule.build(()));
//! registry.validate().expect("registry must be valid");
//! let parser = Parser::new(&registry, rid);
//! for m in parser.findall("Сашу Иванову, Саше Иванову") {
//!     if let Some(fact) = m.fact(&registry) {
//!         println!("{fact}");
//!     }
//!}
//! ```
//!
//! Output:
//! ```text
//! Name(
//!    first='саша',
//!    last='иванова'
//!)
//!Name(
//!    first='саша',
//!    last='иванов'
//!)
//! ```

pub mod bank;
pub mod builder;
pub mod constructors;
pub mod graph;

#[doc(inline)]
pub use bank::{
    case_relation, gender_relation, gnc_relation, number_relation, CaseRelation, GenderRelation,
    GncRelation, NumberRelation,
};
#[doc(inline)]
pub use builder::RuleBuilderRelationExt;
#[doc(inline)]
pub use constructors::{
    and_relation, not_relation, or_relation, AndRelation, NotRelation, OrRelation,
};
#[doc(inline)]
pub use graph::{Edge, FormsRelationsGraph, Relation, RelationsGraph, TokenRelationsGraph};

#[cfg(test)]
mod tests;
