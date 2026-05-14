//! Нормализаторы и рантайм-значения интерпретации в стиле `yargy`.
//!
//! Модуль повторяет форму API `yargy.interpretation.normalizer` и содержит:
//! - морфологические нормализаторы (`NormalizedNormalizer`, `InflectedNormalizer`);
//! - пользовательские нормализаторы (`ConstNormalizer`, `FunctionNormalizer`);
//! - композиции нормализаторов (`FunctionFunctionNormalizer`, `MorphFunctionNormalizer`);
//! - унифицированные рантайм-обёртки значений (`InterpretationValue` и связанные типы).
//!
//! Ключевая идея: любой промежуточный результат интерпретации может быть
//! приведён к текстовому (`normalized`) и JSON-подобному (`as_json`) представлению,
//! а также, при необходимости, сохранить исходные диапазоны (`spans`) в тексте.

use std::any::type_name;
use std::fmt;

use super::attribute::Attribute as YargyAttribute;
use super::fact::InterpretatorFact;
use super::FactValue;
use crate::span::Span;
use crate::token::{
    get_tokens_span, join_inflected_tokens_ref, join_normalized_tokens, join_tokens, Token,
};

/// Базовый маркерный трейт для всех нормализаторов.
pub trait Normalizer {}

/// Маркерный трейт для нормализаторов, опирающихся на морфологию токенов.
pub trait MorphNormalizer: Normalizer {}

/// Маркерный трейт для пользовательских нормализаторов и константных значений.
pub trait CustomNormalizer: Normalizer {}

/// Вход, который можно преобразовать в нормализованное значение.
pub trait NormalizedItem {
    /// Возвращает нормализованное представление в виде [`FactValue`].
    fn normalized_value(&self) -> FactValue;

    /// Упрощённый доступ к `normalized_value()` как к строке.
    #[inline]
    fn normalized(&self) -> String {
        self.normalized_value().to_string()
    }
}

/// Вход, поддерживающий yargy-подобные проекции `as_json` и `spans`.
pub trait RuntimeItem {
    /// Возвращает JSON-подобное представление значения.
    fn as_json_value(&self) -> FactValue;
    /// Возвращает диапазоны исходного текста, связанные со значением.
    fn spans(&self) -> Vec<Span>;
}

pub(crate) trait RuntimeSpans {
    fn append_spans(&self, out: &mut Vec<Span>);
}

#[inline]
fn runtime_fact_as_json_value(fact: &InterpretatorFact<'_>) -> FactValue {
    fact.cached_as_json_object_cloned()
}

/// Цепочка токенов, используемая морфологическими нормализаторами.
#[derive(Debug, Clone)]
pub struct Chain<'a> {
    /// Токены, из которых построено значение.
    pub tokens: Vec<Token<'a>>,
    /// Необязательный заранее вычисленный ключ (приоритетнее токенов).
    pub key: Option<String>,
}

impl<'a> Chain<'a> {
    /// Создаёт новую цепочку токенов.
    #[inline]
    pub fn new(tokens: Vec<Token<'a>>, key: Option<String>) -> Self {
        Self { tokens, key }
    }

    /// Склеивает исходные токены в строку.
    #[inline]
    pub fn normalized(&self) -> String {
        join_tokens(self.tokens.iter())
    }
}

impl NormalizedItem for Chain<'_> {
    fn normalized_value(&self) -> FactValue {
        FactValue::Str(join_tokens(self.tokens.iter()))
    }
}

impl RuntimeItem for Chain<'_> {
    fn as_json_value(&self) -> FactValue {
        FactValue::Str(self.normalized())
    }

    fn spans(&self) -> Vec<Span> {
        let mut out = Vec::with_capacity(1);
        self.append_spans(&mut out);
        out
    }
}

impl RuntimeSpans for Chain<'_> {
    fn append_spans(&self, out: &mut Vec<Span>) {
        if let Some(span) = get_tokens_span(&self.tokens) {
            out.push(span);
        }
    }
}

