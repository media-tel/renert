//! Подсистема интерпретации: атрибуты, нормализаторы, факты и рантайм-интерпретаторы.
//!
//! Этот модуль объединяет API, необходимый для перехода от синтаксического
//! совпадения правила к структурированному факту (в стиле `yargy.interpretation`):
//! - описание атрибутов факта (`attribute`, `Attribute`, repeatable/default);
//! - нормализация значений (`normalized`, `inflected`, `custom`, `const`);
//! - описание и хранение фактов (`fact`, `FactScheme`, `FactRecord`);
//! - рантайм-исполнение интерпретаций (`AnyInterpretator`, `InterpretatorInput`, ...).
//!
//! # Факты: зачем и как кратко
//!
//! **Зачем:** помимо факта «совпало/не совпало», вы получаете именованную структуру полей —
//! [`FactRecord`] с доступом по ключам, JSON-подобное представление ([`FactValue`], `as_json`) и
//! привязку к диапазонам в тексте. Для обмена с API/БД: [`FactValue::to_json_value`](crate::interpretation::FactValue::to_json_value),
//! [`FactRecord::to_json_value`](crate::interpretation::FactRecord::to_json_value); у извлечённого факта из матча —
//! [`ExtractedFact::to_json_value`](crate::parser::ExtractedFact::to_json_value).
//! Это упрощает бизнес-логику, отчёты и пост-обработку без ручного
//! разбора дерева разбора.
//!
//! **Как создать (минимальная цепочка):**
//! 1. Задать схему факта — макрос `fact!` (см. подмодуль [`fact`](mod@fact)) или функция [`fact`](fact::fact)
//!    с описанием атрибутов;
//! 2. На фрагментах правила вызвать [`.interpretation(...)`](RuleInterpretation::interpretation), чтобы
//!    связать совпадение с полем (при необходимости — `.normalized()`, `.inflected(...)`, `.custom(...)`);
//! 3. На корневом правиле — [`.interpretation_fact::<ВашТип>()`](RuleInterpretation::interpretation_fact)
//!    либо обёртка через [`.interpretation(...)`](RuleInterpretation::interpretation) для динамических схем
//!    и интерпретаторов из `prepare_*_interpretator`;
//! 4. Добавить правило в реестр, парсить текст; из матча — `fact` / `facts` (см. [`Parser`](crate::parser::Parser)).
//!
//! # Ключевые сценарии
//!
//! Ниже — пошагово с примерами. В общем виде: импорты → схема факта → интерпретации на кусках
//! грамматики → корневая обёртка → `RuleRegistry` и `Parser`.
//!
//! 0. Импортировать `fact` и `interpretation`:
//! ```rust
//! use renert::fact;
//! use renert::interpretation::{FactValue, RuleInterpretation};
//! ```
//! 1. Описать факт декларативно (`fact!`) или динамически (`fact("Name", ...)`).
//! 2. Собрать правило: на терминах/подправилах — `.interpretation(Fact::field)`; на корне —
//!    `.interpretation_fact::<Fact>()` (для динамики — `prepare_*_interpretator` и `.interpretation(...)`).
//! 3. Зарегистрировать правило, вызвать `Parser::find` / `findall` / `r#match` и читать факты из матчей.
//!
//! Ниже приведён пример с извлечением дат с помощью интерпретации:
//!
//! ```rust
//! use renert::fact;
//! use renert::interpretation::{FactValue, RuleInterpretation};
//! use renert::predicates::{dictionary, gte, lte};
//! use renert::{and, pred, term, Parser, RuleRegistry};
//!
//! fact!(Date => [year, month, day]);
//!
//! static MONTHS: &[&str] = &[
//!     "январь", "февраль", "март", "апрель", "мая", "июнь",
//!     "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
//! ];
//!
//! fn main() {
//!     let month_name = pred(dictionary(MONTHS));
//!     let month = pred(and(vec![gte(1), lte(12)]));
//!     let day = pred(and(vec![gte(1), lte(31)]));
//!     let year = pred(and(vec![gte(1900), lte(2100)]));
//!
//!     let mut registry = RuleRegistry::new();
//!     let date_id = registry.next_id();
//!
//!     // Аналог:
//!     // or_(
//!     //   rule(DAY.interpretation(Date.day), MONTH_NAME.interpretation(Date.month), YEAR.interpretation(Date.year)),
//!     //   rule(YEAR.interpretation(Date.year), '-', MONTH.interpretation(Date.month), '-', DAY.interpretation(Date.day)),
//!     //   rule(YEAR.interpretation(Date.year), 'г', '.')
//!     // ).interpretation(Date)
//!     let date_rule = (
//!         day.clone().interpretation(Date::day)
//!             + month_name.interpretation(Date::month)
//!             + year.clone().interpretation(Date::year)
//!     ) | (
//!         year.clone().interpretation(Date::year)
//!             + term("-")
//!             + month.clone().interpretation(Date::month)
//!             + term("-")
//!             + day.clone().interpretation(Date::day)
//!     ) | (
//!         year.interpretation(Date::year) + term("г") + term(".")
//!     );
//!
//!     let date_rule = date_rule
//!         .interpretation_fact::<Date>()
//!         .build(date_id);
//!
//!     registry.add(date_rule);
//!     registry.validate().expect("registry must be valid");
//!
//!     let parser = Parser::new(&registry, date_id);
//!     let text = "2015г.\n18 июля 2016\n2016-01-02\n";
//!
//!     for m in parser.findall(text) {
//!         if let Some(fact) = m.fact(&registry) {
//!             println!("{}", fact);
//!
//!             // Доступ к полям
//!             println!("year = {:?}", fact.get("year"));
//!
//!             // JSON-проекция без None
//!             println!("as_json = {:?}\n", fact.as_json());
//!         }
//!     }
//! }
//! ```
//!
//! Output:
//! ```text
//! Date(
//!     year='2015',
//!     month=None,
//!     day=None
//! )
//! year = Some(Str("2015"))
//! as_json = [("year", Str("2015"))]
//!
//! Date(
//!     year='2016',
//!     month='июля',
//!     day='18'
//! )
//! year = Some(Str("2016"))
//! as_json = [("year", Str("2016")), ("month", Str("июля")), ("day", Str("18"))]
//!
//! Date(
//!     year='2016',
//!     month='01',
//!     day='02'
//! )
//! year = Some(Str("2016"))
//! as_json = [("year", Str("2016")), ("month", Str("01")), ("day", Str("02"))]
//! ```
//!
//! # Нормализация
//! Содержание полей фактов можно нормировать. Например, получать не
//! `Date('июня', '2016')`, а `Date(6, 2016)`; не `Person('президента', Name('Владимира', 'Путина'))`,
//! а `Person('президент', Name('Владимир', 'Путин'))`. Нормализация
//! задается на уровне вершин-атрибутов через `.normalized()`, `.inflected(...)`,
//! `.custom(...)` и их композиции.
//!
//! В примере ниже слово `"июня"` приводится к нормальной форме `"июнь"` и затем
//! заменяется на число `6` через словарь `MONTHS`. День и год просто приводятся к `i64`:
//!
//! ```rust
//! use std::collections::HashMap;
//!
//! use renert::fact;
//! use renert::interpretation::{FactValue, RuleInterpretation};
//! use renert::predicates::{dictionary, gte, lte};
//! use renert::{and, pred, term, Parser, RuleRegistry};
//!
//! fact!(Date => [year, month, day]);
//!
//! static MONTHS: &[&str] = &[
//!     "январь", "февраль", "март", "апрель", "мая", "июнь",
//!     "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
//! ];
//!
//! fn parse_int(value: FactValue) -> i64 {
//!     value.as_str().unwrap().parse::<i64>().unwrap()
//! }
//!
//! fn main() {
//!     let months: HashMap<&'static str, i64> = HashMap::from([
//!         ("январь", 1),
//!         ("февраль", 2),
//!         ("март", 3),
//!         ("апрель", 4),
//!         ("мая", 5),
//!         ("июнь", 6),
//!         ("июль", 7),
//!         ("август", 8),
//!         ("сентябрь", 9),
//!         ("октябрь", 10),
//!         ("ноябрь", 11),
//!         ("декабрь", 12),
//!     ]);
//!
//!     let month_name = pred(dictionary(MONTHS));
//!     let month = pred(and(vec![gte(1), lte(12)]));
//!     let day = pred(and(vec![gte(1), lte(31)]));
//!     let year = pred(and(vec![gte(1900), lte(2100)]));
//!
//!     let mut registry = RuleRegistry::new();
//!     let date_id = registry.next_id();
//!
//!     let date_rule = (
//!         day.clone().interpretation(Date::day.custom(parse_int))
//!             + month_name.interpretation(
//!                 Date::month.normalized().custom(move |value: String| {
//!                     months.get(value.as_str()).copied().unwrap()
//!                 })
//!             )
//!             + year.clone().interpretation(Date::year.custom(parse_int))
//!     )
//!         | (
//!             year.clone().interpretation(Date::year.custom(parse_int))
//!                 + term("-")
//!                 + month.interpretation(Date::month.custom(parse_int))
//!                 + term("-")
//!                 + day.interpretation(Date::day.custom(parse_int))
//!         )
//!         | (
//!             year.interpretation(Date::year.custom(parse_int))
//!                 + term("г")
//!                 + term(".")
//!         );
//!
//!     let date_rule = date_rule
//!         .interpretation_fact::<Date>()
//!         .build(date_id);
//!
//!     registry.add(date_rule);
//!     registry.validate().expect("registry must be valid");
//!
//!     let parser = Parser::new(&registry, date_id);
//!     let m = parser.r#match("18 июня 2016").expect("expected match");
//!     let fact = m.fact(&registry).expect("expected date fact");
//!
//!     println!("{}", fact);
//! }
//! ```
//!
//! Output:
//! ```text
//! Date(
//!     year=2016,
//!     month=6,
//!     day=18
//! )
//! ```
//!
//! # Извлечение значений с инфлексией
//! `inflected(forms)` полезен, когда значение поля нужно привести к согласованной
//! морфологической форме: например, вытаскивать должности/имена в именительном
//! падеже или собирать канонический вид факта для последующей обработки.
//!
//! ```rust
//! use renert::fact;
//! use renert::interpretation::RuleInterpretation;
//! use renert::pipeline::morph_pipeline;
//! use renert::predicates::gram;
//! use renert::{pred, rule, Parser, RuleBuilder, RuleRegistry};
//!
//! fact!(Person => [position, name]);
//! fact!(Name => [first, last]);
//!
//! static FORMS: &[&str] = &["nomn", "sing"];
//!
//! fn build_person_rule<'a>() -> RuleBuilder<'a> {
//!     let position = RuleBuilder::from_arc(
//!         morph_pipeline(["премьер министр", "президент"])
//!     );
//!
//!     let name = rule([
//!         pred(gram("Name")).interpretation(Name::first.inflected(FORMS)),
//!         pred(gram("Surn")).interpretation(Name::last.inflected(FORMS)),
//!     ])
//!     .interpretation_fact::<Name>();
//!
//!     rule([
//!         position.interpretation(Person::position.inflected(FORMS)),
//!         name.interpretation(Person::name),
//!     ])
//!     .interpretation_fact::<Person>()
//! }
//!
//! fn main() {
//!     let mut registry = RuleRegistry::new();
//!     let root_id = registry.add(build_person_rule().build(()));
//!     registry.validate().expect("registry must be valid");
//!
//!     let parser = Parser::new(&registry, root_id);
//!     let text = "12 марта по приказу президента Владимира Путина ...";
//!
//!     for m in parser.findall(text) {
//!         if let Some(fact) = m.fact(&registry) {
//!             println!("{}", fact);
//!         }
//!     }
//! }
//! ```
//! Output:
//! ```text
//! Person(
//!     position='президент',
//!     name=Name(
//!         first='Владимир',
//!         last='Путин'
//!     )
//! )
//! ```
//!
//! Рекомендации по выбору граммем для `inflected()`:
//! - Падеж: `nomn`, `gent`, `datv`, `accs`, `ablt`, `loct` — когда нужно привести поле к роли в фразе.
//! - Число: `sing`, `plur` — когда важна каноническая форма для единичных/множественных сущностей.
//! - Род: `masc`, `femn`, `neut` — в первую очередь для прилагательных/причастий и согласованных конструкций.
//! - Часть речи и спец. граммемы (`NOUN`, `inan`, `anim` и т.п.) используйте точечно, если надо сузить неоднозначные разборы.
//!

