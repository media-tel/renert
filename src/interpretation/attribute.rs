//! API атрибутов.
//!
//! Модуль описывает:
//! - ссылки на факты и иерархию фактов ([`FactRef`](crate::interpretation::attribute::FactRef));
//! - схемы атрибутов ([`AttributeScheme`](crate::interpretation::attribute::AttributeScheme), [`RepeatableAttributeScheme`](crate::interpretation::attribute::RepeatableAttributeScheme));
//! - рантайм-представление атрибутов ([`Attribute`](crate::interpretation::attribute::Attribute), [`RepeatableAttribute`](crate::interpretation::attribute::RepeatableAttribute));
//! - композиции атрибутов с нормализаторами.
//!
//! Основная цель API: сохранить привычную семантику `yargy` и при этом
//! явно моделировать в Rust различия между обычными и repeatable-атрибутами.
use std::collections::BTreeSet;
use std::sync::Arc;

use super::normalizer::{
    ConstNormalizer, FunctionFunctionNormalizer, FunctionNormalizer, InflectedNormalizer,
    MorphFunctionNormalizer, NormalizedNormalizer,
};
use super::FactValue;

const DEFAULT_INFLECTED_GRAMS: [&str; 2] = ["nomn", "sing"];

#[inline]
fn default_inflected_grams() -> BTreeSet<String> {
    DEFAULT_INFLECTED_GRAMS
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

/// Ссылка на тип факта и его родительские типы.
///
/// `lineage` хранит замыкание по именам классов фактов, что позволяет
/// проверять отношение «подкласс/родитель» через [`FactRef::is_subclass_of`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FactRef {
    name: String,
    lineage: Arc<BTreeSet<String>>,
}

impl FactRef {
    /// Создаёт ссылку на факт без дополнительных родителей.
    ///
    /// Идентификатор факта автоматически добавляется в его lineage.
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut lineage = BTreeSet::new();
        lineage.insert(name.clone());
        Self {
            name,
            lineage: Arc::new(lineage),
        }
    }

    /// Создаёт ссылку на факт с явным списком родительских фактов.
    #[inline]
    pub fn with_parents<I, S>(name: impl Into<String>, parents: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let name = name.into();
        let mut lineage = BTreeSet::new();
        lineage.insert(name.clone());
        for parent in parents {
            lineage.insert(parent.into());
        }
        Self {
            name,
            lineage: Arc::new(lineage),
        }
    }

    /// Возвращает имя факта.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Извлекает имя факта без дополнительного клонирования строки.
    #[inline]
    pub fn into_name(self) -> String {
        self.name
    }

    /// Проверяет, является ли текущий факт подклассом `other`.
    ///
    /// Возвращает `true`, если lineage текущего факта содержит имя `other`.
    #[inline]
    pub fn is_subclass_of(&self, other: &FactRef) -> bool {
        self.lineage.contains(other.name())
    }
}

impl From<&str> for FactRef {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FactRef {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Базовая схема атрибута, используемая при `prepare_attribute`.
///
/// В Python yargy роль базового класса играет иерархия наследования.
/// В Rust тип схемы фиксируется явно через enum, чтобы не терять
/// различия между обычным и repeatable-атрибутом.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeSchemeBase {
    /// Обычный (неповторяемый) атрибут.
    Single(AttributeScheme),
    /// Repeatable-атрибут.
    Repeatable(RepeatableAttributeScheme),
}

impl AttributeSchemeBase {
    /// Возвращает имя атрибута без учёта его вида.
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            AttributeSchemeBase::Single(scheme) => &scheme.name,
            AttributeSchemeBase::Repeatable(scheme) => &scheme.name,
        }
    }
}

/// Схема неповторяемого атрибута с опциональным значением по умолчанию.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeScheme {
    /// Имя атрибута.
    pub name: String,
    /// Значение по умолчанию (если задано).
    pub default: Option<FactValue>,
}

impl AttributeScheme {
    /// Создаёт схему обычного атрибута.
    #[inline]
    pub fn new(name: impl Into<String>, default: Option<FactValue>) -> Self {
        Self {
            name: name.into(),
            default,
        }
    }

    /// Преобразует схему в repeatable-вариант.
    ///
    /// Паника, если у схемы задано значение по умолчанию.
    #[inline]
    pub fn repeatable(&self) -> RepeatableAttributeScheme {
        RepeatableAttributeScheme::new(self)
    }

