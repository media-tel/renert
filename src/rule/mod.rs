//! Подсистема правил грамматики.
//!
//! Модуль объединяет:
//! - конструкторы и fluent-builder для описания грамматик;
//! - нормализацию графа правил;
//! - преобразование в BNF;
//! - реестр правил для парсера.
//!
//! ## Логические операции API
//! Для правил используются:
//! - [`crate::rule::builder::or_`] или оператор `|` — альтернатива (`OR`);
//! - [`crate::rule::builder::rule`] или оператор `+` — последовательность (`AND` в смысле
//!   конкатенации термов, а не булевой операции над предикатами).
//!
//! Для `Rule` отдельного булевого `not`-комбинатора нет: отрицание выражается
//! на уровне терминалов через предикаты (например, `pred(not(eq("...")))`).
//!
//! ## Грамматики
//! Контекстно-свободная грамматика в API этой библиотеки задается через Rust-конструкторы
//! из [`crate::rule::builder`]. Например, классическая грамматика размеров одежды:
//!
//! ```text
//! KEY -> р. | размер
//! VALUE -> S | M | L
//! SIZE -> KEY VALUE
//! ```
//!
//! Rust-версия в этой библиотеке:
//!
//! ```rust
//! use renert::{or_, rule, term};
//!
//! let key = or_([
//!     rule([term("р"), term(".")]),
//!     rule([term("размер")]),
//! ])
//! .named("KEY");
//!
//! let value = or_([
//!     rule([term("S")]),
//!     rule([term("M")]),
//!     rule([term("L")]),
//! ])
//! .named("VALUE");
//!
//! let size = rule([key, value]).named("SIZE");
//!
//! println!("{}", size.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! SIZE -> KEY VALUE
//! KEY -> 'р' '.' | 'размер'
//! VALUE -> 'S' | 'M' | 'L'
//! ```
//!
//! В этой библиотеке терминал грамматики тоже выражается предикатом.
//! Сценарий сокращения записи VALUE через предикат in_:
//!
//! ```rust
//! use renert::predicates::in_;
//! use renert::{or_, pred, rule, term};
//!
//! let key = or_([
//!     rule([term("р"), term(".")]),
//!     rule([term("размер")]),
//! ])
//! .named("KEY");
//!
//! let value = pred(in_(&["S", "M", "L"])).named("VALUE");
//! let size = rule([key, value]).named("SIZE");
//!
//! println!("{}", size.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! SIZE -> KEY VALUE
//! KEY -> 'р' '.' | 'размер'
//! VALUE -> in_(...)
//! ```
//!
//! Для рекурсивных правил используется `forward`:
//!
//! ```text
//! EXPR -> a | ( EXPR + EXPR )
//! ```
//!
//! ```rust
//! use renert::{forward, or_, rule, term};
//!
//! let expr = forward();
//! expr.define(
//!     or_([
//!         rule([term("a")]),
//!         rule([term("("), expr.clone(), term("+"), expr.clone(), term(")")]),
//!     ])
//!     .named("EXPR"),
//! );
//!
//! println!("{}", expr.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! EXPR -> 'a' | R0 EXPR ')'
//! R0 -> '(' EXPR '+'
//! ```
//!
//! Рекурсивные правила описывают последовательности произвольной длины.
//! Пример текста в кавычках:
//!
//! ```rust
//! use renert::{eq, forward, not, or_, pred, rule, term};
//!
//! let word = pred(not(eq("»")));
//! let text = forward();
//! text.define(or_([
//!     rule([word.clone()]),
//!     rule([word.clone(), text.clone()]),
//! ]));
//!
//! let title = rule([term("«"), text, term("»")]).named("TITLE");
//!
//! println!("{}", title.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! TITLE -> '«' R0 '»'
//! R0 -> not('»') | not('»') R0
//! ```
//!
//! Для сокращенной записи повторов используется `repeatable` (внутренний `forward`
//! добавляется автоматически):
//!
//! ```rust
//! use renert::{eq, not, pred, rule, term};
//!
//! let title = rule([
//!     term("«"),
//!     pred(not(eq("»"))).repeatable(),
//!     term("»"),
//! ])
//! .named("TITLE");
//!
//! println!("{}", title.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! TITLE -> '«' R0 '»'
//! R0 -> not('»') R0 | not('»')
//! ```
//!
//! Для управления границами повторения используйте `repeatable_with(min, max, reverse)`:
//! - `repeatable_with(None, None, reverse)` — без ограничений;
//!   при `reverse = false` эквивалентно `repeatable()`;
//! - `repeatable_with(Some(min), None, reverse)` — не меньше `min`;
//! - `repeatable_with(None, Some(max), reverse)` — не больше `max`;
//! - `repeatable_with(Some(min), Some(max), reverse)` — диапазон `[min, max]`.
//!
//! Параметр `reverse` управляет порядком альтернатив в результирующем BNF.
//! Для bounded-вариантов должны выполняться ограничения: `min >= 1`, `max >= 1`, `max >= min`.
//!
//! ```rust
//! use renert::term;
//!
//! let sized = term("a")
//!     .repeatable_with(Some(2), Some(3), false)
//!     .named("A23");
//!
//! println!("{}", sized.normalized().as_bnf());
//! ```
//!
//! Output:
//! ```text
//! A23 -> 'a' R0
//! R0 -> 'a' 'a' | 'a'
//! ```
//!
//! ## Инструкция по сборке правил
//! Базовый рабочий порядок при сборке грамматики:
//! 1. Опишите терминалы через `term(...)` или `pred(...)`.
//! 2. Соберите правила:
//!    - конкатенация через `rule([...])` или оператор `+`;
//!    - альтернатива через `or_([...])` или оператор `|`;
//!    - именование через `.named("...")`.
//! 3. Для рекурсии используйте `forward()` и затем `.define(...)`.
//! 4. Для квантификаторов используйте:
//!    - `.optional()` / `.optional_with(reverse)` — когда фрагмент может
//!      отсутствовать (0 или 1 вхождение), `reverse` меняет порядок альтернатив;
//!    - `.repeatable()` / `.repeatable_with(min, max, reverse)` — когда фрагмент
//!      может повторяться (без границ или с границами `min/max`), `reverse`
//!      влияет на порядок вариантов в результирующей грамматике.
//! 5. Перед выводом в BNF вызовите `.normalized()`.
//! 6. Для текстового BNF-вывода используйте короткий API:
//!    - `println!("{}", rule.normalized().as_bnf())` — получить BNF как строку и вывести.
//! 7. Если нужен структурный объект BNF (например, `source()/start()`),
//!    получите его так:
//!    - `let bnf = rule.normalized().bnf();`
//!      (ручной `BnfTransformator` обычно больше не нужен).
//!
//! Минимальный шаблон:
//! ```rust
//! use renert::{or_, rule, term};
//!
//! let grammar = or_([
//!     rule([term("a"), term("b")]),
//!     rule([term("c")]),
//! ])
//! .named("ROOT");
//!
//! println!("{}", grammar.normalized().as_bnf());
//!
//! // Если нужен доступ к структуре BNF:
//! // let bnf = grammar.normalized().bnf();
//! // let lines: Vec<String> = bnf.source().collect();
//! ```
/// Преобразование графа правил в BNF-граф и строковый BNF-источник.
pub mod bnf;
/// Fluent API для удобного построения правил.
pub mod builder;
/// Низкоуровневые структуры `Rule`/`Production`/`Term` и обход графа правил.
pub mod constructors;
/// Реестр правил с lookup/валидацией и lookahead-предсказанием продукций.
pub mod registry;
/// Набор трансформаторов для нормализации графа правил.
pub mod transformator;

#[cfg(test)]
mod tests;