/// Подмодуль описания атрибутов и их трансформаций.
pub mod attribute;
/// Подмодуль схем фактов, рантайм-записей и вспомогательных типов (`fact!`, [`fact`](fact::fact), …).
pub mod fact;
mod interpretator;
/// Подмодуль нормализаторов и рантайм-значений нормализации.
pub mod normalizer;
mod value;

/// Базовый тип атрибута факта.
#[doc(inline)]
pub use attribute::Attribute;
/// API фактов, схем, атрибутных описаний и рантайм-записей факта.
#[doc(inline)]
pub use fact::{
    fact, prepare_attribute, ConstructedAttribute, Fact, FactAttribute, FactAttributeInput,
    FactError, FactField, FactMeta, FactRecord, FactRecordRaw, FactScheme, FieldType,
    InterpretatorFact, OrderedFactMap, PreparedAttributeScheme, RuntimeAttributeValue,
};
/// API интерпретаторов, подготовки интерпретаторов и legacy-transform payload.
#[doc(inline)]
pub use interpretator::{
    prepare_attribute_interpretator, prepare_rule_interpretator, prepare_token_interpretator,
    AnyInterpretator, AttributeInterpretator, AttributeNormalizerInterpretator, Chain,
    FactInterpretator, FactResult, Interpretation, InterpretatorError, InterpretatorInput,
    InterpretatorInputItem, InterpretatorResult, NormalizerCallable, NormalizerInterpretator,
    NormalizerResult, NormalizerResultValue, PreparedAttributeItem, PreparedRuleItem,
    PreparedTokenItem, Transform,
};
/// Универсальное значение факта/нормализации.
#[doc(inline)]
pub use value::FactValue;

