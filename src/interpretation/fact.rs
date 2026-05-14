//! API фактов и рантайм-структур интерпретации.
//!
//! Модуль содержит:
//! - базовые трейты фактов ([`Fact`], [`FactMeta`], [`FieldType`]);
//! - дескрипторы полей и атрибутов ([`FactField`], [`FactAttribute`]);
//! - рантайм-представления факта ([`FactRecord`], [`InterpretatorFact`]);
//! - функции подготовки схем (`prepare_attribute`, `fact`).
//!
//! Основная цель: дать единое представление факта на всех этапах пайплайна
//! от описания схемы до финального значения, пригодного для `as_json`/`spans`.

use super::attribute::{
    Attribute, ConstAttribute, FunctionAttribute, InflectedAttribute, NormalizedAttribute,
    RepeatableAttribute,
};
use super::normalizer::{InterpretationValue, NormalizedItem, RuntimeItem, RuntimeSpans};
use super::FactValue;
pub use crate::internal::{
    fact, prepare_attribute, ConstructedAttribute, FactAttributeInput, FactError, FactRecord,
    FactRecordRaw, FactScheme, OrderedFactMap, PreparedAttributeScheme,
};
use crate::interpretation::Interpretation;
use crate::span::Span;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::marker::PhantomData;

/// Базовый трейт факта.
///
/// Используется для типизированных фактов (через `fact!`), которые можно
/// хранить за `Box<dyn Fact>`.
pub trait Fact: Debug + Send + Sync {
    /// Возвращает имя факта (обычно имя структуры/типа).
    fn fact_name(&self) -> &str;

    /// Возвращает упорядоченный список имён полей факта.
    fn field_names(&self) -> &[&str];

    /// Возвращает значение поля по имени.
    fn get_field(&self, name: &str) -> Option<FactValue>;

    /// Устанавливает значение поля по имени.
    ///
    /// Возвращает `true`, если поле успешно обновлено.
    fn set_field(&mut self, name: &str, value: FactValue) -> bool;

    /// Клонирует факт в объектный тип `Box<dyn Fact>`.
    fn clone_box(&self) -> Box<dyn Fact>;
}

impl Clone for Box<dyn Fact> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Метаданные типа факта, известные на этапе компиляции.
pub trait FactMeta {
    /// Статическое имя факта.
    const NAME: &'static str;

    /// Полная схема факта, если она известна на этапе компиляции.
    fn scheme() -> Option<FactScheme> {
        None
    }
}

/// Дескриптор атрибута факта для передачи в `.interpretation(...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FactAttribute {
    fact_name: &'static str,
    field_name: &'static str,
    const_val: Option<&'static str>,
}

impl FactAttribute {
    /// Создаёт дескриптор атрибута факта.
    #[inline]
    pub const fn new(fact_name: &'static str, field_name: &'static str) -> Self {
        Self {
            fact_name,
            field_name,
            const_val: None,
        }
    }

    /// Возвращает имя факта.
    #[inline]
    pub fn fact_name(&self) -> &'static str {
        self.fact_name
    }

    /// Возвращает имя поля факта.
    #[inline]
    pub fn field_name(&self) -> &'static str {
        self.field_name
    }

    /// Фиксирует константное значение для поля.
    ///
    /// Применяется в сценариях вида
    /// `.interpretation(Fact::field.with_const("value"))`.
    #[inline]
    pub fn with_const(mut self, value: &'static str) -> Self {
        self.const_val = Some(value);
        self
    }

    /// Возвращает константное значение, если оно задано.
    #[inline]
    pub fn get_const(&self) -> Option<&'static str> {
        self.const_val
    }

    /// Проверяет, задано ли константное значение.
    #[inline]
    pub fn has_const(&self) -> bool {
        self.const_val.is_some()
    }
}

/// Дескриптор поля факта.
///
/// Типовой параметр `F` задаёт факт-владелец (через [`FactMeta`]).
#[derive(Debug)]
pub struct FactField<F> {
    name: &'static str,
    _marker: PhantomData<F>,
}

impl<F> FactField<F> {
    /// Создаёт дескриптор поля.
    #[inline]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }

    /// Возвращает имя поля.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl<F: FactMeta> FactField<F> {
    /// Создаёт базовый атрибут `Fact.field`.
    #[inline]
    pub fn attr(&self) -> FactAttribute {
        FactAttribute::new(F::NAME, self.name)
    }

    /// Создаёт трансформацию `Fact.field.normalized()`.
    #[inline]
    pub fn normalized(&self) -> NormalizedAttribute {
        Attribute::new(F::NAME, self.name, None).normalized()
    }

