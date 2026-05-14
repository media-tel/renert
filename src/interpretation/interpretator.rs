//! Интерпретаторы и адаптеры преобразований.
//!
//! Модуль связывает:
//! - декларативные атрибуты/факты (`Attribute`, `FactScheme`);
//! - нормализаторы (`NormalizedNormalizer`, `FunctionNormalizer` и др.);
//! - рантайм-значения интерпретации (`InterpretationValue`).
//!
//! Основная задача — подготовить унифицированные вызываемые объекты
//! (`AnyInterpretator`), которые принимают токены или промежуточные результаты
//! и возвращают итоговые значения для сборки фактов.
//!
//! В модуле одновременно поддерживаются:
//! - «legacy» payload [`Interpretation`] (для parser extraction);
//! - рантайм-пайплайн через `InterpretatorInput` -> `InterpretatorResult`.

use std::fmt;
use std::sync::Arc;

use super::attribute::{
    Attribute, ConstAttribute, FunctionAttribute, FunctionFunctionAttribute, InflectedAttribute,
    InflectedFunctionAttribute, NormalizedAttribute, NormalizedFunctionAttribute,
    RepeatableAttribute,
};
use super::fact::{FactAttribute, FactError, FactScheme, InterpretatorFact};
use super::normalizer::{
    ConstNormalizer, FunctionFunctionNormalizer, FunctionNormalizer, InflectedNormalizer,
    InterpretationValue, MorphCall, MorphFunctionNormalizer, NormalizedItem, NormalizedNormalizer,
};
use super::FactValue;
use crate::internal::{CoreInterpretation, CoreTransform};
use crate::token::Token;

/// Элемент «legacy» пайплайна трансформаций для parser extraction.
pub type Transform = CoreTransform<FactValue>;

/// Конечный payload интерпретации для parser extraction.
pub type Interpretation = CoreInterpretation<FactValue>;

impl CoreInterpretation<FactValue> {
    /// Создаёт `Interpretation` из дескриптора [`FactAttribute`].
    #[inline]
    pub fn from_attr(attr: &FactAttribute) -> Self {
        Self {
            fact_name: attr.fact_name().to_string(),
            field_name: attr.field_name().to_string(),
            const_value: attr
                .get_const()
                .map(|value| FactValue::Str(value.to_string())),
            transforms: Vec::new(),
            repeatable: false,
        }
    }

    /// Проверяет, задано ли константное значение.
    #[inline]
    pub fn has_const(&self) -> bool {
        self.const_value.is_some()
    }
}

impl From<FactAttribute> for Interpretation {
    fn from(attr: FactAttribute) -> Self {
        Self::from_attr(&attr)
    }
}

impl From<&FactAttribute> for Interpretation {
    fn from(attr: &FactAttribute) -> Self {
        Self::from_attr(attr)
    }
}

impl From<Attribute> for Interpretation {
    fn from(attribute: Attribute) -> Self {
        let super::attribute::Attribute {
            base: super::attribute::AttributeBase { fact, name },
            ..
        } = attribute;
        Self {
            fact_name: fact.into_name(),
            field_name: name,
            const_value: None,
            transforms: Vec::new(),
            repeatable: false,
        }
    }
}

impl From<RepeatableAttribute> for Interpretation {
    fn from(attribute: RepeatableAttribute) -> Self {
        let RepeatableAttribute {
            base: super::attribute::AttributeBase { fact, name },
        } = attribute;
        Self {
            fact_name: fact.into_name(),
            field_name: name,
            const_value: None,
            transforms: Vec::new(),
            repeatable: true,
        }
    }
}

impl From<NormalizedAttribute> for Interpretation {
    fn from(attribute: NormalizedAttribute) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        value.transforms.push(Transform::Normalized);
        value
    }
}

impl From<InflectedAttribute> for Interpretation {
    fn from(attribute: InflectedAttribute) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        value
            .transforms
            .push(Transform::Inflected(attribute.grams.into_iter().collect()));
        value
    }
}

impl<T> From<ConstAttribute<T>> for Interpretation
where
    T: Clone + Into<FactValue>,
{
    fn from(attribute: ConstAttribute<T>) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        value.const_value = Some(attribute.value.into());
        value
    }
}

impl<F, O> From<FunctionAttribute<F>> for Interpretation
where
    F: Fn(FactValue) -> O + Send + Sync + 'static,
    O: Into<FactValue> + 'static,
{
    fn from(attribute: FunctionAttribute<F>) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        let function = attribute.function;
        value
            .transforms
            .push(Transform::Custom(Arc::new(move |item: &FactValue| {
                Some(function(item.clone()).into())
            })));
        value
    }
}

impl<F, G, O1, O2> From<FunctionFunctionAttribute<F, G>> for Interpretation
where
    F: Fn(FactValue) -> O1 + Send + Sync + 'static,
    G: Fn(O1) -> O2 + Send + Sync + 'static,
    O2: Into<FactValue> + 'static,
{
    fn from(attribute: FunctionFunctionAttribute<F, G>) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        let first = attribute.first;
        let second = attribute.second;
        value
            .transforms
            .push(Transform::Custom(Arc::new(move |item: &FactValue| {
                Some(second(first(item.clone())).into())
            })));
        value
    }
}

impl<F, O> From<InflectedFunctionAttribute<F>> for Interpretation
where
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue> + 'static,
{
    fn from(attribute: InflectedFunctionAttribute<F>) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        value
            .transforms
            .push(Transform::Inflected(attribute.grams.into_iter().collect()));
        let function = attribute.function;
        value
            .transforms
            .push(Transform::Custom(Arc::new(move |item: &FactValue| {
                Some(function(item.as_str().unwrap_or_default().to_string()).into())
            })));
        value
    }
}

impl<F, O> From<NormalizedFunctionAttribute<F>> for Interpretation
where
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue> + 'static,
{
    fn from(attribute: NormalizedFunctionAttribute<F>) -> Self {
        let mut value = Interpretation::from(attribute.attribute);
        value.transforms.push(Transform::Normalized);
        let function = attribute.function;
        value
            .transforms
            .push(Transform::Custom(Arc::new(move |item: &FactValue| {
                Some(function(item.as_str().unwrap_or_default().to_string()).into())
            })));
        value
    }
}

