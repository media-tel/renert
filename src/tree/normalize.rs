use std::sync::Arc;

use super::types::{Node, ParseChild};

/// Нормализует узел: удаляет пустые нетерминальные подветки.
pub(super) fn normalize_node(node: Arc<Node>) -> Option<Arc<Node>> {
    let mut children = Vec::with_capacity(node.children.len());

    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => children.push(ParseChild::Leaf(leaf.clone())),
            ParseChild::Node(child_node) => {
                if let Some(normalized) = normalize_node(child_node.clone()) {
                    children.push(ParseChild::Node(normalized));
                }
            }
        }
    }

    if children.is_empty() {
        return None;
    }

    Some(Arc::new(Node {
        rule_id: node.rule_id,
        production_index: node.production_index,
        rank: node.rank,
        children,
    }))
}
