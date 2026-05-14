//! Значения полей фактов.
//!
//! `FactValue` — универсальный тип для хранения значений полей фактов.
//! Реализует [`serde::Serialize`] и [`FactValue::to_json_value`] для интеграции с `serde_json` и БД;
//! вариант [`FactValue::Opaque`] на wire кодируется как JSON `null`.

use serde::Serialize;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct OpaqueValue {
    inner: Arc<dyn Any + Send + Sync>,
}

impl OpaqueValue {
    #[inline]
    pub fn new<T>(value: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self {
            inner: Arc::new(value),
        }
    }

    #[inline]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.inner.as_ref().downcast_ref::<T>()
    }
}

impl fmt::Debug for OpaqueValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<opaque>")
    }
}

impl PartialEq for OpaqueValue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Значение поля факта.
///
/// Поддерживает основные типы данных, необходимые для интерпретации.
#[derive(Debug, Clone, PartialEq)]
pub enum FactValue {
    /// Строковое значение
    Str(String),
    /// Целочисленное значение
    Int(i64),
    /// Булево значение
    Bool(bool),
    /// Список значений (для repeatable)
    List(Vec<FactValue>),
    /// Объектное значение в виде JSON-подобных пар ключ-значение.
    Object(Vec<(String, FactValue)>),
    /// Opaque runtime value (for wrapper-level dynamic objects).
    Opaque(OpaqueValue),
}

impl FactValue {
    #[inline]
    pub fn opaque<T>(value: T) -> Self
    where
        T: Any + Send + Sync,
    {
        FactValue::Opaque(OpaqueValue::new(value))
    }

    #[inline]
    pub fn as_opaque<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        match self {
            FactValue::Opaque(value) => value.downcast_ref::<T>(),
            _ => None,
        }
    }

    /// Проверить, является ли значение строкой.
    #[inline]
    pub fn is_str(&self) -> bool {
        matches!(self, FactValue::Str(_))
    }

    /// Проверить, является ли значение целым числом.
    #[inline]
    pub fn is_int(&self) -> bool {
        matches!(self, FactValue::Int(_))
    }

    /// Проверить, является ли значение булевым.
    #[inline]
    pub fn is_bool(&self) -> bool {
        matches!(self, FactValue::Bool(_))
    }

    /// Получить строковое значение, если это строка.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            FactValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Получить целочисленное значение, если это число.
    #[inline]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            FactValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Получить булево значение, если это bool.
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FactValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Попытаться преобразовать в String, поглощая значение.
    #[inline]
    pub fn into_string(self) -> Option<String> {
        match self {
            FactValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Добавить значение в список (если текущ — List), иначе создать List из двух элементов.
    ///
    /// # Поведение
    ///
    /// - Если текущее значение — это `List`, добавляет `other` в конец списка.
    /// - Если добавляемое значение (`other`) — это `List`, то его элементы распаковываются и добавляются по отдельности (без вложенных списков).
    /// - Если текущее значение — скаляр, создаёт новый список из двух элементов: текущего и добавляемого.
    /// - Если добавляемое значение — это `List`, то скаляр и элементы списка объединяются в один список.
    ///
    /// # Примеры
    ///
    /// ```
    /// use renert::interpretation::FactValue;
    /// // Скаляр + скаляр
    /// let v1 = FactValue::Str("a".into());
    /// let v2 = FactValue::Str("b".into());
    /// let result = v1.push_into_list(v2);
    /// assert!(matches!(result, FactValue::List(_)));
    ///
    /// // List + скаляр
    /// let list = FactValue::List(vec![FactValue::Int(1)]);
    /// let result = list.push_into_list(FactValue::Int(2));
    /// // Результат: [1, 2]
    ///
    /// // List + List (распаковывается)
    /// let list1 = FactValue::List(vec![FactValue::Int(1)]);
    /// let list2 = FactValue::List(vec![FactValue::Int(2), FactValue::Int(3)]);
    /// let result = list1.push_into_list(list2);
    /// // Результат: [1, 2, 3] (без вложения)
    /// ```
    pub fn push_into_list(self, other: FactValue) -> FactValue {
        match self {
            FactValue::List(mut v) => {
                // Если добавляем List, распаковываем его элементы
                match other {
                    FactValue::List(items) => {
                        v.extend(items);
                        FactValue::List(v)
                    }
                    _ => {
                        v.push(other);
                        FactValue::List(v)
                    }
                }
            }
            prev => {
                // Если добавляем List в скаляр, распаковываем List
                match other {
                    FactValue::List(items) => {
                        let mut result = vec![prev];
                        result.extend(items);
                        FactValue::List(result)
                    }
                    _ => FactValue::List(vec![prev, other]),
                }
            }
        }
    }

    /// JSON-представление значения для обмена с внешними системами (`serde_json`, БД).
    ///
    /// Скаляры и вложенные `List`/`Object` отображаются в стандартные JSON-типы.
    /// Вариант [`FactValue::Opaque`] сериализуется как JSON `null` (тип не сериализуем на wire).
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            FactValue::Str(s) => serde_json::Value::String(s.clone()),
            FactValue::Int(n) => serde_json::Value::Number((*n).into()),
            FactValue::Bool(b) => serde_json::Value::Bool(*b),
            FactValue::List(items) => {
                serde_json::Value::Array(items.iter().map(Self::to_json_value).collect())
            }
            FactValue::Object(pairs) => {
                let mut map = serde_json::Map::new();
                for (k, v) in pairs {
                    map.insert(k.clone(), v.to_json_value());
                }
                serde_json::Value::Object(map)
            }
            FactValue::Opaque(_) => serde_json::Value::Null,
        }
    }
}