    /// Создаёт трансформацию `Fact.field.inflected(forms)`.
    #[inline]
    pub fn inflected(&self, forms: &'static [&'static str]) -> InflectedAttribute {
        Attribute::new(F::NAME, self.name, None).inflected_with(forms.iter().copied())
    }

    /// Создаёт пользовательскую трансформацию `Fact.field.custom(function)`.
    #[inline]
    pub fn custom<FN>(&self, function: FN) -> FunctionAttribute<FN> {
        Attribute::new(F::NAME, self.name, None).custom(function)
    }

    /// Помечает поле как repeatable.
    #[inline]
    pub fn repeatable(&self) -> RepeatableAttribute {
        RepeatableAttribute::new(F::NAME, self.name)
    }

    /// Создаёт константную трансформацию `Fact.field.const(value)`.
    #[inline]
    pub fn r#const<T>(&self, value: T) -> ConstAttribute<T> {
        Attribute::new(F::NAME, self.name, None).r#const(value)
    }

    /// Возвращает [`FactAttribute`] с константным строковым значением.
    #[inline]
    pub fn with_const(&self, value: &'static str) -> FactAttribute {
        FactAttribute::new(F::NAME, self.name).with_const(value)
    }
}

// Позволяет передавать `FactField` (например, `Date::year`) напрямую в
// `.interpretation(...)` без явного вызова `.attr()`.
impl<F: FactMeta> From<FactField<F>> for Interpretation {
    #[inline]
    fn from(field: FactField<F>) -> Self {
        field.attr().into()
    }
}

impl<F> Clone for FactField<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for FactField<F> {}

#[doc(hidden)]
/// Вспомогательная конвертация `Option<T>` в `Option<FactValue>`.
pub fn field_to_value<T: FieldType>(field: &Option<T>) -> Option<FactValue> {
    field.as_ref().map(|v| v.to_fact_value())
}

#[doc(hidden)]
/// Вспомогательная установка типизированного поля из `FactValue`.
///
/// Возвращает `true`, если преобразование типа прошло успешно.
pub fn set_field_value<T: FieldType>(field: &mut Option<T>, value: FactValue) -> bool {
    match T::from_fact_value(value) {
        Some(v) => {
            *field = Some(v);
            true
        }
        None => false,
    }
}

/// Трейт типов, допустимых в полях типизированных фактов.
pub trait FieldType: Clone {
    /// Преобразует значение поля в [`FactValue`].
    fn to_fact_value(&self) -> FactValue;

    /// Пытается восстановить тип поля из [`FactValue`].
    fn from_fact_value(value: FactValue) -> Option<Self>;
}

impl FieldType for String {
    fn to_fact_value(&self) -> FactValue {
        FactValue::Str(self.clone())
    }