/// Ошибки, возникающие при выполнении интерпретаторов.
#[derive(Debug, Clone)]
pub enum InterpretatorError {
    /// Некорректная форма входных данных.
    InvalidInput(String),
    /// Ошибка типа/ожидаемого формата значения.
    TypeError(String),
    /// Ошибка операций с фактом.
    FactError(FactError),
}

impl fmt::Display for InterpretatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpretatorError::InvalidInput(msg) => {
                write!(f, "invalid interpretator input: {msg}")
            }
            InterpretatorError::TypeError(msg) => write!(f, "interpretator type error: {msg}"),
            InterpretatorError::FactError(err) => write!(f, "fact error: {err}"),
        }
    }
}

impl std::error::Error for InterpretatorError {}

impl From<FactError> for InterpretatorError {
    fn from(value: FactError) -> Self {
        InterpretatorError::FactError(value)
    }
}

/// Псевдоним цепочки токенов для удобства API интерпретаторов.
pub type Chain<'a> = super::normalizer::Chain<'a>;
/// Псевдоним результата факта.
pub type FactResult<'a> = super::normalizer::FactResult<'a>;
/// Псевдоним результата атрибута.
pub type AttributeResult<'a> = super::normalizer::AttributeResult<'a>;
/// Псевдоним результата нормализатора.
pub type NormalizerResult<'a> = super::normalizer::NormalizerResult<'a>;
/// Псевдоним полезной нагрузки результата нормализатора.
pub type NormalizerResultValue<'a> = super::normalizer::NormalizerResultValue<'a>;
/// Унифицированный результат интерпретатора.
pub type InterpretatorResult<'a> = super::normalizer::InterpretationValue<'a>;

/// Входные данные для вызова интерпретатора.
#[derive(Debug, Clone)]
pub struct InterpretatorInput<'a> {
    /// Элементы входа: токены и/или результаты предыдущих шагов.
    pub items: Vec<InterpretatorInputItem<'a>>,
    /// Необязательный ключ (переопределение нормализованного значения цепочки).
    pub key: Option<String>,
}

impl<'a> InterpretatorInput<'a> {
    /// Создаёт вход из готового списка элементов.
    #[inline]
    pub fn new(items: Vec<InterpretatorInputItem<'a>>, key: Option<String>) -> Self {
        Self { items, key }
    }

    /// Создаёт вход, содержащий только токены.
    #[inline]
    pub fn from_tokens(tokens: Vec<Token<'a>>, key: Option<String>) -> Self {
        Self {
            items: tokens
                .into_iter()
                .map(InterpretatorInputItem::Token)
                .collect(),
            key,
        }
    }

    fn single_result_or_chain(self) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        let len = self.items.len();

        if len == 1 {
            let item = self.items.into_iter().next().expect("single item expected");
            return match item {
                InterpretatorInputItem::Result(result) => Ok(result),
                InterpretatorInputItem::Token(token) => Ok(InterpretationValue::Chain(Chain::new(
                    vec![token],
                    self.key,
                ))),
            };
        }

        let mut tokens = Vec::with_capacity(len);
        for item in self.items {
            match item {
                InterpretatorInputItem::Token(token) => tokens.push(token),
                InterpretatorInputItem::Result(_) => {
                    return Err(InterpretatorError::TypeError(format!(
                        "expected all tokens or a single interpretator item, got {} items",
                        len
                    )));
                }
            }
        }

        Ok(InterpretationValue::Chain(Chain::new(tokens, self.key)))
    }
}

/// Элемент входа интерпретатора.
#[derive(Debug, Clone)]
pub enum InterpretatorInputItem<'a> {
    /// Отдельный токен.
    Token(Token<'a>),
    /// Результат предыдущего шага интерпретации.
    Result(InterpretatorResult<'a>),
}

fn as_chain<'a>(value: &'a InterpretatorResult<'a>) -> Option<&'a Chain<'a>> {
    match value {
        InterpretationValue::Chain(chain) => Some(chain),
        _ => None,
    }
}

/// Функция динамической нормализации для рантайм-пайплайна.
pub type NormalizerFn =
    dyn for<'a> Fn(&InterpretatorResult<'a>) -> Result<FactValue, InterpretatorError> + Send + Sync;

/// Унифицированный вызываемый нормализатор для рантайм-пайплайна.
#[derive(Clone)]
pub enum NormalizerCallable {
    /// Константный нормализатор (вход игнорируется).
    Const {
        /// Константное значение результата.
        value: FactValue,
        /// Человекочитаемая метка вызова.
        label: String,
    },
    /// Динамический нормализатор.
    Dynamic {
        /// Человекочитаемая метка вызова.
        label: String,
        /// Функция преобразования входного результата.
        call: Arc<NormalizerFn>,
    },
}

impl fmt::Debug for NormalizerCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NormalizerCallable::Const { value, label } => f
                .debug_struct("NormalizerCallable::Const")
                .field("label", label)
                .field("value", value)
                .finish(),
            NormalizerCallable::Dynamic { label, .. } => f
                .debug_struct("NormalizerCallable::Dynamic")
                .field("label", label)
                .finish(),
        }
    }
}

impl NormalizerCallable {
    /// Возвращает метку нормализатора.
    #[inline]
    pub fn label(&self) -> &str {
        match self {
            NormalizerCallable::Const { label, .. } => label,
            NormalizerCallable::Dynamic { label, .. } => label,
        }
    }

    fn call<'a>(&self, input: &InterpretatorResult<'a>) -> Result<FactValue, InterpretatorError> {
        match self {
            NormalizerCallable::Const { value, .. } => Ok(value.clone()),
            NormalizerCallable::Dynamic { call, .. } => call(input),
        }
    }

    fn call_result<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<NormalizerResult<'a>, InterpretatorError> {
        match self {
            NormalizerCallable::Const { value, .. } => {
                Ok(NormalizerResult::from_value(value.clone()))
            }
            NormalizerCallable::Dynamic { .. } => {
                let prepared = input.single_result_or_chain()?;
                let value = self.call(&prepared)?;
                Ok(NormalizerResult::from_value(value).with_input(prepared))
            }
        }
    }
}