/// Результат интерпретации факта в рантайм-пайплайне.
///
/// Этот тип является транспортной обёрткой для fact-ветки внутри
/// [`InterpretationValue`]. Он нужен по двум причинам:
/// - представлять факт как обычное значение интерпретации, совместимое с
///   `normalized/as_json/spans`;
/// - сохранять текущий публичный API, в котором факт может быть представлен
///   как готовый JSON-подобный снимок, строковая нормализация или полноценный
///   [`InterpretatorFact`] для ленивой материализации.
///
/// На текущем этапе `FactResult` остаётся compatibility-слоем: внутри него
/// всё ещё сосуществуют несколько форм одного результата. Поэтому код вне
/// этого модуля должен обращаться к нему через методы `normalized_value`,
/// `as_json_value`, `spans`, `runtime_fact` и `into_runtime_fact`, а не через
/// прямой разбор полей.
#[derive(Debug, Clone)]
pub struct FactResult<'a> {
    /// Строковое нормализованное представление.
    ///
    /// Для runtime-фактов поле может оставаться пустым: в этом случае публичные
    /// accessors материализуют нужную проекцию из `runtime_fact` лениво.
    pub normalized: String,
    /// Кэш JSON-представления, если оно уже вычислено.
    ///
    /// Для runtime-фактов может быть пустым до первого обращения через
    /// `as_json_value()`/`normalized_value()`.
    pub as_json: Option<FactValue>,
    /// Кэш диапазонов исходного текста.
    pub spans: Vec<Span>,
    /// Полный рантайм-факт (используется для ленивых проекций).
    pub runtime_fact: Option<Box<InterpretatorFact<'a>>>,
}

impl<'a> FactResult<'a> {
    /// Создаёт результат из нормализованной строки.
    #[inline]
    pub fn new<S>(normalized: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            normalized: normalized.into(),
            as_json: None,
            spans: Vec::new(),
            runtime_fact: None,
        }
    }

    /// Создаёт результат из готового JSON-подобного значения.
    #[inline]
    pub fn from_json(value: FactValue) -> Self {
        Self {
            normalized: value.to_string(),
            as_json: Some(value),
            spans: Vec::new(),
            runtime_fact: None,
        }
    }

    /// Устанавливает диапазоны исходного текста.
    #[inline]
    pub fn with_spans(mut self, spans: Vec<Span>) -> Self {
        self.spans = spans;
        self
    }

    /// Устанавливает JSON-представление и синхронизирует `normalized`.
    #[inline]
    pub fn with_json(mut self, value: FactValue) -> Self {
        self.normalized = value.to_string();
        self.as_json = Some(value);
        self
    }

    /// Создаёт результат из полноценного рантайм-факта.
    ///
    /// Проекции `normalized` и `spans` остаются ленивыми и при необходимости
    /// вычисляются через `runtime_fact`.
    #[inline]
    pub fn from_runtime_fact(fact: InterpretatorFact<'a>) -> Self {
        Self {
            normalized: String::new(),
            as_json: None,
            spans: Vec::new(),
            runtime_fact: Some(Box::new(fact)),
        }
    }
}

impl<'a> FactResult<'a> {
    /// Возвращает ссылку на вложенный рантайм-факт, если результат был
    /// построен через [`Self::from_runtime_fact`].
    ///
    /// Это основной способ безопасно читать runtime-представление факта
    /// из внешних модулей без привязки к внутреннему устройству `FactResult`.
    #[inline]
    pub fn runtime_fact(&self) -> Option<&InterpretatorFact<'a>> {
        self.runtime_fact.as_deref()
    }

    /// Извлекает вложенный рантайм-факт, если результат был построен через
    /// [`Self::from_runtime_fact`].
    ///
    /// Используется там, где требуется перенести владение runtime-фактом
    /// дальше по пайплайну, например при merge вложенных фактов.
    #[inline]
    pub fn into_runtime_fact(self) -> Option<InterpretatorFact<'a>> {
        self.runtime_fact.map(|fact| *fact)
    }

    #[inline]
    fn projected_value(&self) -> FactValue {
        if let Some(as_json) = &self.as_json {
            as_json.clone()
        } else if let Some(fact) = self.runtime_fact() {
            runtime_fact_as_json_value(fact)
        } else {
            FactValue::Str(self.normalized.clone())
        }
    }
}

impl NormalizedItem for FactResult<'_> {
    fn normalized_value(&self) -> FactValue {
        self.projected_value()
    }
}

impl RuntimeItem for FactResult<'_> {
    fn as_json_value(&self) -> FactValue {
        self.projected_value()
    }

    fn spans(&self) -> Vec<Span> {
        let mut out = Vec::new();
        self.append_spans(&mut out);
        out
    }
}

impl RuntimeSpans for FactResult<'_> {
    fn append_spans(&self, out: &mut Vec<Span>) {
        if !self.spans.is_empty() {
            out.extend_from_slice(&self.spans);
        } else if let Some(fact) = self.runtime_fact() {
            fact.append_spans(out);
        }
    }
}

