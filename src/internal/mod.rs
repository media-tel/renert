//! Внутренние общие типы для слоев `rule`, `parser` и интерпретации.
//!
//! Модуль содержит:
//! - payload интерпретации [`CoreInterpretation`];
//! - описание трансформаций [`CoreTransform`];
//! - схемы фактов и нормализованные записи [`fact_schema`].

pub mod fact_schema;
pub mod interpretation_payload;

/// Общие схемы фактов и нормализованные записи.
pub use fact_schema::{
    fact, prepare_attribute, ConstructedAttribute, FactAttributeInput, FactError, FactRecord,
    FactRecordRaw, FactScheme, OrderedFactMap, PreparedAttributeScheme,
};
/// Общая форма payload интерпретации и трансформаций.
pub use interpretation_payload::{CoreInterpretation, CoreTransform};