impl<T> From<ConstNormalizer<T>> for NormalizerCallable
where
    T: Clone + Into<FactValue> + fmt::Debug + Send + Sync + 'static,
{
    fn from(normalizer: ConstNormalizer<T>) -> Self {
        Self::Const {
            value: normalizer.value.clone().into(),
            label: normalizer.label(),
        }
    }
}

impl From<NormalizedNormalizer> for NormalizerCallable {
    fn from(normalizer: NormalizedNormalizer) -> Self {
        let label = normalizer.label().to_string();
        Self::Dynamic {
            label,
            call: Arc::new(move |input| {
                let chain = as_chain(input).ok_or_else(|| {
                    InterpretatorError::TypeError("normalized() expects token chain".to_string())
                })?;
                Ok(FactValue::Str(normalizer.call(chain)))
            }),
        }
    }
}

impl From<InflectedNormalizer> for NormalizerCallable {
    fn from(normalizer: InflectedNormalizer) -> Self {
        let label = normalizer.label();
        Self::Dynamic {
            label,
            call: Arc::new(move |input| {
                let chain = as_chain(input).ok_or_else(|| {
                    InterpretatorError::TypeError("inflected() expects token chain".to_string())
                })?;
                Ok(FactValue::Str(normalizer.call(chain)))
            }),
        }
    }
}

impl<F, O> From<FunctionNormalizer<F>> for NormalizerCallable
where
    F: Fn(FactValue) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(normalizer: FunctionNormalizer<F>) -> Self {
        let label = normalizer.label();
        let function = normalizer.function;
        Self::Dynamic {
            label,
            call: Arc::new(move |input| Ok(function(input.normalized_value()).into())),
        }
    }
}

impl<F, G, O1, O2> From<FunctionFunctionNormalizer<F, G>> for NormalizerCallable
where
    F: Fn(FactValue) -> O1 + Send + Sync + 'static,
    G: Fn(O1) -> O2 + Send + Sync + 'static,
    O2: Into<FactValue>,
{
    fn from(normalizer: FunctionFunctionNormalizer<F, G>) -> Self {
        let label = normalizer.label();
        let first = normalizer.first;
        let second = normalizer.second;
        Self::Dynamic {
            label,
            call: Arc::new(move |input| Ok(second(first(input.normalized_value())).into())),
        }
    }
}

impl<M, F, O> From<MorphFunctionNormalizer<M, F>> for NormalizerCallable
where
    M: MorphCall + Send + Sync + 'static,
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(normalizer: MorphFunctionNormalizer<M, F>) -> Self {
        let label = normalizer.label();
        let morph = normalizer.morph;
        let function = normalizer.function;
        Self::Dynamic {
            label,
            call: Arc::new(move |input| {
                let chain = as_chain(input).ok_or_else(|| {
                    InterpretatorError::TypeError(
                        "morph normalizer expects token chain".to_string(),
                    )
                })?;
                Ok(function(morph.call(chain)).into())
            }),
        }
    }
}

/// Интерпретатор, собирающий итоговый факт из входных результатов.
#[derive(Debug, Clone)]
pub struct FactInterpretator {
    /// Схема целевого факта.
    pub fact: FactScheme,
}

impl FactInterpretator {
    /// Создаёт интерпретатор факта.
    #[inline]
    pub fn new(fact: FactScheme) -> Self {
        Self { fact }
    }

    /// Выполняет сборку рантайм-факта по входным данным.
    ///
    /// Обрабатывает:
    /// - результаты атрибутов подходящей иерархии фактов;
    /// - вложенные `FactResult`, если их схема является подклассом целевой.
    pub fn call<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        let mut fact = InterpretatorFact::new(self.fact.clone());