/// Результат интерпретации отдельного атрибута.
#[derive(Debug, Clone)]
pub struct AttributeResult<'a> {
    /// Значение атрибута (может быть вложенным результатом интерпретации).
    pub value: Box<InterpretationValue<'a>>,
    /// Метаданные атрибута, если они были сохранены.
    pub attribute: Option<YargyAttribute>,
}

impl<'a> AttributeResult<'a> {
    /// Создаёт результат атрибута из значения интерпретации.
    #[inline]
    pub fn new(value: InterpretationValue<'a>) -> Self {
        Self {
            value: Box::new(value),
            attribute: None,
        }
    }

    /// Дополняет результат информацией об атрибуте.
    #[inline]
    pub fn with_attribute(mut self, attribute: YargyAttribute) -> Self {
        self.attribute = Some(attribute);
        self
    }
}

impl NormalizedItem for AttributeResult<'_> {
    fn normalized_value(&self) -> FactValue {
        self.value.normalized_value()
    }
}

impl RuntimeItem for AttributeResult<'_> {
    fn as_json_value(&self) -> FactValue {
        self.value.as_json_value()
    }

    fn spans(&self) -> Vec<Span> {
        let mut out = Vec::new();
        self.append_spans(&mut out);
        out
    }
}

impl RuntimeSpans for AttributeResult<'_> {
    fn append_spans(&self, out: &mut Vec<Span>) {
        self.value.append_spans(out);
    }
}

/// Полезная нагрузка результата нормализации.
///
/// Может быть либо скалярным [`FactValue`], либо вложенным значением
/// интерпретации, которое само умеет отдавать `normalized/as_json/spans`.
#[derive(Debug, Clone)]
pub enum NormalizerResultValue<'a> {
    /// Готовое скалярное значение.
    Value(FactValue),
    /// Вложенный результат интерпретации.
    Nested(Box<InterpretationValue<'a>>),
}

/// Результат работы нормализатора в рантайме.
#[derive(Debug, Clone)]
pub struct NormalizerResult<'a> {
    /// Итог нормализации.
    pub value: NormalizerResultValue<'a>,
    /// Исходное входное значение (если нужно сохранить его `spans`).
    pub input: Option<Box<InterpretationValue<'a>>>,
}

impl<'a> NormalizerResult<'a> {
    /// Создаёт результат из скалярного значения.
    #[inline]
    pub fn from_value<V>(value: V) -> Self
    where
        V: Into<FactValue>,
    {
        Self {
            value: NormalizerResultValue::Value(value.into()),
            input: None,
        }
    }

    /// Создаёт строковый результат.
    #[inline]
    pub fn from_text<S>(value: S) -> Self
    where
        S: Into<String>,
    {
        Self::from_value(FactValue::Str(value.into()))
    }

    /// Создаёт результат из вложенного значения интерпретации.
    #[inline]
    pub fn from_nested(value: InterpretationValue<'a>) -> Self {
        Self {
            value: NormalizerResultValue::Nested(Box::new(value)),
            input: None,
        }
    }

    /// Сохраняет исходный вход для корректного переноса `spans`.
    #[inline]
    pub fn with_input(mut self, input: InterpretationValue<'a>) -> Self {
        self.input = Some(Box::new(input));
        self
    }
}

impl NormalizedItem for NormalizerResult<'_> {
    fn normalized_value(&self) -> FactValue {
        match &self.value {
            NormalizerResultValue::Value(value) => value.clone(),
            NormalizerResultValue::Nested(value) => value.normalized_value(),
        }
    }
}

impl RuntimeItem for NormalizerResult<'_> {
    fn as_json_value(&self) -> FactValue {
        match &self.value {
            NormalizerResultValue::Value(value) => value.clone(),
            NormalizerResultValue::Nested(value) => value.as_json_value(),
        }
    }

    fn spans(&self) -> Vec<Span> {
        let mut out = Vec::new();
        self.append_spans(&mut out);
        out
    }
}

impl RuntimeSpans for NormalizerResult<'_> {
    fn append_spans(&self, out: &mut Vec<Span>) {
        if let Some(input) = &self.input {
            input.append_spans(out);
            return;
        }

        if let NormalizerResultValue::Nested(value) = &self.value {
            value.append_spans(out);
        }
    }
}

