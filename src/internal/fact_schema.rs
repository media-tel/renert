//! Общие схемы фактов и нормализованные записи.
//!
//! Этот модуль содержит структуры, которые нужны сразу нескольким слоям:
//! `interpretation`, `rule` и `parser`.

use crate::interpretation::attribute::{
    attribute, Attribute, AttributeScheme, AttributeSchemeBase, RepeatableAttribute,
    RepeatableAttributeScheme,
};
use crate::interpretation::FactValue;
use crate::span::Span;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Упорядоченное представление `as_json` в стиле yargy.
pub type OrderedFactMap = Vec<(String, FactValue)>;

/// Ошибки операций с фактами.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactError {
    /// Ошибка доступа к несуществующему ключу/полю.
    KeyError(String),
}

impl std::fmt::Display for FactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FactError::KeyError(key) => write!(f, "KeyError({key})"),
        }
    }
}

impl std::error::Error for FactError {}

/// Входной элемент для `prepare_attribute(...)`.
#[derive(Debug, Clone, PartialEq)]
pub enum FactAttributeInput {
    /// Имя атрибута.
    Name(String),
    /// Полная схема обычного атрибута.
    Scheme(AttributeScheme),
    /// Полная схема repeatable-атрибута.
    RepeatableScheme(RepeatableAttributeScheme),
    /// Универсальная схема атрибута.
    Base(AttributeSchemeBase),
}

impl From<&str> for FactAttributeInput {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

impl From<String> for FactAttributeInput {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<AttributeScheme> for FactAttributeInput {
    fn from(value: AttributeScheme) -> Self {
        Self::Scheme(value)
    }
}

impl From<RepeatableAttributeScheme> for FactAttributeInput {
    fn from(value: RepeatableAttributeScheme) -> Self {
        Self::RepeatableScheme(value)
    }
}

impl From<AttributeSchemeBase> for FactAttributeInput {
    fn from(value: AttributeSchemeBase) -> Self {
        Self::Base(value)
    }
}

/// Подготовленная схема атрибута (результат `prepare_attribute`).
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedAttributeScheme {
    /// Обычный атрибут.
    Single(AttributeScheme),
    /// Repeatable-атрибут.
    Repeatable(RepeatableAttributeScheme),
}

impl PreparedAttributeScheme {
    /// Возвращает имя атрибута.
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            PreparedAttributeScheme::Single(scheme) => &scheme.name,
            PreparedAttributeScheme::Repeatable(scheme) => &scheme.name,
        }
    }

    /// Конструирует атрибут для конкретного имени факта.
    #[inline]
    pub fn construct(&self, fact_name: &str) -> ConstructedAttribute {
        match self {
            PreparedAttributeScheme::Single(scheme) => {
                ConstructedAttribute::Single(scheme.construct(fact_name.to_string()))
            }
            PreparedAttributeScheme::Repeatable(scheme) => {
                ConstructedAttribute::Repeatable(scheme.construct(fact_name.to_string()))
            }
        }
    }
}

/// Сконструированный атрибут факта.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstructedAttribute {
    /// Обычный атрибут.
    Single(Attribute),
    /// Repeatable-атрибут.
    Repeatable(RepeatableAttribute),
}

impl ConstructedAttribute {
    /// Возвращает имя атрибута.
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            ConstructedAttribute::Single(attribute) => attribute.name(),
            ConstructedAttribute::Repeatable(attribute) => attribute.name(),
        }
    }

    /// Возвращает `true`, если атрибут является repeatable.
    #[inline]
    pub fn is_repeatable(&self) -> bool {
        matches!(self, ConstructedAttribute::Repeatable(_))
    }

    /// Возвращает значение по умолчанию в нормализованном виде.
    ///
    /// Для repeatable-атрибута возвращается пустой список.
    #[inline]
    pub fn default_value(&self) -> Option<FactValue> {
        match self {
            ConstructedAttribute::Single(attribute) => attribute.default_value(),
            ConstructedAttribute::Repeatable(_) => Some(FactValue::List(Vec::new())),
        }
    }

    /// Возвращает ссылку на значение по умолчанию для обычного атрибута.
    ///
    /// Для repeatable-атрибута всегда `None`.
    #[inline]
    pub fn default_value_ref(&self) -> Option<&FactValue> {
        match self {
            ConstructedAttribute::Single(attribute) => attribute.default_value_ref(),
            ConstructedAttribute::Repeatable(_) => None,
        }
    }
}