        for item in input.items {
            if let InterpretatorInputItem::Result(item) = item {
                match item {
                    InterpretationValue::AttributeResult(attr) => {
                        if let Some(attribute) = attr.attribute.as_ref() {
                            if self.fact.lineage.contains(attribute.fact().name()) {
                                fact.set(attribute.name(), *attr.value)?;
                            }
                        }
                    }
                    InterpretationValue::FactResult(result) => {
                        if let Some(inner) = result.into_runtime_fact() {
                            if inner.scheme().is_subclass_of(&self.fact) {
                                fact.merge_owned(inner);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(InterpretationValue::FactResult(
            FactResult::from_runtime_fact(fact),
        ))
    }

    /// Возвращает метку интерпретатора (имя факта).
    #[inline]
    pub fn label(&self) -> String {
        self.fact.name.clone()
    }
}

/// Интерпретатор присваивания значения атрибуту.
#[derive(Debug, Clone)]
pub struct AttributeInterpretator {
    /// Атрибут, в который будет записан результат.
    pub attribute: Attribute,
}

impl AttributeInterpretator {
    /// Создаёт интерпретатор атрибута.
    #[inline]
    pub fn new(attribute: Attribute) -> Self {
        Self { attribute }
    }

    /// Преобразует вход в `AttributeResult`, сохраняя метаданные атрибута.
    pub fn call<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        let value = input.single_result_or_chain()?;
        let value = match value {
            InterpretationValue::AttributeResult(result) => *result.value,
            InterpretationValue::NormalizerResult(_)
            | InterpretationValue::FactResult(_)
            | InterpretationValue::Chain(_) => value,
        };

        Ok(InterpretationValue::AttributeResult(
            AttributeResult::new(value).with_attribute(self.attribute.clone()),
        ))
    }

    /// Возвращает метку интерпретатора (`Fact.field`).
    #[inline]
    pub fn label(&self) -> String {
        self.attribute.label()
    }
}

/// Интерпретатор, применяющий нормализатор к входу.
#[derive(Debug, Clone)]
pub struct NormalizerInterpretator {
    /// Нормализатор в унифицированной вызываемой форме.
    pub normalizer: NormalizerCallable,
}

impl NormalizerInterpretator {
    /// Создаёт интерпретатор нормализатора.
    #[inline]
    pub fn new(normalizer: NormalizerCallable) -> Self {
        Self { normalizer }
    }

    /// Применяет нормализатор и возвращает `NormalizerResult`.
    pub fn call<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        Ok(InterpretationValue::NormalizerResult(
            self.normalizer.call_result(input)?,
        ))
    }

    /// Возвращает метку интерпретатора.
    #[inline]
    pub fn label(&self) -> String {
        self.normalizer.label().to_string()
    }
}

/// Интерпретатор связки «атрибут + нормализатор».
#[derive(Debug, Clone)]
pub struct AttributeNormalizerInterpretator {
    /// Целевой атрибут.
    pub attribute: Attribute,
    /// Нормализатор значения атрибута.
    pub normalizer: NormalizerCallable,
}

impl AttributeNormalizerInterpretator {
    /// Создаёт интерпретатор атрибута с нормализатором.
    #[inline]
    pub fn new(attribute: Attribute, normalizer: NormalizerCallable) -> Self {
        Self {
            attribute,
            normalizer,
        }
    }

    /// Применяет нормализатор и оборачивает результат в `AttributeResult`.
    pub fn call<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        let value = InterpretationValue::NormalizerResult(self.normalizer.call_result(input)?);
        Ok(InterpretationValue::AttributeResult(
            AttributeResult::new(value).with_attribute(self.attribute.clone()),
        ))
    }

    /// Возвращает метку вида `Fact.field.normalizer`.
    #[inline]
    pub fn label(&self) -> String {
        let attribute = self.attribute.label();
        let normalizer = self.normalizer.label();
        let mut label = String::with_capacity(attribute.len() + 1 + normalizer.len());
        label.push_str(&attribute);
        label.push('.');
        label.push_str(normalizer);
        label
    }
}

/// Унифицированный enum всех поддерживаемых интерпретаторов.
#[derive(Clone, Debug)]
pub enum AnyInterpretator {
    /// Интерпретатор факта.
    Fact(FactInterpretator),
    /// Интерпретатор атрибута.
    Attribute(AttributeInterpretator),
    /// Интерпретатор нормализатора.
    Normalizer(NormalizerInterpretator),
    /// Интерпретатор связки «атрибут + нормализатор».
    AttributeNormalizer(AttributeNormalizerInterpretator),
}

impl AnyInterpretator {
    /// Выполняет вызов конкретного варианта интерпретатора.
    pub fn call<'a>(
        &self,
        input: InterpretatorInput<'a>,
    ) -> Result<InterpretatorResult<'a>, InterpretatorError> {
        match self {
            AnyInterpretator::Fact(interp) => interp.call(input),
            AnyInterpretator::Attribute(interp) => interp.call(input),
            AnyInterpretator::Normalizer(interp) => interp.call(input),
            AnyInterpretator::AttributeNormalizer(interp) => interp.call(input),
        }
    }

    /// Возвращает метку активного интерпретатора.
    #[inline]
    pub fn label(&self) -> String {
        match self {
            AnyInterpretator::Fact(interp) => interp.label(),
            AnyInterpretator::Attribute(interp) => interp.label(),
            AnyInterpretator::Normalizer(interp) => interp.label(),
            AnyInterpretator::AttributeNormalizer(interp) => interp.label(),
        }
    }
}

/// Подготовленный элемент для сборки интерпретатора атрибута.
#[derive(Clone, Debug)]
pub enum PreparedAttributeItem {
    /// Обычный атрибут без нормализации.
    Attribute(Attribute),
    /// Атрибут с морфологическим нормализатором.
    MorphAttribute {
        /// Целевой атрибут.
        attribute: Attribute,
        /// Нормализатор морфологического шага.
        normalizer: NormalizerCallable,
    },
    /// Атрибут с пользовательским нормализатором.
    CustomAttribute {
        /// Целевой атрибут.
        attribute: Attribute,
        /// Пользовательский нормализатор.
        normalizer: NormalizerCallable,
    },
}

impl PreparedAttributeItem {
    #[inline]
    fn morph(attribute: Attribute, normalizer: NormalizerCallable) -> Self {
        Self::MorphAttribute {
            attribute,
            normalizer,
        }
    }

    #[inline]
    fn custom(attribute: Attribute, normalizer: NormalizerCallable) -> Self {
        Self::CustomAttribute {
            attribute,
            normalizer,
        }
    }

    #[inline]
    fn into_attribute_normalizer(self) -> Option<(Attribute, NormalizerCallable)> {
        match self {
            Self::Attribute(_) => None,
            Self::MorphAttribute {
                attribute,
                normalizer,
            }
            | Self::CustomAttribute {
                attribute,
                normalizer,
            } => Some((attribute, normalizer)),
        }
    }
}

impl From<Attribute> for PreparedAttributeItem {
    fn from(value: Attribute) -> Self {
        PreparedAttributeItem::Attribute(value)
    }
}

impl From<RepeatableAttribute> for PreparedAttributeItem {
    fn from(value: RepeatableAttribute) -> Self {
        let RepeatableAttribute { base } = value;
        PreparedAttributeItem::Attribute(Attribute {
            base,
            default: None,
        })
    }
}

impl From<InflectedAttribute> for PreparedAttributeItem {
    fn from(value: InflectedAttribute) -> Self {
        let InflectedAttribute { attribute, grams } = value;
        PreparedAttributeItem::morph(
            attribute,
            InflectedNormalizer::new(Some(grams.into_iter().collect())).into(),
        )
    }
}

impl From<NormalizedAttribute> for PreparedAttributeItem {
    fn from(value: NormalizedAttribute) -> Self {
        let NormalizedAttribute { attribute } = value;
        PreparedAttributeItem::morph(attribute, NormalizedNormalizer.into())
    }
}