    /// Строит атрибут для конкретного факта.
    #[inline]
    pub fn construct(&self, fact: impl Into<FactRef>) -> Attribute {
        Attribute::new(fact, self.name.clone(), self.default.clone())
    }

    /// Оборачивает схему в общий enum-представитель.
    #[inline]
    pub fn as_base(&self) -> AttributeSchemeBase {
        AttributeSchemeBase::Single(self.clone())
    }
}

/// Схема repeatable-атрибута.
///
/// Для repeatable-атрибутов значение по умолчанию запрещено.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatableAttributeScheme {
    /// Имя атрибута.
    pub name: String,
}

impl RepeatableAttributeScheme {
    /// Создаёт repeatable-схему из обычной схемы.
    ///
    /// Паника, если у исходной схемы задан `default`.
    #[inline]
    pub fn new(attribute: &AttributeScheme) -> Self {
        assert!(
            attribute.default.is_none(),
            "repeatable attribute scheme must not have default value"
        );
        Self {
            name: attribute.name.clone(),
        }
    }

    /// Строит repeatable-атрибут для конкретного факта.
    #[inline]
    pub fn construct(&self, fact: impl Into<FactRef>) -> RepeatableAttribute {
        RepeatableAttribute::new(fact, self.name.clone())
    }

    /// Оборачивает схему в общий enum-представитель.
    #[inline]
    pub fn as_base(&self) -> AttributeSchemeBase {
        AttributeSchemeBase::Repeatable(self.clone())
    }
}

/// Базовые данные атрибута: факт и имя поля.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeBase {
    /// Факт, которому принадлежит атрибут.
    pub fact: FactRef,
    /// Имя поля атрибута.
    pub name: String,
}

impl AttributeBase {
    /// Создаёт базовую часть атрибута.
    #[inline]
    pub fn new(fact: impl Into<FactRef>, name: impl Into<String>) -> Self {
        Self {
            fact: fact.into(),
            name: name.into(),
        }
    }

    /// Возвращает подпись атрибута в формате `Fact.field`.
    #[inline]
    pub fn label(&self) -> String {
        let fact_name = self.fact.name();
        let mut label = String::with_capacity(fact_name.len() + 1 + self.name.len());
        label.push_str(fact_name);
        label.push('.');
        label.push_str(&self.name);
        label
    }
}

/// Обычный (неповторяемый) атрибут факта.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    /// Базовые данные атрибута.
    pub base: AttributeBase,
    /// Значение по умолчанию.
    pub default: Option<FactValue>,
}

impl Attribute {
    /// Создаёт атрибут.
    #[inline]
    pub fn new(
        fact: impl Into<FactRef>,
        name: impl Into<String>,
        default: Option<FactValue>,
    ) -> Self {
        Self {
            base: AttributeBase::new(fact, name),
            default,
        }
    }

    /// Возвращает базовую часть атрибута.
    #[inline]
    pub fn base(&self) -> &AttributeBase {
        &self.base
    }

    /// Возвращает ссылку на факт атрибута.
    #[inline]
    pub fn fact(&self) -> &FactRef {
        &self.base.fact
    }

    /// Возвращает имя атрибута.
    #[inline]
    pub fn name(&self) -> &str {
        &self.base.name
    }

    /// Возвращает подпись атрибута `Fact.field`.
    #[inline]
    pub fn label(&self) -> String {
        self.base.label()
    }

    /// Возвращает клонированное значение по умолчанию.
    #[inline]
    pub fn default_value(&self) -> Option<FactValue> {
        self.default_value_ref().cloned()
    }

    /// Возвращает ссылку на значение по умолчанию.
    #[inline]
    pub fn default_value_ref(&self) -> Option<&FactValue> {
        self.default.as_ref()
    }

    /// Python-совместимый вариант `inflected()` с дефолтными граммемами
    /// `{"nomn", "sing"}`.
    #[inline]
    pub fn inflected(self) -> InflectedAttribute {
        InflectedAttribute::new(self, default_inflected_grams())
    }