/// Динамическая схема факта (аналог Python `fact(...)`).
#[derive(Debug, Clone, PartialEq)]
pub struct FactScheme {
    /// Имя факта.
    pub name: String,
    /// Множество имён предков (включая сам факт).
    pub lineage: BTreeSet<String>,
    /// Порядок атрибутов в сериализации/проекциях.
    pub attributes_order: Vec<String>,
    /// Карта атрибутов схемы.
    pub attributes: BTreeMap<String, ConstructedAttribute>,
}

impl FactScheme {
    /// Создаёт пустую схему факта с заданным именем.
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut lineage = BTreeSet::new();
        lineage.insert(name.clone());
        Self {
            name,
            lineage,
            attributes_order: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    /// Добавляет родителей в lineage схемы.
    #[inline]
    pub fn with_parents<I, S>(mut self, parents: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for parent in parents {
            self.lineage.insert(parent.into());
        }
        self
    }

    /// Проверяет, является ли схема подклассом `other`.
    #[inline]
    pub fn is_subclass_of(&self, other: &FactScheme) -> bool {
        self.lineage.contains(&other.name)
    }

    /// Возвращает атрибут схемы по ключу.
    #[inline]
    pub fn attribute(&self, key: &str) -> Option<&ConstructedAttribute> {
        self.attributes.get(key)
    }

    /// Пытается создать запись факта из именованных аргументов.
    #[inline]
    pub fn try_new_record(
        &self,
        kwargs: BTreeMap<String, FactValue>,
    ) -> Result<FactRecord, FactError> {
        FactRecord::try_new(self.clone(), kwargs)
    }

    /// Rust-аналог вызова `FactClass(**kwargs)`.
    #[inline]
    pub fn call(&self, kwargs: BTreeMap<String, FactValue>) -> Result<FactRecord, FactError> {
        self.try_new_record(kwargs)
    }
}

/// Экземпляр факта времени выполнения (аналог Python-объекта `Fact`).
#[derive(Debug, Clone, PartialEq)]
pub struct FactRecord {
    /// Схема факта.
    pub scheme: FactScheme,
    /// Значения атрибутов по ключам.
    pub attributes: BTreeMap<String, Option<FactValue>>,
    /// Сырые проекции, полученные из рантайма интерпретации.
    pub raw: Option<FactRecordRaw>,
}

impl FactRecord {
    /// Пытается создать запись факта из схемы и набора полей.
    pub fn try_new(
        scheme: FactScheme,
        kwargs: BTreeMap<String, FactValue>,
    ) -> Result<Self, FactError> {
        let mut kwargs = kwargs;
        for key in kwargs.keys() {
            if !scheme.attributes.contains_key(key) {
                return Err(FactError::KeyError(key.clone()));
            }
        }

        let mut attributes = BTreeMap::new();
        for key in &scheme.attributes_order {
            let value = if let Some(value) = kwargs.remove(key) {
                Some(value)
            } else {
                match scheme.attributes.get(key) {
                    Some(attribute) => attribute.default_value(),
                    None => None,
                }
            };
            attributes.insert(key.clone(), value);
        }

        Ok(Self {
            scheme,
            attributes,
            raw: None,
        })
    }

    /// Создаёт запись факта или паникует при ошибке валидации ключей.
    pub fn new(scheme: FactScheme, kwargs: BTreeMap<String, FactValue>) -> Self {
        Self::try_new(scheme, kwargs).expect("failed to construct FactRecord")
    }

    /// Возвращает имя факта.
    #[inline]
    pub fn name(&self) -> &str {
        &self.scheme.name
    }

    /// Возвращает значение атрибута по ключу.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&FactValue> {
        self.attributes.get(key).and_then(|value| value.as_ref())
    }

    /// Возвращает JSON-подобное представление факта.
    #[inline]
    pub fn as_json(&self) -> OrderedFactMap {
        if let Some(raw) = &self.raw {
            raw.as_json.clone()
        } else {
            let mut data = Vec::with_capacity(self.scheme.attributes_order.len());
            for key in &self.scheme.attributes_order {
                if let Some(Some(value)) = self.attributes.get(key) {
                    data.push((key.clone(), value.clone()));
                }
            }
            data
        }
    }

