use std::sync::Arc;

use super::types::{Node, ParseChild};

/// Элемент обхода дерева.
pub type WalkItem = ParseChild;

/// Состояние обхода дерева разбора в глубину.
///
/// Создаётся через [`Tree::walk`](crate::tree::Tree::walk); порядок элементов задаётся реализацией
/// [`Iterator`] ниже (preorder).
pub struct TreeWalker {
    stack: Vec<WalkItem>,
}

impl TreeWalker {
    /// Создает обходчик с указанным корнем.
    pub(super) fn new(root: Arc<Node>) -> Self {
        Self {
            stack: vec![WalkItem::Node(root)],
        }
    }
}

/// Обход дерева в глубину (preorder).
impl Iterator for TreeWalker {
    type Item = WalkItem;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.stack.pop()?;
        if let WalkItem::Node(node) = &item {
            for child in node.children.iter().rev() {
                self.stack.push(child.clone());
            }
        }
        Some(item)
    }
}