impl<T> From<ConstAttribute<T>> for PreparedAttributeItem
where
    T: Clone + Into<FactValue> + fmt::Debug + Send + Sync + 'static,
{
    fn from(value: ConstAttribute<T>) -> Self {
        let ConstAttribute {
            attribute,
            value: const_value,
        } = value;
        PreparedAttributeItem::custom(attribute, ConstNormalizer::new(const_value).into())
    }
}

impl<F, O> From<FunctionAttribute<F>> for PreparedAttributeItem
where
    F: Fn(FactValue) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: FunctionAttribute<F>) -> Self {
        let FunctionAttribute {
            attribute,
            function,
        } = value;
        PreparedAttributeItem::custom(attribute, FunctionNormalizer::new(function).into())
    }
}

impl<F, G, O1, O2> From<FunctionFunctionAttribute<F, G>> for PreparedAttributeItem
where
    F: Fn(FactValue) -> O1 + Send + Sync + 'static,
    G: Fn(O1) -> O2 + Send + Sync + 'static,
    O2: Into<FactValue>,
{
    fn from(value: FunctionFunctionAttribute<F, G>) -> Self {
        let FunctionFunctionAttribute {
            attribute,
            first,
            second,
        } = value;
        PreparedAttributeItem::custom(
            attribute,
            FunctionFunctionNormalizer::new(first, second).into(),
        )
    }
}

impl<F, O> From<InflectedFunctionAttribute<F>> for PreparedAttributeItem
where
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: InflectedFunctionAttribute<F>) -> Self {
        let InflectedFunctionAttribute {
            attribute,
            grams,
            function,
        } = value;
        PreparedAttributeItem::custom(
            attribute,
            MorphFunctionNormalizer::new(
                InflectedNormalizer::new(Some(grams.into_iter().collect())),
                function,
            )
            .into(),
        )
    }
}

impl<F, O> From<NormalizedFunctionAttribute<F>> for PreparedAttributeItem
where
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: NormalizedFunctionAttribute<F>) -> Self {
        let NormalizedFunctionAttribute {
            attribute,
            function,
        } = value;
        PreparedAttributeItem::custom(
            attribute,
            MorphFunctionNormalizer::new(NormalizedNormalizer, function).into(),
        )
    }
}

/// Подготовленный элемент для сборки token-level интерпретатора.
#[derive(Clone, Debug)]
pub enum PreparedTokenItem {
    /// Вариант атрибута (возможно, с нормализатором).
    Attribute(PreparedAttributeItem),
    /// Отдельный нормализатор.
    Normalizer(NormalizerCallable),
}

impl<T> From<T> for PreparedTokenItem
where
    T: Into<PreparedAttributeItem>,
{
    fn from(value: T) -> Self {
        PreparedTokenItem::Attribute(value.into())
    }
}

impl From<NormalizerCallable> for PreparedTokenItem {
    fn from(value: NormalizerCallable) -> Self {
        PreparedTokenItem::Normalizer(value)
    }
}