impl Serialize for FactValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_json_value().serialize(serializer)
    }
}

impl fmt::Display for FactValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FactValue::Str(s) => write!(f, "{}", s),
            FactValue::Int(n) => write!(f, "{}", n),
            FactValue::Bool(b) => write!(f, "{}", b),
            FactValue::List(v) => {
                let parts: Vec<String> = v.iter().map(|x| format!("{}", x)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            FactValue::Object(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            FactValue::Opaque(_) => write!(f, "<opaque>"),
        }
    }
}

impl From<String> for FactValue {
    fn from(s: String) -> Self {
        FactValue::Str(s)
    }
}

impl From<&str> for FactValue {
    fn from(s: &str) -> Self {
        FactValue::Str(s.to_string())
    }
}

impl From<i64> for FactValue {
    fn from(n: i64) -> Self {
        FactValue::Int(n)
    }
}

impl From<i32> for FactValue {
    fn from(n: i32) -> Self {
        FactValue::Int(n as i64)
    }
}

impl From<bool> for FactValue {
    fn from(b: bool) -> Self {
        FactValue::Bool(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fact_value_to_json_scalars() {
        assert_eq!(FactValue::Str("x".into()).to_json_value(), json!("x"));
        assert_eq!(FactValue::Int(42).to_json_value(), json!(42));
        assert_eq!(FactValue::Bool(false).to_json_value(), json!(false));
        let s = serde_json::to_string(&FactValue::Str("π".into())).expect("serialize");
        assert_eq!(s, "\"π\"");
    }

    #[test]
    fn fact_value_to_json_nested() {
        let v = FactValue::Object(vec![
            (
                "items".into(),
                FactValue::List(vec![FactValue::Int(1), FactValue::Str("a".into())]),
            ),
            ("flag".into(), FactValue::Bool(true)),
        ]);
        assert_eq!(
            v.to_json_value(),
            json!({ "items": [1, "a"], "flag": true })
        );
    }

    #[test]
    fn fact_value_opaque_json() {
        let v = FactValue::opaque(42_i64);
        assert_eq!(v.to_json_value(), json!(null));
        let s = serde_json::to_string(&v).expect("serialize opaque");
        assert_eq!(s, "null");
    }

    #[test]
    fn fact_value_json_roundtrip_through_value() {
        let v = FactValue::List(vec![FactValue::Object(vec![(
            "k".into(),
            FactValue::Int(7),
        )])]);
        let json_str = serde_json::to_string(&v).expect("to_string");
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("valid JSON for DB/API");
        assert_eq!(parsed, json!([{ "k": 7 }]));
    }

    #[test]
    fn test_str_value() {
        let v = FactValue::Str("тест".into());
        assert!(v.is_str());
        assert!(!v.is_int());
        assert_eq!(v.as_str(), Some("тест"));
    }

    #[test]
    fn test_int_value() {
        let v = FactValue::Int(42);
        assert!(v.is_int());
        assert!(!v.is_str());
        assert_eq!(v.as_int(), Some(42));
    }

    #[test]
    fn test_bool_value() {
        let v = FactValue::Bool(true);
        assert!(v.is_bool());
        assert_eq!(v.as_bool(), Some(true));
    }

    #[test]
    fn test_from_string() {
        let borrowed: FactValue = "hello".into();
        assert_eq!(borrowed, FactValue::Str("hello".into()));

        let owned: FactValue = String::from("world").into();
        assert_eq!(owned, FactValue::Str("world".into()));
    }

    #[test]
    fn test_from_i64() {
        let v: FactValue = 123i64.into();
        assert_eq!(v, FactValue::Int(123));
    }

    #[test]
    fn test_from_i32_and_bool() {
        let i32_value: FactValue = 7i32.into();
        assert_eq!(i32_value, FactValue::Int(7));

        let bool_value: FactValue = false.into();
        assert_eq!(bool_value, FactValue::Bool(false));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", FactValue::Str("abc".into())), "abc");
        assert_eq!(format!("{}", FactValue::Int(42)), "42");
        assert_eq!(format!("{}", FactValue::Bool(true)), "true");
    }

    #[test]
    fn test_display_object_and_opaque() {
        let object = FactValue::Object(vec![
            ("name".to_string(), FactValue::Str("Moscow".into())),
            ("rank".to_string(), FactValue::Int(1)),
        ]);
        assert_eq!(format!("{}", object), "{name: Moscow, rank: 1}");

        let opaque = FactValue::opaque(10_i64);
        assert_eq!(format!("{}", opaque), "<opaque>");
    }

    #[test]
    fn test_type_guards_and_accessors_negative_paths() {
        let int_value = FactValue::Int(1);
        assert!(!int_value.is_str());
        assert!(!int_value.is_bool());
        assert_eq!(int_value.as_str(), None);
        assert_eq!(int_value.as_bool(), None);

        let str_value = FactValue::Str("x".into());
        assert!(!str_value.is_int());
        assert_eq!(str_value.as_int(), None);
    }

    #[test]
    fn test_into_string() {
        let v = FactValue::Str("test".into());
        assert_eq!(v.into_string(), Some("test".into()));

        let v = FactValue::Int(42);
        assert_eq!(v.into_string(), None);
    }

    #[test]
    fn test_push_into_list_scalar_plus_list_flattens() {
        let scalar = FactValue::Str("head".into());
        let list = FactValue::List(vec![
            FactValue::Str("tail1".into()),
            FactValue::Str("tail2".into()),
        ]);

        let result = scalar.push_into_list(list);
        assert_eq!(
            result,
            FactValue::List(vec![
                FactValue::Str("head".into()),
                FactValue::Str("tail1".into()),
                FactValue::Str("tail2".into()),
            ])
        );
    }

    #[test]
    fn test_opaque_value_downcast_and_debug() {
        let opaque = OpaqueValue::new(42_i64);
        assert_eq!(opaque.downcast_ref::<i64>(), Some(&42));
        assert_eq!(opaque.downcast_ref::<String>(), None);
        assert_eq!(format!("{:?}", opaque), "<opaque>");
    }

    #[test]
    fn test_opaque_value_partial_eq_is_identity_based() {
        let value = OpaqueValue::new(String::from("x"));
        let same = value.clone();
        let other = OpaqueValue::new(String::from("x"));

        assert_eq!(value, same);
        assert_ne!(value, other);
    }

    #[test]
    fn test_fact_value_opaque_helpers() {
        let value = FactValue::opaque(99_i64);
        assert_eq!(value.as_opaque::<i64>(), Some(&99));
        assert_eq!(value.as_opaque::<String>(), None);

        let plain = FactValue::Int(99);
        assert_eq!(plain.as_opaque::<i64>(), None);
    }

    #[test]
    fn test_list_value_creation() {
        let v = FactValue::List(vec![FactValue::Str("a".into()), FactValue::Int(1)]);

        match v {
            FactValue::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], FactValue::Str("a".into()));
                assert_eq!(items[1], FactValue::Int(1));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_list_display() {
        let v = FactValue::List(vec![
            FactValue::Str("a".into()),
            FactValue::Str("b".into()),
            FactValue::Int(3),
        ]);

        assert_eq!(format!("{}", v), "[a, b, 3]");
    }

    #[test]
    fn test_push_into_list_from_scalar() {
        let v1 = FactValue::Str("one".into());
        let v2 = FactValue::Str("two".into());

        let list = v1.push_into_list(v2);

        match list {
            FactValue::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], FactValue::Str("one".into()));
                assert_eq!(items[1], FactValue::Str("two".into()));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_push_into_list_existing_list() {
        let list = FactValue::List(vec![FactValue::Int(1), FactValue::Int(2)]);

        let result = list.push_into_list(FactValue::Int(3));

        match result {
            FactValue::List(items) => {
                assert_eq!(
                    items,
                    vec![FactValue::Int(1), FactValue::Int(2), FactValue::Int(3),]
                );
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_push_into_list_list_to_list() {
        let list1 = FactValue::List(vec![FactValue::Str("a".into())]);

        let list2 = FactValue::List(vec![FactValue::Str("b".into()), FactValue::Str("c".into())]);

        let result = list1.push_into_list(list2);

        match result {
            FactValue::List(items) => {
                assert_eq!(
                    items,
                    vec![
                        FactValue::Str("a".into()),
                        FactValue::Str("b".into()),
                        FactValue::Str("c".into()),
                    ]
                );
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_nested_list_not_created() {
        let list = FactValue::List(vec![FactValue::Int(1)]);
        let result = list.push_into_list(FactValue::List(vec![FactValue::Int(2)]));

        match result {
            FactValue::List(items) => {
                // важно: без вложенных списков
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], FactValue::Int(1)));
                assert!(matches!(items[1], FactValue::Int(2)));
            }
            _ => panic!("expected List"),
        }
    }
}
