//! Дерево разбора и утилиты обхода/нормализации.
//!
//! Модуль содержит:
//! - структуры дерева: [`Leaf`], [`Node`], [`ParseChild`], [`Tree`];
//! - обход в глубину: [`TreeWalker`] / [`WalkItem`];
//! - нормализацию дерева (удаление пустых нетерминальных ветвей);
//! - детерминированное сравнение деревьев через [`Ord`].

mod normalize;
mod ordering;
mod types;
mod walker;

pub mod transform;

pub use self::types::{Leaf, Node, ParseChild, Tree};
pub use self::walker::{TreeWalker, WalkItem};

#[cfg(test)]
mod tests;