impl From<NormalizedNormalizer> for PreparedTokenItem {
    fn from(value: NormalizedNormalizer) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

impl From<InflectedNormalizer> for PreparedTokenItem {
    fn from(value: InflectedNormalizer) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

impl<T> From<ConstNormalizer<T>> for PreparedTokenItem
where
    T: Clone + Into<FactValue> + fmt::Debug + Send + Sync + 'static,
{
    fn from(value: ConstNormalizer<T>) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

impl<F, O> From<FunctionNormalizer<F>> for PreparedTokenItem
where
    F: Fn(FactValue) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: FunctionNormalizer<F>) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

impl<F, G, O1, O2> From<FunctionFunctionNormalizer<F, G>> for PreparedTokenItem
where
    F: Fn(FactValue) -> O1 + Send + Sync + 'static,
    G: Fn(O1) -> O2 + Send + Sync + 'static,
    O2: Into<FactValue>,
{
    fn from(value: FunctionFunctionNormalizer<F, G>) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

impl<M, F, O> From<MorphFunctionNormalizer<M, F>> for PreparedTokenItem
where
    M: MorphCall + Send + Sync + 'static,
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: MorphFunctionNormalizer<M, F>) -> Self {
        PreparedTokenItem::Normalizer(value.into())
    }
}

/// Подготовленный элемент для сборки rule-level интерпретатора.
#[derive(Clone, Debug)]
pub enum PreparedRuleItem {
    /// Уже готовый интерпретатор.
    Interpretator(AnyInterpretator),
    /// Схема факта для `FactInterpretator`.
    Fact(FactScheme),
    /// Атрибутный элемент.
    Attribute(PreparedAttributeItem),
    /// Нормализаторный элемент.
    Normalizer(NormalizerCallable),
}

impl From<AnyInterpretator> for PreparedRuleItem {
    fn from(value: AnyInterpretator) -> Self {
        PreparedRuleItem::Interpretator(value)
    }
}

impl From<FactScheme> for PreparedRuleItem {
    fn from(value: FactScheme) -> Self {
        PreparedRuleItem::Fact(value)
    }
}

impl<T> From<T> for PreparedRuleItem
where
    T: Into<PreparedAttributeItem>,
{
    fn from(value: T) -> Self {
        PreparedRuleItem::Attribute(value.into())
    }
}

impl From<NormalizerCallable> for PreparedRuleItem {
    fn from(value: NormalizerCallable) -> Self {
        PreparedRuleItem::Normalizer(value)
    }
}

impl From<NormalizedNormalizer> for PreparedRuleItem {
    fn from(value: NormalizedNormalizer) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

impl From<InflectedNormalizer> for PreparedRuleItem {
    fn from(value: InflectedNormalizer) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

impl<T> From<ConstNormalizer<T>> for PreparedRuleItem
where
    T: Clone + Into<FactValue> + fmt::Debug + Send + Sync + 'static,
{
    fn from(value: ConstNormalizer<T>) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

impl<F, O> From<FunctionNormalizer<F>> for PreparedRuleItem
where
    F: Fn(FactValue) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: FunctionNormalizer<F>) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

impl<F, G, O1, O2> From<FunctionFunctionNormalizer<F, G>> for PreparedRuleItem
where
    F: Fn(FactValue) -> O1 + Send + Sync + 'static,
    G: Fn(O1) -> O2 + Send + Sync + 'static,
    O2: Into<FactValue>,
{
    fn from(value: FunctionFunctionNormalizer<F, G>) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

impl<M, F, O> From<MorphFunctionNormalizer<M, F>> for PreparedRuleItem
where
    M: MorphCall + Send + Sync + 'static,
    F: Fn(String) -> O + Send + Sync + 'static,
    O: Into<FactValue>,
{
    fn from(value: MorphFunctionNormalizer<M, F>) -> Self {
        PreparedRuleItem::Normalizer(value.into())
    }
}

/// Подготавливает интерпретатор уровня атрибута.
///
/// Обычный атрибут превращается в [`AttributeInterpretator`], а варианты
/// с нормализатором — в [`AttributeNormalizerInterpretator`].
#[inline]
pub fn prepare_attribute_interpretator<T>(item: T) -> AnyInterpretator
where
    T: Into<PreparedAttributeItem>,
{
    match item.into() {
        PreparedAttributeItem::Attribute(attribute) => {
            AnyInterpretator::Attribute(AttributeInterpretator::new(attribute))
        }
        other => {
            let (attribute, normalizer) = other
                .into_attribute_normalizer()
                .expect("normalized attribute path must contain a normalizer");
            AnyInterpretator::AttributeNormalizer(AttributeNormalizerInterpretator::new(
                attribute, normalizer,
            ))
        }
    }
}

/// Подготавливает интерпретатор уровня токена.
#[inline]
pub fn prepare_token_interpretator<T>(item: T) -> AnyInterpretator
where
    T: Into<PreparedTokenItem>,
{
    match item.into() {
        PreparedTokenItem::Attribute(attribute) => prepare_attribute_interpretator(attribute),
        PreparedTokenItem::Normalizer(normalizer) => {
            AnyInterpretator::Normalizer(NormalizerInterpretator::new(normalizer))
        }
    }
}

/// Подготавливает интерпретатор уровня правила.
#[inline]
pub fn prepare_rule_interpretator<T>(item: T) -> AnyInterpretator
where
    T: Into<PreparedRuleItem>,
{
    match item.into() {
        PreparedRuleItem::Interpretator(interp) => interp,
        PreparedRuleItem::Fact(fact) => AnyInterpretator::Fact(FactInterpretator::new(fact)),
        PreparedRuleItem::Attribute(attribute) => prepare_attribute_interpretator(attribute),
        PreparedRuleItem::Normalizer(normalizer) => {
            AnyInterpretator::Normalizer(NormalizerInterpretator::new(normalizer))
        }
    }
}

/// Макрос декларативного описания типизированного факта.
///
/// Поддерживает формы:
/// - `fact!(F => [a, b])`;
/// - `fact!(pub F => [a, b])`;
/// - полную форму `fact!(pub struct F { a: String, ... })`.
///
/// Макрос генерирует:
/// - структуру с `Option<T>`-полями;
/// - реализации [`FactMeta`](crate::interpretation::fact::FactMeta) и [`Fact`](crate::interpretation::fact::Fact);
/// - константы-дескрипторы полей (`F::a`, `F::b`, ...).
#[macro_export]
macro_rules! fact {
    // Короткая Python-подобная форма:
    // fact!(F => [a, b]) разворачивается в struct F { a: String, b: String }.
    (
        $(#[$meta:meta])*
        $name:ident => [$($field_name:ident),* $(,)?]
    ) => {
        $crate::fact! {
            $(#[$meta])*
            struct $name {
                $(
                    $field_name: String,
                )*
            }
        }
    };

    // Короткая Python-подобная форма с явной видимостью:
    // fact!(pub F => [a, b]).
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident => [$($field_name:ident),* $(,)?]
    ) => {
        $crate::fact! {
            $(#[$meta])*
            $vis struct $name {
                $(
                    $field_name: String,
                )*
            }
        }
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_name:ident : $field_type:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                pub $field_name: Option<$field_type>,
            )*
        }

        impl $crate::interpretation::FactMeta for $name {
            const NAME: &'static str = stringify!($name);

            fn scheme() -> Option<$crate::interpretation::FactScheme> {
                Some($crate::interpretation::fact(
                    stringify!($name),
                    vec![
                        $(
                            $crate::interpretation::FactAttributeInput::from(stringify!($field_name)),
                        )*
                    ],
                ))
            }
        }

        impl $crate::interpretation::Fact for $name {
            fn fact_name(&self) -> &str {
                stringify!($name)
            }

            fn field_names(&self) -> &[&str] {
                &[$(stringify!($field_name)),*]
            }

            fn get_field(&self, name: &str) -> Option<$crate::interpretation::FactValue> {
                match name {
                    $(
                        stringify!($field_name) => {
                            $crate::interpretation::fact::field_to_value(&self.$field_name)
                        }
                    )*
                    _ => None,
                }
            }

            fn set_field(
                &mut self,
                name: &str,
                value: $crate::interpretation::FactValue,
            ) -> bool {
                match name {
                    $(
                        stringify!($field_name) => {
                            $crate::interpretation::fact::set_field_value(
                                &mut self.$field_name,
                                value
                            )
                        }
                    )*
                    _ => false,
                }
            }

            fn clone_box(&self) -> Box<dyn $crate::interpretation::Fact> {
                Box::new(self.clone())
            }
        }

        impl $name {
            #[allow(dead_code)]
            /// Возвращает дескриптор поля факта по строковому имени.
            pub fn attr(field: &'static str) -> $crate::interpretation::FactAttribute {
                $crate::interpretation::FactAttribute::new(stringify!($name), field)
            }

            $(
                #[allow(non_upper_case_globals, dead_code)]
                /// Дескриптор поля факта для использования в `.interpretation(...)`.
                pub const $field_name: $crate::interpretation::FactField<$name> =
                    $crate::interpretation::FactField::new(stringify!($field_name));
            )*
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::attribute::attribute;
    use crate::interpretation::normalizer::NormalizedItem;
    use crate::interpretation::FactAttributeInput;
    use crate::interpretation::FactValue;
    use crate::token::Tokenizer;

    fn chain_result(text: &'static str) -> InterpretatorResult<'static> {
        InterpretationValue::Chain(Chain::new(Tokenizer::new().tokenize(text), None))
    }

    #[test]
    fn prepared_attribute_item_accepts_repeatable_attribute() {
        let repeatable = RepeatableAttribute::new("City", "aliases");
        let prepared: PreparedAttributeItem = repeatable.into();

        match prepared {
            PreparedAttributeItem::Attribute(attribute) => {
                assert_eq!(attribute.fact().name(), "City");
                assert_eq!(attribute.name(), "aliases");
            }
            _ => panic!("expected plain attribute variant"),
        }
    }

    #[test]
    fn prepare_interpretator_accepts_direct_normalizer_types() {
        let token_interp = prepare_token_interpretator(NormalizedNormalizer);
        assert!(matches!(token_interp, AnyInterpretator::Normalizer(_)));
        assert_eq!(token_interp.label(), "normalized()");

        let rule_interp = prepare_rule_interpretator(ConstNormalizer::new("CITY"));
        assert!(matches!(rule_interp, AnyInterpretator::Normalizer(_)));
        assert_eq!(rule_interp.label(), "const(\"CITY\")");
    }

    #[test]
    fn interpretation_conversions_cover_consts_repeatable_and_custom_transforms() {
        let fact_attr = FactAttribute::new("City", "kind").with_const("capital");
        let interp = Interpretation::from(fact_attr);
        assert_eq!(interp.fact_name, "City");
        assert_eq!(interp.field_name, "kind");
        assert!(interp.has_const());
        assert_eq!(
            interp.const_value,
            Some(FactValue::Str("capital".to_string()))
        );

        let repeatable = Interpretation::from(RepeatableAttribute::new("City", "aliases"));
        assert!(repeatable.repeatable);
        assert!(repeatable.transforms.is_empty());

        let normalized = Interpretation::from(Attribute::new("City", "name", None).normalized());
        assert!(matches!(normalized.transforms[0], Transform::Normalized));

        let inflected = Interpretation::from(
            Attribute::new("City", "name", None).inflected_with(["nomn", "sing"]),
        );
        assert!(matches!(inflected.transforms[0], Transform::Inflected(_)));

        let custom = Interpretation::from(
            Attribute::new("City", "rank", None)
                .custom(|value: FactValue| value.as_str().unwrap().len() as i64),
        );
        assert!(matches!(custom.transforms[0], Transform::Custom(_)));
        if let Transform::Custom(call) = &custom.transforms[0] {
            assert_eq!(
                call(&FactValue::Str("abcd".to_string())),
                Some(FactValue::Int(4))
            );
        }
    }

    #[test]
    fn interpretator_error_display_and_from_fact_error_work() {
        let invalid = InterpretatorError::InvalidInput("bad".to_string());
        assert_eq!(format!("{invalid}"), "invalid interpretator input: bad");

        let type_error = InterpretatorError::TypeError("oops".to_string());
        assert_eq!(format!("{type_error}"), "interpretator type error: oops");

        let fact_error = InterpretatorError::from(FactError::KeyError("x".to_string()));
        assert_eq!(format!("{fact_error}"), "fact error: KeyError(x)");
    }

    #[test]
    fn interpretator_input_single_result_or_chain_covers_basic_paths() {
        let one_token = InterpretatorInput::from_tokens(
            Tokenizer::new().tokenize("LoNDon"),
            Some("CITY".to_string()),
        );
        let as_chain = one_token.single_result_or_chain().unwrap();
        assert_eq!(
            as_chain.normalized_value(),
            FactValue::Str("LoNDon".to_string())
        );
        if let InterpretationValue::Chain(chain) = as_chain {
            assert_eq!(chain.key, Some("CITY".to_string()));
        } else {
            panic!("expected chain");
        }

        let single_result = chain_result("Paris");
        let input = InterpretatorInput::new(
            vec![InterpretatorInputItem::Result(single_result.clone())],
            None,
        );
        assert_eq!(
            input.single_result_or_chain().unwrap().normalized_value(),
            single_result.normalized_value()
        );

        let multi_tokens = InterpretatorInput::from_tokens(Tokenizer::new().tokenize("A B"), None);
        assert_eq!(
            multi_tokens
                .single_result_or_chain()
                .unwrap()
                .normalized_value(),
            FactValue::Str("A B".to_string())
        );

        let mixed = InterpretatorInput::new(
            vec![
                InterpretatorInputItem::Token(Tokenizer::new().tokenize("A")[0].clone()),
                InterpretatorInputItem::Result(chain_result("B")),
            ],
            None,
        );
        let err = mixed.single_result_or_chain().unwrap_err();
        assert!(matches!(err, InterpretatorError::TypeError(_)));
    }

    #[test]
    fn normalizer_callable_supports_const_dynamic_and_type_errors() {
        let const_callable: NormalizerCallable = ConstNormalizer::new(5_i64).into();
        assert_eq!(const_callable.label(), "const(5)");
        assert_eq!(
            const_callable.call(&chain_result("ignored")).unwrap(),
            FactValue::Int(5)
        );

        let normalized_callable: NormalizerCallable = NormalizedNormalizer.into();
        assert_eq!(
            normalized_callable.call(&chain_result("LoNDon")).unwrap(),
            FactValue::Str("london".to_string())
        );
        let err = normalized_callable
            .call(&InterpretationValue::NormalizerResult(
                NormalizerResult::from_value("x"),
            ))
            .unwrap_err();
        assert!(matches!(err, InterpretatorError::TypeError(_)));
    }

    #[test]
    fn fact_interpretator_merges_matching_inputs_and_ignores_unrelated() {
        let city_scheme = crate::interpretation::fact(
            "City",
            vec![
                FactAttributeInput::from("name"),
                FactAttributeInput::from(attribute("alias").repeatable()),
            ],
        )
        .with_parents(["Location"]);
        let interp = FactInterpretator::new(city_scheme.clone());
        assert_eq!(interp.label(), "City");

        let attr_from_parent = InterpretationValue::AttributeResult(
            AttributeResult::new(chain_result("Moscow"))
                .with_attribute(Attribute::new("Location", "name", None)),
        );
        let unrelated_attr = InterpretationValue::AttributeResult(
            AttributeResult::new(chain_result("Ignored"))
                .with_attribute(Attribute::new("Country", "name", None)),
        );

        let capital_scheme =
            crate::interpretation::fact("Capital", vec![FactAttributeInput::from("name")])
                .with_parents(["City"]);
        let mut inner = InterpretatorFact::new(capital_scheme);
        inner.set("name", chain_result("Paris")).unwrap();
        let nested_fact = InterpretationValue::FactResult(FactResult::from_runtime_fact(inner));

        let input = InterpretatorInput::new(
            vec![
                InterpretatorInputItem::Result(attr_from_parent),
                InterpretatorInputItem::Result(nested_fact),
                InterpretatorInputItem::Result(unrelated_attr),
            ],
            None,
        );
        let out = interp.call(input).unwrap();
        let InterpretationValue::FactResult(result) = out else {
            panic!("expected FactResult");
        };
        let fact = result.into_runtime_fact().expect("runtime fact expected");
        let normalized = fact.normalized();
        assert_eq!(
            normalized.get("name"),
            Some(&FactValue::Str("Paris".to_string()))
        );
        assert_eq!(normalized.get("alias"), Some(&FactValue::List(Vec::new())));
    }

    #[test]
    fn attribute_and_normalizer_interpretators_wrap_and_label_results() {
        let attr = Attribute::new("City", "name", None);

        let attr_interp = AttributeInterpretator::new(attr.clone());
        assert_eq!(attr_interp.label(), "City.name");
        let out = attr_interp
            .call(InterpretatorInput::from_tokens(
                Tokenizer::new().tokenize("Moscow"),
                None,
            ))
            .unwrap();
        let InterpretationValue::AttributeResult(result) = out else {
            panic!("expected attribute result");
        };
        assert_eq!(result.attribute, Some(attr.clone()));
        assert_eq!(
            result.value.normalized_value(),
            FactValue::Str("Moscow".to_string())
        );

        let norm_interp = NormalizerInterpretator::new(NormalizedNormalizer.into());
        assert_eq!(norm_interp.label(), "normalized()");
        let out = norm_interp
            .call(InterpretatorInput::from_tokens(
                Tokenizer::new().tokenize("LoNDon"),
                None,
            ))
            .unwrap();
        let InterpretationValue::NormalizerResult(result) = out else {
            panic!("expected normalizer result");
        };
        assert_eq!(
            result.normalized_value(),
            FactValue::Str("london".to_string())
        );
        assert!(result.input.is_some());

        let attr_norm_interp =
            AttributeNormalizerInterpretator::new(attr, ConstNormalizer::new("CITY").into());
        assert_eq!(attr_norm_interp.label(), "City.name.const(\"CITY\")");
        let out = attr_norm_interp
            .call(InterpretatorInput::new(Vec::new(), None))
            .unwrap();
        let InterpretationValue::AttributeResult(result) = out else {
            panic!("expected attribute result");
        };
        assert_eq!(
            result.normalized_value(),
            FactValue::Str("CITY".to_string())
        );
    }

    #[test]
    fn any_and_prepare_interpretators_dispatch_variants() {
        let any = AnyInterpretator::Normalizer(NormalizerInterpretator::new(
            ConstNormalizer::new(1).into(),
        ));
        assert_eq!(any.label(), "const(1)");
        let out = any.call(InterpretatorInput::new(Vec::new(), None)).unwrap();
        assert_eq!(out.normalized_value(), FactValue::Int(1));

        let plain_attr = prepare_attribute_interpretator(Attribute::new("City", "name", None));
        assert!(matches!(plain_attr, AnyInterpretator::Attribute(_)));

        let morph_attr =
            prepare_attribute_interpretator(Attribute::new("City", "name", None).normalized());
        assert!(matches!(
            morph_attr,
            AnyInterpretator::AttributeNormalizer(_)
        ));

        let token_interp =
            prepare_token_interpretator(Attribute::new("City", "name", None).r#const("CITY"));
        assert!(matches!(
            token_interp,
            AnyInterpretator::AttributeNormalizer(_)
        ));

        let fact_scheme =
            crate::interpretation::fact("City", vec![FactAttributeInput::from("name")]);
        let rule_fact = prepare_rule_interpretator(fact_scheme);
        assert!(matches!(rule_fact, AnyInterpretator::Fact(_)));
    }

    #[test]
    fn normalized_inflected_const_and_custom_analogs() {
        let normalized = NormalizerInterpretator::new(NormalizedNormalizer.into());
        let out = normalized
            .call(InterpretatorInput::from_tokens(
                Tokenizer::new().tokenize("LoNDon"),
                None,
            ))
            .unwrap();
        assert_eq!(out.normalized_value(), FactValue::Str("london".to_string()));

        let inflected = NormalizerInterpretator::new(
            InflectedNormalizer::new(Some(vec!["nomn".to_string(), "sing".to_string()])).into(),
        );
        let out = inflected
            .call(InterpretatorInput::from_tokens(
                Tokenizer::new().tokenize("LoNDon"),
                None,
            ))
            .unwrap();
        assert_eq!(out.normalized_value(), FactValue::Str("london".to_string()));

        let const_interp = NormalizerInterpretator::new(ConstNormalizer::new(1_i64).into());
        let out = const_interp
            .call(InterpretatorInput::new(Vec::new(), None))
            .unwrap();
        assert_eq!(out.normalized_value(), FactValue::Int(1));

        let custom = NormalizerInterpretator::new(
            FunctionNormalizer::new(|v: FactValue| v.as_str().unwrap().len() as i64).into(),
        );
        let out = custom
            .call(InterpretatorInput::from_tokens(
                Tokenizer::new().tokenize("abc"),
                None,
            ))
            .unwrap();
        assert_eq!(out.normalized_value(), FactValue::Int(3));
    }
}