    /// Упорядоченный JSON-объект полей факта (как [`Self::as_json`], без схемы).
    ///
    /// Порядок ключей совпадает с порядком пар в [`Self::as_json`] (т. е. с
    /// `scheme.attributes_order`): для этого в зависимости крейта включён `serde_json`
    /// с feature `preserve_order`.
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (k, v) in self.as_json() {
            map.insert(k, v.to_json_value());
        }
        serde_json::Value::Object(map)
    }

    /// Возвращает spans факта, отсортированные по `start`.
    #[inline]
    pub fn spans(&self) -> Vec<Span> {
        let mut spans = if let Some(raw) = &self.raw {
            raw.spans.clone()
        } else {
            Vec::new()
        };
        if spans.len() > 1 {
            spans.sort_by_key(|span| span.start);
        }
        spans
    }
}

impl fmt::Display for FactRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}(", self.scheme.name)?;

        for key in &self.scheme.attributes_order {
            let value = self.attributes.get(key).and_then(|value| value.as_ref());
            write!(f, "    {}=", key)?;
            match value {
                Some(value) => write_fact_value_display(f, value)?,
                None => write!(f, "None")?,
            }

            if key != self.scheme.attributes_order.last().unwrap_or(key) {
                writeln!(f, ",")?;
            } else {
                writeln!(f)?;
            }
        }

        write!(f, ")")
    }
}

fn write_fact_value_display(f: &mut fmt::Formatter<'_>, value: &FactValue) -> fmt::Result {
    match value {
        FactValue::Str(value) => write!(f, "'{}'", value),
        FactValue::Int(value) => write!(f, "{}", value),
        FactValue::Bool(value) => write!(f, "{}", value),
        FactValue::List(items) => {
            write!(f, "[")?;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    write!(f, ", ")?;
                }
                write_fact_value_display(f, item)?;
            }
            write!(f, "]")
        }
        FactValue::Object(items) => {
            write!(f, "{{")?;
            for (index, (key, value)) in items.iter().enumerate() {
                if index > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}: ", key)?;
                write_fact_value_display(f, value)?;
            }
            write!(f, "}}")
        }
        FactValue::Opaque(_) => write!(f, "<opaque>"),
    }
}

/// Снимок сырых проекций, передаваемый из runtime в `FactRecord`.
#[derive(Debug, Clone, PartialEq)]
pub struct FactRecordRaw {
    /// Кэш JSON-подобного представления.
    pub as_json: OrderedFactMap,
    /// Кэш диапазонов исходного текста.
    pub spans: Vec<Span>,
}

/// Нормализует входной дескриптор атрибута к [`PreparedAttributeScheme`].
#[inline]
pub fn prepare_attribute<T>(item: T) -> PreparedAttributeScheme
where
    T: Into<FactAttributeInput>,
{
    match item.into() {
        FactAttributeInput::Name(name) => PreparedAttributeScheme::Single(attribute(name)),
        FactAttributeInput::Base(base) => match base {
            AttributeSchemeBase::Single(scheme) => PreparedAttributeScheme::Single(scheme),
            AttributeSchemeBase::Repeatable(scheme) => PreparedAttributeScheme::Repeatable(scheme),
        },
        FactAttributeInput::Scheme(scheme) => PreparedAttributeScheme::Single(scheme),
        FactAttributeInput::RepeatableScheme(scheme) => PreparedAttributeScheme::Repeatable(scheme),
    }
}

/// Строит динамическую схему факта в стиле yargy.
pub fn fact<T, I>(name: impl Into<String>, attributes: I) -> FactScheme
where
    T: Into<FactAttributeInput>,
    I: IntoIterator<Item = T>,
{
    let mut scheme = FactScheme::new(name);

    for item in attributes {
        let prepared = prepare_attribute(item);
        let key = prepared.name().to_string();
        assert!(
            !scheme.attributes.contains_key(&key),
            "duplicate fact attribute: {key}"
        );
        scheme.attributes_order.push(key.clone());
        let attribute = prepared.construct(&scheme.name);
        scheme.attributes.insert(key, attribute);
    }

    scheme
}