    fn from_fact_value(value: FactValue) -> Option<Self> {
        match value {
            FactValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl FieldType for i64 {
    fn to_fact_value(&self) -> FactValue {
        FactValue::Int(*self)
    }

    fn from_fact_value(value: FactValue) -> Option<Self> {
        match value {
            FactValue::Int(n) => Some(n),
            _ => None,
        }
    }
}

impl FieldType for bool {
    fn to_fact_value(&self) -> FactValue {
        FactValue::Bool(*self)
    }

    fn from_fact_value(value: FactValue) -> Option<Self> {
        match value {
            FactValue::Bool(b) => Some(b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RuntimeAttributeValue<'a> {
    /// Обычное значение (возможно, отсутствует).
    Plain(Option<FactValue>),
    /// Одно значение интерпретации.
    Result(InterpretationValue<'a>),
    /// Набор значений для repeatable-атрибута.
    Repeatable(Vec<InterpretationValue<'a>>),
}

#[derive(Debug, Clone)]
struct RuntimeFactLayout {
    index_by_name: HashMap<String, usize>,
    names: Vec<String>,
    defaults: Vec<Option<FactValue>>,
    repeatable: Vec<bool>,
}

impl RuntimeFactLayout {
    fn from_scheme(scheme: &FactScheme) -> Self {
        let mut index_by_name = HashMap::with_capacity(scheme.attributes_order.len());
        let mut names = Vec::with_capacity(scheme.attributes_order.len());
        let mut defaults = Vec::with_capacity(scheme.attributes_order.len());
        let mut repeatable = Vec::with_capacity(scheme.attributes_order.len());

        for (index, key) in scheme.attributes_order.iter().enumerate() {
            let attribute = scheme
                .attributes
                .get(key)
                .expect("fact scheme must contain every ordered attribute");

            index_by_name.insert(key.clone(), index);
            names.push(key.clone());
            defaults.push(attribute.default_value_ref().cloned());
            repeatable.push(attribute.is_repeatable());
        }

        Self {
            index_by_name,
            names,
            defaults,
            repeatable,
        }
    }

    #[inline]
    fn index_of(&self, key: &str) -> Option<usize> {
        self.index_by_name.get(key).copied()
    }

    #[inline]
    fn len(&self) -> usize {
        self.names.len()
    }
}

/// Рантайм-аккумулятор факта (аналог Python `InterpretatorFact`).
#[derive(Debug, Clone)]
pub struct InterpretatorFact<'a> {
    /// Схема факта.
    scheme: FactScheme,
    /// Индексное представление runtime-факта.
    layout: RuntimeFactLayout,
    /// Текущее состояние атрибутов.
    values: Vec<RuntimeAttributeValue<'a>>,
    /// Индексы атрибутов, изменённых в ходе интерпретации.
    modified_indices: Vec<usize>,
    /// Маска модифицированных атрибутов для O(1)-проверок.
    modified_mask: Vec<bool>,
    /// Кэш JSON-проекции runtime-факта.
    cached_as_json: RefCell<Option<OrderedFactMap>>,
    /// Кэш объединённых spans runtime-факта.
    cached_spans: RefCell<Option<Vec<Span>>>,
}

impl<'a> InterpretatorFact<'a> {
    #[inline]
    fn invalidate_caches(&mut self) {
        *self.cached_as_json.borrow_mut() = None;
        *self.cached_spans.borrow_mut() = None;
    }

    #[inline]
    fn mark_modified(&mut self, index: usize) {
        if !self.modified_mask[index] {
            self.modified_mask[index] = true;
            self.modified_indices.push(index);
        }
    }

    #[inline]
    fn collect_spans(&self, out: &mut Vec<Span>) {
        for &index in &self.modified_indices {
            let value = &self.values[index];
            match value {
                RuntimeAttributeValue::Repeatable(items) => {
                    for item in items {
                        item.append_spans(out);
                    }
                }
                RuntimeAttributeValue::Result(item) => item.append_spans(out),
                RuntimeAttributeValue::Plain(_) => {}
            }
        }
    }

    #[inline]
    pub(crate) fn append_spans(&self, out: &mut Vec<Span>) {
        if let Some(cached) = self.cached_spans.borrow().as_ref() {
            out.extend_from_slice(cached);
            return;
        }

        let mut computed = Vec::with_capacity(self.modified_indices.len());
        self.collect_spans(&mut computed);
        out.extend_from_slice(&computed);
        *self.cached_spans.borrow_mut() = Some(computed);
    }

    #[inline]
    fn default_value_by_index(&self, index: usize) -> Option<FactValue> {
        self.layout.defaults[index].clone()
    }

    #[inline]
    fn index_of(&self, key: &str) -> Option<usize> {
        self.layout.index_of(key)
    }

    #[inline]
    fn get_runtime_by_index(&self, index: usize) -> Option<&RuntimeAttributeValue<'a>> {
        self.values.get(index)
    }

    #[inline]
    fn get_runtime_mut_by_index(&mut self, index: usize) -> Option<&mut RuntimeAttributeValue<'a>> {
        self.values.get_mut(index)
    }

    #[inline]
    fn compute_as_json(&self) -> OrderedFactMap {
        self.project_fields(false).1
    }

    #[inline]
    fn compute_spans(&self) -> Vec<Span> {
        let mut spans = Vec::with_capacity(self.modified_indices.len());
        self.append_spans(&mut spans);
        spans
    }

    #[inline]
    pub(crate) fn cached_as_json_cloned(&self) -> OrderedFactMap {
        if let Some(cached) = self.cached_as_json.borrow().clone() {
            return cached;
        }

        let computed = self.compute_as_json();
        *self.cached_as_json.borrow_mut() = Some(computed.clone());
        computed
    }

    #[inline]
    pub(crate) fn cached_spans_cloned(&self) -> Vec<Span> {
        if let Some(cached) = self.cached_spans.borrow().clone() {
            return cached;
        }

        let computed = self.compute_spans();
        *self.cached_spans.borrow_mut() = Some(computed.clone());
        computed
    }

    #[inline]
    pub(crate) fn cached_as_json_object_cloned(&self) -> FactValue {
        FactValue::Object(self.cached_as_json_cloned())
    }

    #[inline]
    fn set_by_index(
        &mut self,
        index: usize,
        value: InterpretationValue<'a>,
    ) -> Result<(), FactError> {
        self.invalidate_caches();

        match self.get_runtime_mut_by_index(index) {
            Some(RuntimeAttributeValue::Repeatable(values)) => {
                values.push(value);
            }
            Some(slot) => {
                *slot = RuntimeAttributeValue::Result(value);
            }
            None => Err(FactError::KeyError(self.layout.names[index].clone()))?,
        }

        self.mark_modified(index);
        Ok(())
    }

    /// Создаёт новый рантайм-факт из схемы.
    pub fn new(scheme: FactScheme) -> Self {
        let layout = RuntimeFactLayout::from_scheme(&scheme);
        let mut values = Vec::with_capacity(layout.len());

        for is_repeatable in &layout.repeatable {
            if *is_repeatable {
                values.push(RuntimeAttributeValue::Repeatable(Vec::new()));
            } else {
                // Обычные атрибуты храним без раннего клонирования defaults.
                values.push(RuntimeAttributeValue::Plain(None));
            }
        }
        let values_len = values.len();

        Self {
            scheme,
            layout,
            values,
            modified_indices: Vec::new(),
            modified_mask: vec![false; values_len],
            cached_as_json: RefCell::new(None),
            cached_spans: RefCell::new(None),
        }
    }

    #[inline]
    pub fn scheme(&self) -> &FactScheme {
        &self.scheme
    }

    #[inline]
    pub fn is_repeatable(&self, key: &str) -> bool {
        self.index_of(key)
            .map(|index| self.layout.repeatable[index])
            .unwrap_or(false)
    }

    #[inline]
    pub fn contains_modified(&self, key: &str) -> bool {
        self.index_of(key)
            .map(|index| self.modified_mask[index])
            .unwrap_or(false)
    }

    #[inline]
    pub fn get_runtime(&self, key: &str) -> Option<&RuntimeAttributeValue<'a>> {
        self.index_of(key)
            .and_then(|index| self.get_runtime_by_index(index))
    }

    /// Устанавливает значение атрибута.
    ///
    /// Для repeatable-атрибутов значение добавляется в список.
    pub fn set(&mut self, key: &str, value: InterpretationValue<'a>) -> Result<(), FactError> {
        let index = self
            .index_of(key)
            .ok_or_else(|| FactError::KeyError(key.to_string()))?;
        self.set_by_index(index, value)
    }

    /// Сливает изменения из другого рантайм-факта.
    ///
    /// Переносятся только атрибуты, помеченные у `fact` как `modified`.
    pub fn merge(&mut self, fact: &InterpretatorFact<'a>) {
        let mut changed = false;

        if self.layout.names == fact.layout.names {
            for &index in &fact.modified_indices {
                self.values[index] = fact.values[index].clone();
                self.mark_modified(index);
                changed = true;
            }
            if changed {
                self.invalidate_caches();
            }
            return;
        }

        for &source_index in &fact.modified_indices {
            let key = &fact.layout.names[source_index];
            if let Some(target_index) = self.index_of(key) {
                self.values[target_index] = fact.values[source_index].clone();
                self.mark_modified(target_index);
                changed = true;
            }
        }

        if changed {
            self.invalidate_caches();
        }
    }

    pub(crate) fn merge_owned(&mut self, fact: InterpretatorFact<'a>) {
        let same_layout = self.layout.names == fact.layout.names;
        let mut values = fact.values.into_iter().map(Some).collect::<Vec<_>>();
        let mut changed = false;

        for source_index in fact.modified_indices {
            let target_index = if same_layout {
                Some(source_index)
            } else {
                self.index_of(&fact.layout.names[source_index])
            };

            if let Some(target_index) = target_index {
                if let Some(value) = values[source_index].take() {
                    self.values[target_index] = value;
                    self.mark_modified(target_index);
                    changed = true;
                }
            }
        }

        if changed {
            self.invalidate_caches();
        }
    }

    #[inline]
    fn project_fields(
        &self,
        include_attributes: bool,
    ) -> (Option<BTreeMap<String, Option<FactValue>>>, OrderedFactMap) {
        let mut attributes = include_attributes.then(BTreeMap::new);
        let mut as_json = Vec::with_capacity(self.layout.len());

        for index in 0..self.layout.len() {
            let key = &self.layout.names[index];
            let (value, json_value) = match &self.values[index] {
                RuntimeAttributeValue::Repeatable(values) => {
                    let mut json_values = Vec::with_capacity(values.len());
                    if include_attributes {
                        let mut normalized_values = Vec::with_capacity(values.len());
                        for item in values {
                            normalized_values.push(normalized_fact_value(item));
                            json_values.push(item.as_json_value());
                        }
                        (
                            Some(FactValue::List(normalized_values)),
                            Some(FactValue::List(json_values)),
                        )
                    } else {
                        for item in values {
                            json_values.push(item.as_json_value());
                        }
                        (None, Some(FactValue::List(json_values)))
                    }
                }
                RuntimeAttributeValue::Result(value) => (
                    include_attributes.then(|| normalized_fact_value(value)),
                    Some(value.as_json_value()),
                ),
                RuntimeAttributeValue::Plain(Some(value)) => {
                    let json_value = value.clone();
                    let value = if include_attributes {
                        Some(json_value.clone())
                    } else {
                        None
                    };
                    (value, Some(json_value))
                }
                RuntimeAttributeValue::Plain(None) => {
                    let json_value = self.default_value_by_index(index);
                    let value = if include_attributes {
                        json_value.clone()
                    } else {
                        None
                    };
                    (value, json_value)
                }
            };

            if let Some(attributes) = attributes.as_mut() {
                attributes.insert(key.clone(), value);
            }
            if let Some(value) = json_value {
                as_json.push((key.clone(), value));
            }
        }

        (attributes, as_json)
    }

    /// Возвращает нормализованную запись факта.
    ///
    /// Итог включает:
    /// - нормализованные значения полей;
    /// - `raw.as_json` для точной JSON-проекции рантайм-значений;
    /// - `raw.spans` для диапазонов исходного текста.
    #[inline]
    pub fn normalized(&self) -> FactRecord {
        let (attributes, as_json) = self.project_fields(true);
        let spans = self.compute_spans();

        *self.cached_as_json.borrow_mut() = Some(as_json.clone());
        *self.cached_spans.borrow_mut() = Some(spans.clone());

        FactRecord {
            scheme: self.scheme.clone(),
            attributes: attributes.expect("normalized projection must include attributes"),
            raw: Some(FactRecordRaw { as_json, spans }),
        }
    }

    /// Возвращает объединённые spans всех изменённых атрибутов.
    #[inline]
    pub fn spans(&self) -> Vec<Span> {
        self.cached_spans_cloned()
    }

    /// Возвращает JSON-подобное представление текущего рантайм-состояния.
    #[inline]
    pub fn as_json(&self) -> Vec<(String, FactValue)> {
        self.cached_as_json_cloned()
    }
}

fn normalized_fact_value(value: &InterpretationValue<'_>) -> FactValue {
    match value {
        InterpretationValue::Chain(chain) => FactValue::Str(chain.normalized()),
        InterpretationValue::FactResult(result) => result.normalized_value(),
        InterpretationValue::AttributeResult(result) => normalized_fact_value(&result.value),
        InterpretationValue::NormalizerResult(result) => match &result.value {
            super::normalizer::NormalizerResultValue::Value(value) => value.clone(),
            super::normalizer::NormalizerResultValue::Nested(value) => normalized_fact_value(value),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact;
    use crate::interpretation::attribute::attribute_with_default;
    use crate::interpretation::attribute::{attribute, AttributeScheme};
    use crate::interpretation::normalizer::{AttributeResult, Chain, FactResult};
    use crate::interpretation::{Interpretation, Transform};
    use crate::span::Span;
    use crate::token::{Token, TokenType};
    use std::borrow::Cow;

    #[test]
    fn test_fact_trait_for_typed_fact() {
        fact!(
            struct PersonFact {
                name: String,
                age: i64,
            }
        );

        let mut fact = PersonFact::default();
        assert!(fact.set_field("name", FactValue::Str("Иван".into())));
        assert!(fact.set_field("age", FactValue::Int(42)));

        assert_eq!(fact.fact_name(), "PersonFact");
        assert_eq!(fact.field_names(), &["name", "age"]);
        assert_eq!(fact.get_field("name"), Some(FactValue::Str("Иван".into())));
        assert_eq!(fact.get_field("age"), Some(FactValue::Int(42)));
    }

    #[test]
    fn test_fact_clone_box_for_typed_fact() {
        fact!(
            struct TraitFact {
                x: i64,
            }
        );

        let mut fact = TraitFact::default();
        assert!(fact.set_field("x", FactValue::Int(42)));

        let boxed: Box<dyn Fact> = fact.clone_box();
        assert_eq!(boxed.fact_name(), "TraitFact");
        assert_eq!(boxed.get_field("x"), Some(FactValue::Int(42)));
    }

    #[test]
    fn test_fact_attribute_creation() {
        let attr = FactAttribute::new("City", "name");
        assert_eq!(attr.fact_name(), "City");
        assert_eq!(attr.field_name(), "name");
        assert!(!attr.has_const());
    }

    #[test]
    fn test_fact_attribute_const() {
        let attr = FactAttribute::new("Street", "type").with_const("street");

        assert!(attr.has_const());
        assert_eq!(attr.get_const(), Some("street"));
    }

    #[test]
    fn test_fact_field() {
        let field: FactField<()> = FactField::new("test");
        assert_eq!(field.name(), "test");

        let cloned = field;
        assert_eq!(cloned.name(), "test");
    }

    #[test]
    fn test_field_type_string() {
        let s = String::from("test");
        let value = s.to_fact_value();

        assert_eq!(value, FactValue::Str("test".into()));

        let restored = String::from_fact_value(value);
        assert_eq!(restored, Some("test".into()));
    }

    #[test]
    fn test_field_type_i64() {
        let n: i64 = 42;
        let value = n.to_fact_value();

        assert_eq!(value, FactValue::Int(42));

        let restored = i64::from_fact_value(value);
        assert_eq!(restored, Some(42));
    }

    #[test]
    fn test_field_type_bool() {
        let b = true;
        let value = b.to_fact_value();

        assert_eq!(value, FactValue::Bool(true));

        let restored = bool::from_fact_value(value);
        assert_eq!(restored, Some(true));
    }

    #[test]
    fn test_field_type_mismatch() {
        let value = FactValue::Str("not a number".into());
        let result = i64::from_fact_value(value);
        assert_eq!(result, None);
    }

    fact! {
        pub struct City {
            name: String,
        }
    }

    #[test]
    fn test_fact_field_normalized() {
        let attr = City::name.normalized();
        assert_eq!(attr.attribute.fact().name(), "City");
        assert_eq!(attr.attribute.name(), "name");

        let interp: Interpretation = attr.into();
        assert_eq!(interp.fact_name, "City");
        assert_eq!(interp.field_name, "name");
        assert_eq!(interp.transforms.len(), 1);
        assert!(matches!(interp.transforms[0], Transform::Normalized));
    }

    #[test]
    fn test_fact_field_inflected() {
        static FORMS: &[&str] = &["city", "town"];
        let attr = City::name.inflected(FORMS);

        assert_eq!(attr.attribute.fact().name(), "City");
        assert_eq!(attr.attribute.name(), "name");
        assert!(attr.grams.contains("city"));
        assert!(attr.grams.contains("town"));

        let interp: Interpretation = attr.into();
        assert_eq!(interp.transforms.len(), 1);
        assert!(matches!(
            &interp.transforms[0],
            Transform::Inflected(forms)
                if forms.len() == 2
                && forms.iter().any(|f| f == "city")
                && forms.iter().any(|f| f == "town")
        ));
    }

    #[test]
    fn test_fact_field_custom() {
        let attr = City::name.custom(|s: FactValue| s.as_str().unwrap().to_uppercase());
        let interp: Interpretation = attr.into();
        assert!(matches!(interp.transforms[0], Transform::Custom(_)));
    }

    #[test]
    fn test_fact_field_repeatable() {
        let attr = City::name.repeatable();
        assert_eq!(attr.fact().name(), "City");
        assert_eq!(attr.name(), "name");

        let interp: Interpretation = attr.into();
        assert!(interp.repeatable);
        assert!(interp.transforms.is_empty());
    }

    #[test]
    fn test_fact_field_chained_transforms() {
        static FORMS: &[&str] = &["city"];

        let attr = City::name
            .inflected(FORMS)
            .custom(|s: String| s.to_uppercase());

        let interp: Interpretation = attr.into();
        assert!(!interp.repeatable);
        assert_eq!(interp.transforms.len(), 2);
        assert!(matches!(interp.transforms[0], Transform::Inflected(_)));
        assert!(matches!(interp.transforms[1], Transform::Custom(_)));
    }

    #[test]
    fn yargy_prepare_attribute_from_name() {
        let prepared = prepare_attribute("name");
        match prepared {
            PreparedAttributeScheme::Single(scheme) => {
                assert_eq!(scheme.name, "name");
                assert_eq!(scheme.default, None);
            }
            _ => panic!("expected single scheme"),
        }
    }

    #[test]
    fn yargy_fact_scheme_and_record_defaults() {
        let scheme = fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from(attribute("alias").repeatable()),
            ],
        );

        let record = scheme.try_new_record(BTreeMap::new()).unwrap();
        assert_eq!(record.get("name"), None);
        assert_eq!(record.get("alias"), Some(&FactValue::List(Vec::new())));
    }

    #[test]
    fn yargy_key_error_for_unknown_field() {
        let scheme = fact("City", vec![FactAttributeInput::from("name")]);
        let mut kwargs = BTreeMap::new();
        kwargs.insert("unknown".to_string(), FactValue::Str("x".to_string()));
        let err = scheme.try_new_record(kwargs).unwrap_err();
        assert_eq!(err, FactError::KeyError("unknown".to_string()));
    }

    #[test]
    fn yargy_interpretator_fact_normalized_and_as_json() {
        let scheme = fact("City", vec![FactAttributeInput::from("name")]);
        let mut runtime = InterpretatorFact::new(scheme.clone());

        let token = Token {
            value: Cow::Borrowed("Moscow"),
            span: Span::new(0, 6),
            token_type: TokenType::Latin,
        };

        let chain = Chain::new(vec![token], None);
        runtime
            .set(
                "name",
                InterpretationValue::AttributeResult(AttributeResult::new(chain.into())),
            )
            .unwrap();

        let normalized = runtime.normalized();
        assert_eq!(
            normalized.get("name"),
            Some(&FactValue::Str("Moscow".to_string()))
        );

        let as_json = normalized.as_json();
        assert_eq!(
            as_json,
            vec![("name".to_string(), FactValue::Str("Moscow".to_string()))]
        );
    }

    #[test]
    fn yargy_interpretator_fact_collects_spans() {
        let scheme = fact(
            "City",
            vec![FactAttributeInput::from(attribute("name").repeatable())],
        );
        let mut runtime = InterpretatorFact::new(scheme);

        let first = Token {
            value: Cow::Borrowed("Moscow"),
            span: Span::new(0, 6),
            token_type: TokenType::Russian,
        };
        let second = Token {
            value: Cow::Borrowed("Tver"),
            span: Span::new(7, 12),
            token_type: TokenType::Russian,
        };

        runtime
            .set(
                "name",
                InterpretationValue::Chain(Chain::new(vec![first], None)),
            )
            .unwrap();
        runtime
            .set(
                "name",
                InterpretationValue::Chain(Chain::new(vec![second], None)),
            )
            .unwrap();

        let spans = runtime.spans();
        assert_eq!(spans, vec![Span::new(0, 6), Span::new(7, 12)]);
    }

    #[test]
    fn yargy_interpretator_fact_preserves_fact_result_json_and_spans() {
        let scheme = fact("City", vec![FactAttributeInput::from("rank")]);
        let mut runtime = InterpretatorFact::new(scheme);

        let value = FactResult::from_json(FactValue::Int(7)).with_spans(vec![Span::new(5, 6)]);
        runtime
            .set("rank", InterpretationValue::FactResult(value))
            .unwrap();

        let normalized = runtime.normalized();
        assert_eq!(normalized.get("rank"), Some(&FactValue::Int(7)));
        assert_eq!(normalized.spans(), vec![Span::new(5, 6)]);
        assert_eq!(
            normalized.as_json(),
            vec![("rank".to_string(), FactValue::Int(7))]
        );
    }

    #[test]
    fn yargy_fact_record_preserves_values_and_defaults() {
        let scheme = fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from(attribute("alias").repeatable()),
            ],
        );

        let mut kwargs = BTreeMap::new();
        kwargs.insert("name".to_string(), FactValue::Str("Moscow".to_string()));
        let record = scheme.try_new_record(kwargs).unwrap();

        assert_eq!(record.name(), "City");
        assert_eq!(
            record.get("name"),
            Some(&FactValue::Str("Moscow".to_string()))
        );
        assert_eq!(record.get("alias"), Some(&FactValue::List(Vec::new())));
    }

    #[test]
    fn yargy_interpretator_fact_normalized_record_keeps_values() {
        let scheme = fact("City", vec![FactAttributeInput::from("name")]);
        let mut runtime = InterpretatorFact::new(scheme);

        let token = Token {
            value: Cow::Borrowed("Moscow"),
            span: Span::new(0, 6),
            token_type: TokenType::Latin,
        };

        runtime
            .set(
                "name",
                InterpretationValue::Chain(Chain::new(vec![token], None)),
            )
            .unwrap();

        let record = runtime.normalized();
        assert_eq!(record.name(), "City");
        assert_eq!(
            record.get("name"),
            Some(&FactValue::Str("Moscow".to_string()))
        );
    }

    #[test]
    fn yargy_field_helpers_convert_and_validate_types() {
        let value = field_to_value(&Some(5_i64));
        assert_eq!(value, Some(FactValue::Int(5)));

        let mut field: Option<i64> = None;
        assert!(set_field_value(&mut field, FactValue::Int(7)));
        assert_eq!(field, Some(7));

        assert!(!set_field_value(
            &mut field,
            FactValue::Str("bad".to_string())
        ));
        assert_eq!(field, Some(7));
    }

    #[test]
    fn yargy_fact_error_display() {
        let err = FactError::KeyError("name".to_string());
        assert_eq!(format!("{err}"), "KeyError(name)");
    }

    #[test]
    fn yargy_prepare_attribute_variants_and_constructed_accessors() {
        let single = prepare_attribute(AttributeScheme::new(
            "name",
            Some(FactValue::Str("x".to_string())),
        ));
        assert_eq!(single.name(), "name");
        let constructed_single = single.construct("City");
        assert_eq!(constructed_single.name(), "name");
        assert!(!constructed_single.is_repeatable());
        assert_eq!(
            constructed_single.default_value_ref(),
            Some(&FactValue::Str("x".to_string()))
        );

        let repeatable_scheme = AttributeScheme::new("alias", None).repeatable();
        let repeatable = prepare_attribute(repeatable_scheme.clone());
        assert_eq!(repeatable.name(), "alias");
        let constructed_repeatable = repeatable.construct("City");
        assert!(constructed_repeatable.is_repeatable());
        assert_eq!(
            constructed_repeatable.default_value(),
            Some(FactValue::List(Vec::new()))
        );
        assert_eq!(constructed_repeatable.default_value_ref(), None);

        let from_base = prepare_attribute(repeatable_scheme.as_base());
        assert!(matches!(from_base, PreparedAttributeScheme::Repeatable(_)));
    }

    #[test]
    fn yargy_fact_scheme_hierarchy_attribute_lookup_and_call() {
        let entity = FactScheme::new("Entity");
        let city = fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from(attribute_with_default(
                    "kind",
                    FactValue::Str("city".to_string()),
                )),
            ],
        )
        .with_parents(["Entity"]);

        assert!(city.is_subclass_of(&entity));
        assert!(city.attribute("name").is_some());
        assert!(city.attribute("missing").is_none());

        let mut kwargs = BTreeMap::new();
        kwargs.insert("name".to_string(), FactValue::Str("Moscow".to_string()));
        let record = city.call(kwargs).unwrap();
        assert_eq!(
            record.get("name"),
            Some(&FactValue::Str("Moscow".to_string()))
        );
        assert_eq!(
            record.get("kind"),
            Some(&FactValue::Str("city".to_string()))
        );
    }

