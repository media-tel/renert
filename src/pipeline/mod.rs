//! Газеттиры (pipelines) для быстрого описания словарей.
//!
//! ## Газеттир
//!
//! Словарь профессий, географических объектов и других устойчивых фраз можно записывать
//! стандартными средствами через `rule`, `or_`, `normalized`, `caseless`:
//!
//! ```rust
//! use renert::predicates::{caseless, normalized};
//! use renert::{or_, pred, rule, term};
//!
//! let _position = or_([
//!     rule([
//!         pred(normalized("генеральный")),
//!         pred(normalized("директор")),
//!     ]),
//!     rule([
//!         pred(normalized("бухгалтер")),
//!     ]),
//! ]);
//!
//! let _geo = or_([
//!     rule([
//!         pred(normalized("Ростов")),
//!         term("-"),
//!         pred(caseless("на")),
//!         term("-"),
//!         pred(caseless("Дону")),
//!     ]),
//!     rule([
//!         pred(normalized("Москва")),
//!     ]),
//! ]);
//! ```
//!
//! Это неудобно, и в такой записи легко ошибиться. Для составления словарей в библиотеке
//! используются готовые газеттиры: [`morph_pipeline`] и [`caseless_pipeline`].
//!
//! `morph_pipeline` перед сравнением приводит слова к нормальной форме:
//!
//! ```rust
//! use renert::pipeline::morph_pipeline;
//! use renert::{Parser, RuleRegistry};
//!
//! let ty = morph_pipeline(["электронный дневник"]);
//!
//! let mut registry = RuleRegistry::new();
//! let root_id = registry.add(ty);
//! let parser = Parser::new(&registry, root_id);
//!
//! let text = "электронным дневником, электронные дневники, электронное дневнику";
//! for m in parser.findall(text) {
//!     let values: Vec<String> = m
//!         .tokens()
//!         .into_iter()
//!         .map(|t| t.value.into_owned())
//!         .collect();
//!     println!("{:?}", values);
//! }
//! ```
//! Output:
//! ```text
//! ["электронным", "дневником"]
//! ["электронные", "дневники"]
//! ["электронное", "дневнику"]
//! ```
//!
//! `caseless_pipeline` ищет слова без морфологической нормализации, но без учёта регистра.
//! Например, поиск арабских имён:
//!
//! ```rust,no_run
//! use renert::pipeline::caseless_pipeline;
//! use renert::{Parser, RuleRegistry};
//!
//! let name = caseless_pipeline([
//!     "Абд Аль-Азиз Бин Мухаммад",
//!     "Абд ар-Рахман Наср ас-Са ди",
//! ]);
//!
//! let mut registry = RuleRegistry::new();
//! let root_id = registry.add(name);
//! let parser = Parser::new(&registry, root_id);
//!
//! let text = "Абд Аль-Азиз Бин Мухаммад, АБД АР-РАХМАН НАСР АС-СА ДИ";
//! for m in parser.findall(text) {
//!     let values: Vec<String> = m
//!         .tokens()
//!         .into_iter()
//!         .map(|t| t.value.into_owned())
//!         .collect();
//!     println!("{:?}", values);
//! }
//! ```
//! Output:
//! ```text
//! ["Абд", "Аль", "-", "Азиз", "Бин", "Мухаммад"]
//! ["АБД", "АР", "-", "РАХМАН", "НАСР", "АС", "-", "СА", "ДИ"]
//! ```
//!
//! ## Расширенный публичный API
//!
//! Помимо фасадных функций, модуль экспортирует структуры для поэтапной сборки и отладки
//! словарей:
//!
//! - [`Key`] — один словарный ключ (`value` + список термов `terms`);
//! - [`PipelineProduction`] — продукция правила с исходным `value` ключа;
//! - [`Pipeline`], [`CaselessPipeline`], [`MorphPipeline`] — активированные пайплайны
//!   с методами `productions`, `as_bnf`, `into_rule`;
//! - [`PipelineScheme`], [`CaselessPipelineScheme`], [`MorphPipelineScheme`] — схемы, где
//!   строки задаются заранее и активируются нужным токенизатором;
//! - [`PipelineBNFRule`], [`CaselessPipelineBNFRule`], [`MorphPipelineBNFRule`] — BNF-слой
//!   с `predict(...)` для lookahead-кандидатов.
//!
//! Пример: поэтапная активация схемы и получение `Rule`:
//!
//! ```rust
//! use renert::pipeline::pipeline_scheme;
//!
//! let scheme = pipeline_scheme(["генеральный директор", "бухгалтер"]);
//! let pipeline = scheme.activate_default();
//! let _rule = pipeline.into_rule();
//! ```
//!
//! Пример: ручная сборка `Pipeline` через [`Key`] и просмотр продукций:
//!
//! ```rust
//! use renert::pipeline::{Key, Pipeline};
//!
//! let pipeline = Pipeline::new([Key::simple(
//!     "генеральный директор",
//!     vec!["генеральный".to_string(), "директор".to_string()],
//! )]);
//!
//! let productions = pipeline.productions();
//! assert_eq!(productions.len(), 1);
//! assert_eq!(productions[0].value, "генеральный директор");
//! ```
//!
//! Пример: использование BNF-индекса `predict(...)`:
//!
//! ```rust
//! use renert::pipeline::pipeline_scheme;
//! use renert::token::Tokenizer;
//!
//! let pipeline = pipeline_scheme(["бухгалтер", "генеральный директор"]).activate_default();
//! let bnf = pipeline.as_bnf();
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.tokenize("бухгалтер");
//! let candidates: Vec<_> = bnf.predict(&tokens[0]).collect();
//! assert!(!candidates.is_empty());
//! ```
//!
//! Для простых сценариев остаются фасады [`pipeline`], [`caseless_pipeline`],
//! [`morph_pipeline`]. Для кастомной активации доступны схемы:
//! [`pipeline_scheme`], [`caseless_pipeline_scheme`], [`morph_pipeline_scheme`].
//!
//! Внутри активированные пайплайны компилируют rule/BNF-представление лениво и
//! переиспользуют его в повторных вызовах `productions`, `as_bnf`, `into_rule`
//! в рамках одного экземпляра пайплайна.
//! Для exact/caseless-схем `activate_default()` использует обычный [`Tokenizer`](crate::token::Tokenizer)
//! и не инициализирует морфологию без необходимости.

#[allow(clippy::module_inception)]
mod pipeline;

pub use pipeline::{
    caseless_pipeline, caseless_pipeline_scheme, morph_pipeline, morph_pipeline_scheme, pipeline,
    pipeline_scheme, CaselessPipeline, CaselessPipelineBNFRule, CaselessPipelineScheme, Key,
    MorphPipeline, MorphPipelineBNFRule, MorphPipelineScheme, Pipeline, PipelineBNFRule,
    PipelineProduction, PipelineScheme,
};