use std::sync::Arc;

use crate::rule::builder::RuleBuilder;
use crate::rule::constructors::{Rule, RuleKind};

/// Соединяет API-интерфейс `interpretation` с новым модулем `rule`, основанным на графах.
///
/// Новый модуль `rule` поддерживает интерпретационные оболочки на уровне правил, а разбор
/// конкретной полезной нагрузки и сборка [`FactRecord`] выполняются в конвейере парсера и извлечения фактов.
pub trait RuleInterpretation<'a>: Sized {
    /// Помечает правило интерпретирующей оболочкой.
    fn interpretation<T>(self, item: T) -> Self
    where
        T: Into<Interpretation>;

    /// Помечает правило как основанное на фактах.
    ///
    /// В графическом режиме оно сопоставляется с именованным узлом верхнего уровня, сохраняя
    /// совместимость с fluent API.
    fn interpretation_fact<F>(self) -> Self
    where
        F: FactMeta;
}

impl<'a> RuleInterpretation<'a> for RuleBuilder<'a> {
    fn interpretation<T>(self, item: T) -> Self
    where
        T: Into<Interpretation>,
    {
        let interpretation: Interpretation = item.into();
        let rule = self.into_arc();
        RuleBuilder::from_arc(Arc::new(Rule {
            kind: RuleKind::Interpretation {
                rule,
                interpretation,
            },
        }))
    }

    fn interpretation_fact<F>(self) -> Self
    where
        F: FactMeta,
    {
        if let Some(scheme) = F::scheme() {
            self.named_fact(scheme)
        } else {
            self.named(F::NAME)
        }
    }
}

impl<'a> RuleInterpretation<'a> for Arc<Rule<'a>> {
    fn interpretation<T>(self, item: T) -> Self
    where
        T: Into<Interpretation>,
    {
        let interpretation: Interpretation = item.into();
        Arc::new(Rule {
            kind: RuleKind::Interpretation {
                rule: self,
                interpretation,
            },
        })
    }

    fn interpretation_fact<F>(self) -> Self
    where
        F: FactMeta,
    {
        if let Some(scheme) = F::scheme() {
            self.named_fact(scheme)
        } else {
            self.named(F::NAME)
        }
    }
}

#[cfg(test)]
mod tests;