    #[test]
    #[should_panic(expected = "duplicate fact attribute: name")]
    fn yargy_fact_panics_on_duplicate_attribute() {
        let _ = fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from("name"),
            ],
        );
    }

    #[test]
    fn yargy_fact_record_json_without_raw_and_sorted_spans_with_raw() {
        let scheme = fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from("rank"),
            ],
        );
        let mut kwargs = BTreeMap::new();
        kwargs.insert("name".to_string(), FactValue::Str("Moscow".to_string()));
        let record = scheme.try_new_record(kwargs).unwrap();
        assert_eq!(
            record.as_json(),
            vec![("name".to_string(), FactValue::Str("Moscow".to_string()))]
        );

        let raw_record = FactRecord {
            scheme: record.scheme.clone(),
            attributes: record.attributes.clone(),
            raw: Some(FactRecordRaw {
                as_json: vec![("name".to_string(), FactValue::Str("RAW".to_string()))],
                spans: vec![Span::new(7, 9), Span::new(1, 3)],
            }),
        };
        assert_eq!(
            raw_record.as_json(),
            vec![("name".to_string(), FactValue::Str("RAW".to_string()))]
        );
        assert_eq!(raw_record.spans(), vec![Span::new(1, 3), Span::new(7, 9)]);
    }

    #[test]
    fn yargy_interpretator_fact_new_set_error_merge_and_defaults() {
        let scheme = fact(
            "City",
            vec![
                FactAttributeInput::from(attribute_with_default(
                    "kind",
                    FactValue::Str("city".to_string()),
                )),
                FactAttributeInput::from("name"),
                FactAttributeInput::from(attribute("alias").repeatable()),
            ],
        );

        let mut runtime = InterpretatorFact::new(scheme.clone());
        assert!(runtime.is_repeatable("alias"));
        assert!(matches!(
            runtime.get_runtime("kind"),
            Some(RuntimeAttributeValue::Plain(None))
        ));
        assert!(matches!(
            runtime.get_runtime("alias"),
            Some(RuntimeAttributeValue::Repeatable(values)) if values.is_empty()
        ));

        let bad = runtime.set(
            "unknown",
            InterpretationValue::Chain(Chain::new(Vec::new(), None)),
        );
        assert_eq!(bad, Err(FactError::KeyError("unknown".to_string())));

        let token = Token {
            value: Cow::Borrowed("Moscow"),
            span: Span::new(0, 6),
            token_type: TokenType::Latin,
        };
        runtime
            .set(
                "name",
                InterpretationValue::Chain(Chain::new(vec![token], None)),
            )
            .unwrap();

        let token_alias = Token {
            value: Cow::Borrowed("Capital"),
            span: Span::new(7, 14),
            token_type: TokenType::Latin,
        };
        let mut other = InterpretatorFact::new(scheme);
        other
            .set(
                "alias",
                InterpretationValue::Chain(Chain::new(vec![token_alias], None)),
            )
            .unwrap();

        runtime.merge(&other);
        let normalized = runtime.normalized();
        assert_eq!(
            normalized.get("kind"),
            Some(&FactValue::Str("city".to_string()))
        );
        assert_eq!(
            normalized.get("name"),
            Some(&FactValue::Str("Moscow".to_string()))
        );
        assert_eq!(
            normalized.get("alias"),
            Some(&FactValue::List(vec![FactValue::Str(
                "Capital".to_string()
            )]))
        );
    }

    #[test]
    fn yargy_interpretator_fact_invalidates_cached_projections_after_set() {
        let scheme = fact("City", vec![FactAttributeInput::from("name")]);
        let mut runtime = InterpretatorFact::new(scheme);

        assert_eq!(runtime.as_json(), Vec::new());
        assert!(runtime.spans().is_empty());

        let token = Token {
            value: Cow::Borrowed("Moscow"),
            span: Span::new(0, 6),
            token_type: TokenType::Latin,
        };
        runtime
            .set(
                "name",
                InterpretationValue::Chain(Chain::new(vec![token], None)),
            )
            .unwrap();

        assert_eq!(
            runtime.as_json(),
            vec![("name".to_string(), FactValue::Str("Moscow".to_string()))]
        );
        assert_eq!(runtime.spans(), vec![Span::new(0, 6)]);
    }
}