/// Унифицированное значение интерпретации в рантайм-пайплайне.
#[derive(Debug, Clone)]
pub enum InterpretationValue<'a> {
    /// Непосредственная цепочка токенов.
    Chain(Chain<'a>),
    /// Результат построения факта.
    FactResult(FactResult<'a>),
    /// Результат построения атрибута.
    AttributeResult(AttributeResult<'a>),
    /// Результат работы нормализатора.
    NormalizerResult(NormalizerResult<'a>),
}

impl<'a> From<Chain<'a>> for InterpretationValue<'a> {
    fn from(value: Chain<'a>) -> Self {
        Self::Chain(value)
    }
}

impl<'a> From<FactResult<'a>> for InterpretationValue<'a> {
    fn from(value: FactResult<'a>) -> Self {
        Self::FactResult(value)
    }
}

impl<'a> From<AttributeResult<'a>> for InterpretationValue<'a> {
    fn from(value: AttributeResult<'a>) -> Self {
        Self::AttributeResult(value)
    }
}

impl<'a> From<NormalizerResult<'a>> for InterpretationValue<'a> {
    fn from(value: NormalizerResult<'a>) -> Self {
        Self::NormalizerResult(value)
    }
}

impl NormalizedItem for InterpretationValue<'_> {
    fn normalized_value(&self) -> FactValue {
        match self {
            InterpretationValue::Chain(value) => value.normalized_value(),
            InterpretationValue::FactResult(value) => value.normalized_value(),
            InterpretationValue::AttributeResult(value) => value.normalized_value(),
            InterpretationValue::NormalizerResult(value) => value.normalized_value(),
        }
    }
}

impl RuntimeItem for InterpretationValue<'_> {
    fn as_json_value(&self) -> FactValue {
        match self {
            InterpretationValue::Chain(value) => value.as_json_value(),
            InterpretationValue::FactResult(value) => value.as_json_value(),
            InterpretationValue::AttributeResult(value) => value.as_json_value(),
            InterpretationValue::NormalizerResult(value) => value.as_json_value(),
        }
    }

    fn spans(&self) -> Vec<Span> {
        let mut out = Vec::new();
        self.append_spans(&mut out);
        out
    }
}

impl RuntimeSpans for InterpretationValue<'_> {
    fn append_spans(&self, out: &mut Vec<Span>) {
        match self {
            InterpretationValue::Chain(value) => value.append_spans(out),
            InterpretationValue::FactResult(value) => value.append_spans(out),
            InterpretationValue::AttributeResult(value) => value.append_spans(out),
            InterpretationValue::NormalizerResult(value) => value.append_spans(out),
        }
    }
}

fn short_type_name<T>() -> &'static str {
    let full = type_name::<T>();
    full.rsplit("::").next().unwrap_or(full)
}

fn callable_name<T>() -> String {
    let name = short_type_name::<T>();
    if name.contains("closure") {
        "<lambda>".to_string()
    } else {
        name.to_string()
    }
}

/// Общий интерфейс морфологического шага нормализации.
///
/// Используется в композициях вида `{morph}.{function}`.
pub trait MorphCall {
    /// Выполняет морфологическое преобразование над цепочкой токенов.
    fn call(&self, item: &Chain<'_>) -> String;
    /// Возвращает человекочитаемую метку вызова.
    fn label(&self) -> String;
}

/// Морфологический нормализатор `normalized()`.
///
/// Возвращает `key`, если он задан в [`Chain`], иначе склеивает
/// нормализованные формы токенов.
#[derive(Debug, Clone, Default)]
pub struct NormalizedNormalizer;

impl NormalizedNormalizer {
    /// Добавляет пользовательскую функцию к результату `normalized()`.
    #[inline]
    pub fn custom<F, O>(self, function: F) -> MorphFunctionNormalizer<Self, F>
    where
        F: Fn(String) -> O,
    {
        MorphFunctionNormalizer::new(self, function)
    }

    /// Выполняет нормализацию цепочки токенов.
    #[inline]
    pub fn call(&self, item: &Chain<'_>) -> String {
        if let Some(key) = &item.key {
            key.clone()
        } else {
            join_normalized_tokens(item.tokens.iter())
        }
    }

    /// Строковая метка нормализатора в стиле yargy.
    #[inline]
    pub const fn label(&self) -> &'static str {
        "normalized()"
    }
}

impl Normalizer for NormalizedNormalizer {}
impl MorphNormalizer for NormalizedNormalizer {}

impl MorphCall for NormalizedNormalizer {
    fn call(&self, item: &Chain<'_>) -> String {
        NormalizedNormalizer::call(self, item)
    }

    fn label(&self) -> String {
        NormalizedNormalizer::label(self).to_string()
    }
}

