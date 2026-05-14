//! Общие типы payload для интерпретации правил.
//!
//! Используются как в слое правил, так и в мосте к слою интерпретации.

use std::fmt;
use std::sync::Arc;

/// Функция пользовательского преобразования значения.
pub type CoreTransformFn<V> = dyn Fn(&V) -> Option<V> + Send + Sync;

/// Описание преобразования значения в интерпретации.
#[derive(Clone)]
pub enum CoreTransform<V> {
    /// Нормализация значения (например, лемматизация строки).
    Normalized,
    /// Инфлексия значения по набору форм.
    Inflected(Vec<String>),
    /// Пользовательское преобразование.
    ///
    /// Возвращает `Some(new_value)`, если преобразование применимо,
    /// и `None`, если значение оставить без изменения.
    Custom(Arc<CoreTransformFn<V>>),
}

impl<V> fmt::Debug for CoreTransform<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreTransform::Normalized => write!(f, "Normalized"),
            CoreTransform::Inflected(forms) => write!(f, "Inflected({forms:?})"),
            CoreTransform::Custom(_) => write!(f, "Custom(<fn>)"),
        }
    }
}

/// Универсальная структура payload интерпретации.
#[derive(Debug, Clone, Default)]
pub struct CoreInterpretation<V, T = CoreTransform<V>> {
    /// Имя факта (сущности), в которую будет записано значение.
    pub fact_name: String,
    /// Имя поля факта.
    pub field_name: String,
    /// Константное значение, если поле задается напрямую.
    pub const_value: Option<V>,
    /// Цепочка трансформаций, применяемых к значению.
    pub transforms: Vec<T>,
    /// Признак того, что поле допускает повторяющиеся значения.
    pub repeatable: bool,
}