    /// Явный Rust-вариант `inflected(...)` с заданным набором граммем.
    #[inline]
    pub fn inflected_with<I, S>(self, grams: I) -> InflectedAttribute
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        InflectedAttribute::new(self, grams.into_iter().map(Into::into).collect())
    }

    /// Строит морфологическую трансформацию `attribute.normalized()`.
    #[inline]
    pub fn normalized(self) -> NormalizedAttribute {
        NormalizedAttribute::new(self)
    }

    /// Строит константную трансформацию `attribute.const(value)`.
    ///
    /// Используется raw-идентификатор `r#const`, чтобы сохранить совместимость
    /// с Python-стилем API и не конфликтовать с ключевым словом Rust.
    #[inline]
    pub fn r#const<T>(self, value: T) -> ConstAttribute<T> {
        ConstAttribute::new(self, value)
    }

    /// Строит пользовательскую трансформацию `attribute.custom(function)`.
    #[inline]
    pub fn custom<F>(self, function: F) -> FunctionAttribute<F> {
        FunctionAttribute::new(self, function)
    }
}

/// Repeatable-атрибут факта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatableAttribute {
    /// Базовые данные атрибута.
    pub base: AttributeBase,
}

impl RepeatableAttribute {
    /// Создаёт repeatable-атрибут.
    #[inline]
    pub fn new(fact: impl Into<FactRef>, name: impl Into<String>) -> Self {
        Self {
            base: AttributeBase::new(fact, name),
        }
    }

    /// Возвращает базовую часть атрибута.
    #[inline]
    pub fn base(&self) -> &AttributeBase {
        &self.base
    }

    /// Возвращает ссылку на факт атрибута.
    #[inline]
    pub fn fact(&self) -> &FactRef {
        &self.base.fact
    }

    /// Возвращает имя атрибута.
    #[inline]
    pub fn name(&self) -> &str {
        &self.base.name
    }

    /// Возвращает подпись атрибута `Fact.field`.
    #[inline]
    pub fn label(&self) -> String {
        self.base.label()
    }
}

/// Вид интерпретатора, который требуется для конкретного атрибута.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeInterpretatorKind {
    /// Достаточно только присвоения атрибута.
    AttributeOnly,
    /// Нужна связка «атрибут + нормализатор».
    AttributeAndNormalizer,
}

/// Трейт для определения вида требуемого интерпретатора атрибута.
pub trait InterpretatorDispatch {
    /// Возвращает категорию обработки атрибута в пайплайне интерпретации.
    fn interpretator_kind(&self) -> AttributeInterpretatorKind;
}

/// Маркерный трейт для атрибутов с морфологической нормализацией.
pub trait MorphAttribute: InterpretatorDispatch {
    /// Возвращает исходный атрибут.
    fn attribute(&self) -> &Attribute;

    /// Возвращает подпись атрибута `Fact.field`.
    #[inline]
    fn label(&self) -> String {
        self.attribute().label()
    }
}

/// Маркерный трейт для атрибутов с пользовательской нормализацией.
pub trait CustomAttribute: InterpretatorDispatch {
    /// Возвращает исходный атрибут.
    fn attribute(&self) -> &Attribute;

    /// Возвращает подпись атрибута `Fact.field`.
    #[inline]
    fn label(&self) -> String {
        self.attribute().label()
    }
}

impl InterpretatorDispatch for Attribute {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeOnly
    }
}

impl InterpretatorDispatch for RepeatableAttribute {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeOnly
    }
}

/// Морфологическая трансформация `attribute.inflected(...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct InflectedAttribute {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Множество граммем для инфлексии.
    pub grams: BTreeSet<String>,
}

impl InflectedAttribute {
    /// Создаёт инфлексированное представление атрибута.
    #[inline]
    pub fn new(attribute: Attribute, grams: BTreeSet<String>) -> Self {
        Self { attribute, grams }
    }

    /// Добавляет пользовательскую функцию после морфологического шага.
    #[inline]
    pub fn custom<F>(self, function: F) -> InflectedFunctionAttribute<F> {
        InflectedFunctionAttribute::new(self.attribute, self.grams, function)
    }

    /// Преобразует атрибут в соответствующий нормализатор.
    #[inline]
    pub fn normalizer(self) -> InflectedNormalizer {
        InflectedNormalizer::new(Some(self.grams.into_iter().collect()))
    }
}

impl InterpretatorDispatch for InflectedAttribute {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl MorphAttribute for InflectedAttribute {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Морфологическая трансформация `attribute.normalized()`.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedAttribute {
    /// Исходный атрибут.
    pub attribute: Attribute,
}

impl NormalizedAttribute {
    /// Создаёт нормализованное представление атрибута.
    #[inline]
    pub fn new(attribute: Attribute) -> Self {
        Self { attribute }
    }