/// Морфологический нормализатор `inflected(grams...)`.
#[derive(Debug, Clone, Default)]
pub struct InflectedNormalizer {
    /// Граммемы, используемые при приведении формы, либо `None` для дефолта.
    pub grams: Option<Vec<String>>,
}

impl InflectedNormalizer {
    /// Создаёт нормализатор с указанным набором граммем.
    #[inline]
    pub fn new(grams: Option<Vec<String>>) -> Self {
        Self { grams }
    }

    /// Добавляет пользовательскую функцию к результату `inflected(...)`.
    #[inline]
    pub fn custom<F, O>(self, function: F) -> MorphFunctionNormalizer<Self, F>
    where
        F: Fn(String) -> O,
    {
        MorphFunctionNormalizer::new(self, function)
    }

    /// Выполняет инфлексию токенов по заданным граммемам.
    #[inline]
    pub fn call(&self, item: &Chain<'_>) -> String {
        join_inflected_tokens_ref(item.tokens.iter(), self.grams.as_deref())
    }

    /// Строковая метка нормализатора в стиле yargy.
    #[inline]
    pub fn label(&self) -> String {
        match &self.grams {
            Some(grams) => format!("inflected({})", grams.join(", ")),
            None => "inflected()".to_string(),
        }
    }
}

impl Normalizer for InflectedNormalizer {}
impl MorphNormalizer for InflectedNormalizer {}

impl MorphCall for InflectedNormalizer {
    fn call(&self, item: &Chain<'_>) -> String {
        InflectedNormalizer::call(self, item)
    }

    fn label(&self) -> String {
        InflectedNormalizer::label(self)
    }
}

/// Нормализатор `const(value)`, всегда возвращающий одну и ту же константу.
#[derive(Debug, Clone)]
pub struct ConstNormalizer<T> {
    /// Константное значение, не зависящее от входа.
    pub value: T,
}

impl<T> ConstNormalizer<T> {
    /// Создаёт константный нормализатор.
    #[inline]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Игнорирует вход и возвращает сохранённое значение.
    #[inline]
    pub fn call<I>(&self, _item: I) -> T
    where
        T: Clone,
    {
        self.value.clone()
    }
}

impl<T> ConstNormalizer<T>
where
    T: fmt::Debug,
{
    /// Строковая метка нормализатора в стиле `const(...)`.
    #[inline]
    pub fn label(&self) -> String {
        format!("const({:?})", self.value)
    }
}

impl<T> Normalizer for ConstNormalizer<T> {}
impl<T> CustomNormalizer for ConstNormalizer<T> {}

/// Пользовательский нормализатор `custom(function)`.
pub struct FunctionNormalizer<F> {
    /// Функция преобразования, принимающая `FactValue`.
    pub function: F,
}

impl<F> FunctionNormalizer<F> {
    /// Создаёт пользовательский нормализатор.
    #[inline]
    pub fn new(function: F) -> Self {
        Self { function }
    }

    /// Строит композицию `custom(first).custom(second)`.
    #[inline]
    pub fn custom<G>(self, function: G) -> FunctionFunctionNormalizer<F, G> {
        FunctionFunctionNormalizer::new(self.function, function)
    }

    /// Строковая метка нормализатора в стиле yargy.
    #[inline]
    pub fn label(&self) -> String {
        format!("custom({})", callable_name::<F>())
    }
}

impl<F, O> FunctionNormalizer<F>
where
    F: Fn(FactValue) -> O,
{
    /// Применяет функцию к нормализованному значению входа.
    #[inline]
    pub fn call<I>(&self, item: &I) -> O
    where
        I: NormalizedItem,
    {
        (self.function)(item.normalized_value())
    }
}

impl<F> Normalizer for FunctionNormalizer<F> {}
impl<F> CustomNormalizer for FunctionNormalizer<F> {}

/// Композиция пользовательских нормализаторов `custom(first).custom(second)`.
pub struct FunctionFunctionNormalizer<F, G> {
    /// Первая функция в цепочке.
    pub first: F,
    /// Вторая функция, получающая результат первой.
    pub second: G,
}

impl<F, G> FunctionFunctionNormalizer<F, G> {
    /// Создаёт композицию из двух функций.
    #[inline]
    pub fn new(first: F, second: G) -> Self {
        Self { first, second }
    }

    /// Строковая метка нормализатора в стиле yargy.
    #[inline]
    pub fn label(&self) -> String {
        format!(
            "custom({}).custom({})",
            callable_name::<F>(),
            callable_name::<G>()
        )
    }
}

