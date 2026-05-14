//! Диапазоны в тексте.
//!
//! Модуль содержит [`Span`] — полуинтервал вида `[start, stop)`.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Диапазон в тексте в формате полуинтервала `[start, stop)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Начало диапазона (включительно).
    pub start: usize,
    /// Конец диапазона (не включается).
    pub stop: usize,
}

impl Span {
    /// Создает новый диапазон `[start, stop)`.
    ///
    /// # Panics
    ///
    /// Паникует, если `start > stop`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use renert::span::Span;
    ///
    /// let span = Span::new(2, 5);
    /// assert_eq!(span.start, 2);
    /// assert_eq!(span.stop, 5);
    /// ```
    pub fn new(start: usize, stop: usize) -> Self {
        assert!(start <= stop, "Start must be less or equal to stop");
        Span { start, stop }
    }

    /// Возвращает длину диапазона (`stop - start`).
    pub fn len(&self) -> usize {
        self.stop - self.start
    }

    /// Возвращает `true`, если диапазон пустой.
    pub fn is_empty(&self) -> bool {
        self.start == self.stop
    }

    /// Проверяет, входит ли позиция `pos` в диапазон.
    ///
    /// Для полуинтервала `[start, stop)` значение `stop` не входит в диапазон.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use renert::span::Span;
    ///
    /// let span = Span::new(2, 5);
    /// assert!(span.contains(2));
    /// assert!(span.contains(4));
    /// assert!(!span.contains(5));
    /// ```
    pub fn contains(&self, pos: usize) -> bool {
        self.start <= pos && pos < self.stop
    }

    /// Проверяет пересекаются ли два диапазона
    pub fn overlaps(&self, other: &Span) -> bool {
        self.start < other.stop && other.start < self.stop
    }

    /// Проверяет, соприкасаются ли два диапазона по границе.
    ///
    /// Например, `[0, 5)` и `[5, 8)` соприкасаются, но не пересекаются.
    pub fn touches(&self, other: &Span) -> bool {
        self.stop == other.start || other.stop == self.start
    }

    /// Объединяет два диапазона, если они пересекаются или соприкасаются.
    ///
    /// Возвращает `Some(Span)` для объединения и `None`, если диапазоны раздельны.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use renert::span::Span;
    ///
    /// let a = Span::new(0, 5);
    /// let b = Span::new(5, 8);
    /// let c = Span::new(9, 10);
    ///
    /// assert_eq!(a.merge(&b), Some(Span::new(0, 8)));
    /// assert_eq!(a.merge(&c), None);
    /// ```
    pub fn merge(&self, other: &Span) -> Option<Span> {
        if self.overlaps(other) || self.touches(other) {
            let start = usize::min(self.start, other.start);
            let stop = usize::max(self.stop, other.stop);
            Some(Span { start, stop })
        } else {
            None
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {})", self.start, self.stop)
    }
}

#[cfg(test)]
mod tests;