    /// Добавляет пользовательскую функцию после `normalized()`.
    #[inline]
    pub fn custom<F>(self, function: F) -> NormalizedFunctionAttribute<F> {
        NormalizedFunctionAttribute::new(self.attribute, function)
    }

    /// Преобразует атрибут в соответствующий нормализатор.
    #[inline]
    pub fn normalizer(self) -> NormalizedNormalizer {
        NormalizedNormalizer
    }
}

impl InterpretatorDispatch for NormalizedAttribute {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl MorphAttribute for NormalizedAttribute {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Пользовательская трансформация `attribute.const(value)`.
pub struct ConstAttribute<T> {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Константное значение.
    pub value: T,
}

impl<T> ConstAttribute<T> {
    /// Создаёт константное представление атрибута.
    #[inline]
    pub fn new(attribute: Attribute, value: T) -> Self {
        Self { attribute, value }
    }

    /// Преобразует атрибут в константный нормализатор.
    #[inline]
    pub fn normalizer(self) -> ConstNormalizer<T> {
        ConstNormalizer::new(self.value)
    }
}

impl<T> InterpretatorDispatch for ConstAttribute<T> {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl<T> CustomAttribute for ConstAttribute<T> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Пользовательская трансформация `attribute.custom(function)`.
pub struct FunctionAttribute<F> {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Функция преобразования.
    pub function: F,
}

impl<F> FunctionAttribute<F> {
    /// Создаёт пользовательское представление атрибута.
    #[inline]
    pub fn new(attribute: Attribute, function: F) -> Self {
        Self {
            attribute,
            function,
        }
    }

    /// Строит композицию `custom(first).custom(second)`.
    #[inline]
    pub fn custom<G>(self, function: G) -> FunctionFunctionAttribute<F, G> {
        FunctionFunctionAttribute::new(self.attribute, self.function, function)
    }

    /// Преобразует атрибут в пользовательский нормализатор.
    #[inline]
    pub fn normalizer(self) -> FunctionNormalizer<F> {
        FunctionNormalizer::new(self.function)
    }
}

impl<F> InterpretatorDispatch for FunctionAttribute<F> {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl<F> CustomAttribute for FunctionAttribute<F> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Композиция `attribute.custom(first).custom(second)`.
pub struct FunctionFunctionAttribute<F, G> {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Первая функция в цепочке.
    pub first: F,
    /// Вторая функция в цепочке.
    pub second: G,
}

impl<F, G> FunctionFunctionAttribute<F, G> {
    /// Создаёт цепочку из двух пользовательских функций.
    #[inline]
    pub fn new(attribute: Attribute, first: F, second: G) -> Self {
        Self {
            attribute,
            first,
            second,
        }
    }

    /// Преобразует атрибут в составной нормализатор.
    #[inline]
    pub fn normalizer(self) -> FunctionFunctionNormalizer<F, G> {
        FunctionFunctionNormalizer::new(self.first, self.second)
    }
}

impl<F, G> InterpretatorDispatch for FunctionFunctionAttribute<F, G> {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl<F, G> CustomAttribute for FunctionFunctionAttribute<F, G> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Композиция `attribute.inflected(...).custom(function)`.
pub struct InflectedFunctionAttribute<F> {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Граммемы морфологического шага.
    pub grams: BTreeSet<String>,
    /// Пользовательская функция, применяемая после инфлексии.
    pub function: F,
}

impl<F> InflectedFunctionAttribute<F> {
    /// Создаёт морфологически-пользовательское представление атрибута.
    #[inline]
    pub fn new(attribute: Attribute, grams: BTreeSet<String>, function: F) -> Self {
        Self {
            attribute,
            grams,
            function,
        }
    }

    /// Преобразует атрибут в морфологически-пользовательский нормализатор.
    #[inline]
    pub fn normalizer(self) -> MorphFunctionNormalizer<InflectedNormalizer, F> {
        MorphFunctionNormalizer::new(
            InflectedNormalizer::new(Some(self.grams.into_iter().collect())),
            self.function,
        )
    }
}

impl<F> InterpretatorDispatch for InflectedFunctionAttribute<F> {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl<F> MorphAttribute for InflectedFunctionAttribute<F> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

impl<F> CustomAttribute for InflectedFunctionAttribute<F> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Композиция `attribute.normalized().custom(function)`.
pub struct NormalizedFunctionAttribute<F> {
    /// Исходный атрибут.
    pub attribute: Attribute,
    /// Пользовательская функция, применяемая после `normalized()`.
    pub function: F,
}

impl<F> NormalizedFunctionAttribute<F> {
    /// Создаёт представление для цепочки `normalized().custom(...)`.
    #[inline]
    pub fn new(attribute: Attribute, function: F) -> Self {
        Self {
            attribute,
            function,
        }
    }