impl<F, G, O1, O2> FunctionFunctionNormalizer<F, G>
where
    F: Fn(FactValue) -> O1,
    G: Fn(O1) -> O2,
{
    /// Применяет обе функции последовательно.
    #[inline]
    pub fn call<I>(&self, item: &I) -> O2
    where
        I: NormalizedItem,
    {
        let value = item.normalized_value();
        (self.second)((self.first)(value))
    }
}

impl<F, G> Normalizer for FunctionFunctionNormalizer<F, G> {}
impl<F, G> CustomNormalizer for FunctionFunctionNormalizer<F, G> {}

/// Композиция морфологического и пользовательского нормализаторов `{morph}.{function}`.
pub struct MorphFunctionNormalizer<M, F> {
    /// Морфологическая часть нормализации.
    pub morph: M,
    /// Пользовательская функция, применяемая после морфологического шага.
    pub function: F,
}

impl<M, F> MorphFunctionNormalizer<M, F> {
    /// Создаёт композицию из морфологического шага и функции.
    #[inline]
    pub fn new(morph: M, function: F) -> Self {
        Self { morph, function }
    }

    /// Строковая метка нормализатора в стиле yargy.
    #[inline]
    pub fn label(&self) -> String
    where
        M: MorphCall,
    {
        format!("{}.{}", self.morph.label(), callable_name::<F>())
    }
}

impl<M, F, O> MorphFunctionNormalizer<M, F>
where
    M: MorphCall,
    F: Fn(String) -> O,
{
    /// Выполняет морфологический шаг и применяет пользовательскую функцию.
    #[inline]
    pub fn call(&self, item: &Chain<'_>) -> O {
        let value = self.morph.call(item);
        (self.function)(value)
    }
}