    /// Преобразует атрибут в морфологически-пользовательский нормализатор.
    #[inline]
    pub fn normalizer(self) -> MorphFunctionNormalizer<NormalizedNormalizer, F> {
        MorphFunctionNormalizer::new(NormalizedNormalizer, self.function)
    }
}

impl<F> InterpretatorDispatch for NormalizedFunctionAttribute<F> {
    fn interpretator_kind(&self) -> AttributeInterpretatorKind {
        AttributeInterpretatorKind::AttributeAndNormalizer
    }
}

impl<F> MorphAttribute for NormalizedFunctionAttribute<F> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

impl<F> CustomAttribute for NormalizedFunctionAttribute<F> {
    fn attribute(&self) -> &Attribute {
        &self.attribute
    }
}

/// Хелпер, эквивалентный Python-вызову `attribute(name, default=None)`.
#[inline]
pub fn attribute(name: impl Into<String>) -> AttributeScheme {
    AttributeScheme::new(name, None)
}

/// Хелпер для создания схемы атрибута с явным значением по умолчанию.
#[inline]
pub fn attribute_with_default(name: impl Into<String>, default: FactValue) -> AttributeScheme {
    AttributeScheme::new(name, Some(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::normalizer::{
        AttributeResult, Chain, InterpretationValue, NormalizerResult,
    };
    use crate::token::Tokenizer;

    #[test]
    fn fact_ref_supports_subclass_check() {
        let city = FactRef::new("City");
        let capital = FactRef::with_parents("Capital", ["City"]);
        assert!(capital.is_subclass_of(&city));
        assert!(!city.is_subclass_of(&capital));
    }

    #[test]
    fn scheme_construct_and_repeatable_work() {
        let scheme = AttributeScheme::new("name", None);
        let attr = scheme.construct("City");
        assert_eq!(attr.label(), "City.name");

        let repeatable = scheme.repeatable();
        let rep_attr = repeatable.construct("City");
        assert_eq!(rep_attr.label(), "City.name");
    }

    #[test]
    #[should_panic(expected = "repeatable attribute scheme must not have default value")]
    fn repeatable_panics_on_non_empty_default() {
        let scheme = AttributeScheme::new("name", Some(FactValue::Str("x".into())));
        let _ = scheme.repeatable();
    }

    #[test]
    fn normalized_and_inflected_normalizers_are_accessible() {
        let attr = Attribute::new("City", "name", None);

        let normalized = attr.clone().normalized().normalizer();
        assert_eq!(normalized.label(), "normalized()");

        let inflected = attr.inflected().normalizer();
        assert_eq!(inflected.label(), "inflected(nomn, sing)");
    }

    #[test]
    fn function_function_attribute_normalizer_uses_runtime_normalized_bridge() {
        let attr = Attribute::new("Number", "value", None);
        let normalizer = attr
            .custom(|s: FactValue| s.as_str().unwrap().parse::<i64>().unwrap())
            .custom(|n| n + 1)
            .normalizer();

        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("1");
        let chain = Chain::new(tokens, None);
        let value = AttributeResult::new(InterpretationValue::Chain(chain));

        assert_eq!(normalizer.call(&value), 2);
    }

    #[test]
    fn fact_ref_from_conversions_and_name_work() {
        let from_str: FactRef = "City".into();
        let from_string: FactRef = "City".to_string().into();

        assert_eq!(from_str.name(), "City");
        assert_eq!(from_string.name(), "City");
        assert_eq!(from_str, from_string);
    }

    #[test]
    fn attribute_scheme_base_name_and_as_base_work() {
        let scheme = AttributeScheme::new("name", None);
        let single_base = scheme.as_base();
        assert_eq!(single_base.name(), "name");
        assert!(matches!(single_base, AttributeSchemeBase::Single(_)));

        let repeatable = scheme.repeatable();
        let repeatable_base = repeatable.as_base();
        assert_eq!(repeatable_base.name(), "name");
        assert!(matches!(
            repeatable_base,
            AttributeSchemeBase::Repeatable(_)
        ));
    }

    #[test]
    fn attribute_base_and_attribute_accessors_work() {
        let base = AttributeBase::new("City", "name");
        assert_eq!(base.label(), "City.name");

        let attr = Attribute::new("City", "name", Some(FactValue::Str("def".to_string())));
        assert_eq!(attr.base().label(), "City.name");
        assert_eq!(attr.fact().name(), "City");
        assert_eq!(attr.name(), "name");
        assert_eq!(attr.label(), "City.name");
        assert_eq!(
            attr.default_value_ref(),
            Some(&FactValue::Str("def".to_string()))
        );
        assert_eq!(
            attr.default_value(),
            Some(FactValue::Str("def".to_string()))
        );
    }

    #[test]
    fn repeatable_attribute_accessors_and_dispatch_kind_work() {
        let repeatable = RepeatableAttribute::new("City", "alias");
        assert_eq!(repeatable.base().label(), "City.alias");
        assert_eq!(repeatable.fact().name(), "City");
        assert_eq!(repeatable.name(), "alias");
        assert_eq!(repeatable.label(), "City.alias");
        assert_eq!(
            repeatable.interpretator_kind(),
            AttributeInterpretatorKind::AttributeOnly
        );
    }

    #[test]
    fn attribute_transform_builders_and_kinds_work() {
        let attr = Attribute::new("City", "name", None);
        assert_eq!(
            attr.interpretator_kind(),
            AttributeInterpretatorKind::AttributeOnly
        );

        let inflected = attr.clone().inflected_with(["sing", "nomn", "sing"]);
        assert_eq!(
            inflected.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(MorphAttribute::label(&inflected), "City.name");
        assert_eq!(inflected.normalizer().label(), "inflected(nomn, sing)");

        let normalized = attr.clone().normalized();
        assert_eq!(
            normalized.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(MorphAttribute::label(&normalized), "City.name");
        assert_eq!(normalized.normalizer().label(), "normalized()");
    }

    #[test]
    fn const_and_function_attributes_expose_custom_trait_and_normalizers() {
        let attr = Attribute::new("City", "rank", None);

        let const_attr = attr.clone().r#const(42_i64);
        assert_eq!(
            const_attr.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(CustomAttribute::label(&const_attr), "City.rank");
        assert_eq!(const_attr.normalizer().call(()), 42);

        let function_attr = attr.custom(|value: FactValue| value.as_int().unwrap_or_default() + 1);
        assert_eq!(
            function_attr.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(CustomAttribute::label(&function_attr), "City.rank");
        assert_eq!(
            function_attr
                .normalizer()
                .call(&NormalizerResult::from_value(1)),
            2
        );
    }

    #[test]
    fn function_function_attribute_normalizer_chains_calls() {
        let attr = Attribute::new("Number", "value", None);
        let normalizer = attr
            .custom(|value: FactValue| value.as_int().unwrap_or_default() + 1)
            .custom(|value| value * 2)
            .normalizer();

        assert_eq!(normalizer.call(&NormalizerResult::from_value(3)), 8);
    }

    #[test]
    fn morph_function_attributes_keep_labels_and_kinds() {
        let attr = Attribute::new("City", "name", None);

        let inflected_custom = attr
            .clone()
            .inflected_with(["nomn", "sing"])
            .custom(|value: String| value.to_uppercase());
        assert_eq!(
            inflected_custom.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(MorphAttribute::label(&inflected_custom), "City.name");
        assert_eq!(CustomAttribute::label(&inflected_custom), "City.name");
        assert_eq!(
            inflected_custom.normalizer().label(),
            "inflected(nomn, sing).<lambda>"
        );

        let normalized_custom = attr.normalized().custom(|value: String| value.len());
        assert_eq!(
            normalized_custom.interpretator_kind(),
            AttributeInterpretatorKind::AttributeAndNormalizer
        );
        assert_eq!(MorphAttribute::label(&normalized_custom), "City.name");
        assert_eq!(CustomAttribute::label(&normalized_custom), "City.name");
        assert_eq!(
            normalized_custom.normalizer().label(),
            "normalized().<lambda>"
        );
    }

    #[test]
    fn helper_attribute_functions_build_expected_schemes() {
        let plain = attribute("field");
        assert_eq!(plain.name, "field");
        assert_eq!(plain.default, None);

        let with_default = attribute_with_default("flag", FactValue::Bool(true));
        assert_eq!(with_default.name, "flag");
        assert_eq!(with_default.default, Some(FactValue::Bool(true)));
    }
}