impl<M, F> Normalizer for MorphFunctionNormalizer<M, F> {}
impl<M, F> MorphNormalizer for MorphFunctionNormalizer<M, F> {}
impl<M, F> CustomNormalizer for MorphFunctionNormalizer<M, F> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::{fact, FactAttributeInput};
    use crate::span::Span;
    use crate::token::Tokenizer;

    fn runtime_fact_with_name<'a>(name: &'a str) -> InterpretatorFact<'a> {
        let scheme = fact("City", vec![FactAttributeInput::from("name")]);
        let mut runtime = InterpretatorFact::new(scheme);
        let tokens = Tokenizer::new().tokenize(name);
        let chain = Chain::new(tokens, None);

        runtime.set("name", chain.into()).unwrap();
        runtime
    }

    #[test]
    fn normalized_normalizer_uses_key_when_set() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Москва");
        let chain = Chain::new(tokens, Some("CITY".to_string()));

        let normalizer = NormalizedNormalizer;
        assert_eq!(normalizer.call(&chain), "CITY");
        assert_eq!(normalizer.label(), "normalized()");
    }

    #[test]
    fn normalized_normalizer_uses_tokens_when_key_absent() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("LoNDon");
        let chain = Chain::new(tokens, None);

        let normalizer = NormalizedNormalizer;
        assert_eq!(normalizer.call(&chain), "london");
    }

    #[test]
    fn const_normalizer_returns_value() {
        let normalizer = ConstNormalizer::new(42_i64);
        assert_eq!(normalizer.call(()), 42);
        assert_eq!(normalizer.label(), "const(42)");
    }

    #[test]
    fn function_normalizer_and_custom_chain() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("moscow");
        let chain = Chain::new(tokens, None);

        let normalizer =
            FunctionNormalizer::new(|value: FactValue| value.as_str().unwrap().to_uppercase());
        assert_eq!(normalizer.call(&chain), "MOSCOW");

        let chained = normalizer.custom(|value: String| format!("[{value}]"));
        assert_eq!(chained.call(&chain), "[MOSCOW]");
    }

    #[test]
    fn morph_function_normalizer_from_normalized() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Moscow");
        let chain = Chain::new(tokens, None);

        let normalizer = NormalizedNormalizer.custom(|value: String| value.len());
        assert_eq!(normalizer.call(&chain), 6);
    }

    #[test]
    fn inflected_normalizer_label() {
        let normalizer =
            InflectedNormalizer::new(Some(vec!["nomn".to_string(), "sing".to_string()]));
        assert_eq!(normalizer.label(), "inflected(nomn, sing)");
    }

    #[test]
    fn function_normalizer_accepts_attribute_result_like_yargy() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("1");
        let chain = Chain::new(tokens, None);
        let attr_result = AttributeResult::new(InterpretationValue::Chain(chain));

        let normalizer = FunctionNormalizer::new(|value: FactValue| {
            value.as_str().unwrap().parse::<i64>().unwrap()
        });
        assert_eq!(normalizer.call(&attr_result), 1);
    }

    #[test]
    fn function_normalizer_accepts_nested_runtime_result() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("2");
        let chain = Chain::new(tokens, None);

        let value = InterpretationValue::NormalizerResult(NormalizerResult::from_nested(
            InterpretationValue::AttributeResult(AttributeResult::new(InterpretationValue::Chain(
                chain,
            ))),
        ));

        let normalizer = FunctionNormalizer::new(|value: FactValue| {
            value.as_str().unwrap().parse::<i64>().unwrap()
        });
        assert_eq!(normalizer.call(&value), 2);
    }

    #[test]
    fn chain_runtime_projection_and_spans() {
        let tokenizer = Tokenizer::new();
        let chain = Chain::new(tokenizer.tokenize("A B"), None);

        assert_eq!(chain.normalized(), "A B");
        assert_eq!(chain.normalized_value(), FactValue::Str("A B".to_string()));
        assert_eq!(chain.as_json_value(), FactValue::Str("A B".to_string()));
        assert_eq!(chain.spans(), vec![Span::new(0, 3)]);

        let empty_chain = Chain::new(Vec::new(), None);
        assert!(empty_chain.spans().is_empty());
    }

    #[test]
    fn fact_result_builders_cover_value_and_json_paths() {
        let plain = FactResult::new("name").with_spans(vec![Span::new(2, 6)]);
        assert_eq!(plain.normalized_value(), FactValue::Str("name".to_string()));
        assert_eq!(plain.as_json_value(), FactValue::Str("name".to_string()));
        assert_eq!(plain.spans(), vec![Span::new(2, 6)]);

        let from_json = FactResult::from_json(FactValue::Int(7));
        assert_eq!(from_json.normalized, "7");
        assert_eq!(from_json.normalized_value(), FactValue::Int(7));
        assert_eq!(from_json.as_json_value(), FactValue::Int(7));

        let with_json = FactResult::new("old").with_json(FactValue::Bool(true));
        assert_eq!(with_json.normalized, "true");
        assert_eq!(with_json.normalized_value(), FactValue::Bool(true));
        assert_eq!(with_json.as_json_value(), FactValue::Bool(true));
    }

    #[test]
    fn fact_result_from_runtime_fact_uses_runtime_fallbacks() {
        let runtime = runtime_fact_with_name("Moscow");
        let result = FactResult::from_runtime_fact(runtime);

        let expected = FactValue::Object(vec![(
            "name".to_string(),
            FactValue::Str("Moscow".to_string()),
        )]);

        assert_eq!(result.as_json_value(), expected.clone());
        assert_eq!(result.normalized_value(), expected);
        assert_eq!(result.spans(), vec![Span::new(0, 6)]);
    }

    #[test]
    fn fact_result_runtime_without_cached_json_materializes_json_lazily() {
        let runtime = runtime_fact_with_name("Paris");
        let result = FactResult {
            normalized: String::new(),
            as_json: None,
            spans: Vec::new(),
            runtime_fact: Some(Box::new(runtime)),
        };

        assert_eq!(
            result.normalized_value(),
            FactValue::Object(vec![(
                "name".to_string(),
                FactValue::Str("Paris".to_string())
            )])
        );

        assert_eq!(
            result.as_json_value(),
            FactValue::Object(vec![(
                "name".to_string(),
                FactValue::Str("Paris".to_string())
            )])
        );
        assert_eq!(result.spans(), vec![Span::new(0, 5)]);
    }

    #[test]
    fn fact_result_runtime_reuses_runtime_fact_cached_projections() {
        let runtime = runtime_fact_with_name("Berlin");
        let result = FactResult::from_runtime_fact(runtime);

        let expected = FactValue::Object(vec![(
            "name".to_string(),
            FactValue::Str("Berlin".to_string()),
        )]);

        assert_eq!(result.as_json_value(), expected.clone());
        assert_eq!(result.as_json_value(), expected.clone());
        assert_eq!(result.normalized_value(), expected);
        assert_eq!(result.spans(), vec![Span::new(0, 6)]);
        assert_eq!(result.spans(), vec![Span::new(0, 6)]);
    }

    #[test]
    fn attribute_result_delegates_and_keeps_attribute() {
        let tokenizer = Tokenizer::new();
        let chain = Chain::new(tokenizer.tokenize("London"), None);
        let attribute = YargyAttribute::new("City", "name", None);

        let result = AttributeResult::new(chain.into()).with_attribute(attribute.clone());
        assert_eq!(result.attribute, Some(attribute));
        assert_eq!(
            result.normalized_value(),
            FactValue::Str("London".to_string())
        );
        assert_eq!(result.as_json_value(), FactValue::Str("London".to_string()));
        assert_eq!(result.spans(), vec![Span::new(0, 6)]);
    }

    #[test]
    fn normalizer_result_supports_value_nested_and_input_spans() {
        let from_text = NormalizerResult::from_text("RAW");
        assert_eq!(
            from_text.normalized_value(),
            FactValue::Str("RAW".to_string())
        );
        assert_eq!(from_text.as_json_value(), FactValue::Str("RAW".to_string()));
        assert!(from_text.spans().is_empty());

        let tokenizer = Tokenizer::new();
        let nested_chain = Chain::new(tokenizer.tokenize("A B"), None);
        let nested = NormalizerResult::from_nested(nested_chain.into());
        assert_eq!(nested.normalized_value(), FactValue::Str("A B".to_string()));
        assert_eq!(nested.as_json_value(), FactValue::Str("A B".to_string()));
        assert_eq!(nested.spans(), vec![Span::new(0, 3)]);

        let value_chain = Chain::new(tokenizer.tokenize("X"), None);
        let input_chain = Chain::new(tokenizer.tokenize("input value"), None);
        let with_input = NormalizerResult::from_nested(value_chain.into())
            .with_input(InterpretationValue::Chain(input_chain));
        assert_eq!(with_input.spans(), vec![Span::new(0, 11)]);
    }

    #[test]
    fn append_spans_matches_public_spans_for_nested_runtime_values() {
        let tokenizer = Tokenizer::new();
        let chain = Chain::new(tokenizer.tokenize("A B"), None);
        let nested = InterpretationValue::NormalizerResult(NormalizerResult::from_nested(
            InterpretationValue::AttributeResult(AttributeResult::new(InterpretationValue::Chain(
                chain,
            ))),
        ));

        let mut appended = Vec::new();
        nested.append_spans(&mut appended);

        assert_eq!(appended, nested.spans());
        assert_eq!(appended, vec![Span::new(0, 3)]);
    }

    #[test]
    fn interpretation_value_from_impls_preserve_runtime_behavior() {
        let tokenizer = Tokenizer::new();
        let chain = Chain::new(tokenizer.tokenize("abc"), None);
        let chain_value: InterpretationValue<'_> = chain.into();
        assert_eq!(
            chain_value.normalized_value(),
            FactValue::Str("abc".to_string())
        );

        let fact_value: InterpretationValue<'_> = FactResult::from_json(FactValue::Int(3)).into();
        assert_eq!(fact_value.normalized_value(), FactValue::Int(3));
        assert_eq!(fact_value.as_json_value(), FactValue::Int(3));

        let attr_value: InterpretationValue<'_> = AttributeResult::new(chain_value).into();
        assert_eq!(
            attr_value.normalized_value(),
            FactValue::Str("abc".to_string())
        );

        let norm_value: InterpretationValue<'_> = NormalizerResult::from_value(5).into();
        assert_eq!(norm_value.normalized_value(), FactValue::Int(5));
    }

    #[test]
    fn inflected_normalizer_without_grams_has_stable_label_and_call() {
        let tokenizer = Tokenizer::new();
        let chain = Chain::new(tokenizer.tokenize("LoNDon"), None);
        let normalizer = InflectedNormalizer::new(None);

        assert_eq!(normalizer.label(), "inflected()");
        assert_eq!(normalizer.call(&chain), "london");
    }

    #[test]
    fn labels_for_function_and_morph_normalizers_use_lambda_placeholder() {
        let function = FunctionNormalizer::new(|value: FactValue| value.to_string());
        assert_eq!(function.label(), "custom(<lambda>)");

        let chained = function.custom(|value: String| value.len());
        assert_eq!(chained.label(), "custom(<lambda>).custom(<lambda>)");

        let morph = NormalizedNormalizer.custom(|value: String| value.len());
        assert_eq!(morph.label(), "normalized().<lambda>");

        let inflected_morph = InflectedNormalizer::new(None).custom(|value: String| value.len());
        assert_eq!(inflected_morph.label(), "inflected().<lambda>");
    }
}
