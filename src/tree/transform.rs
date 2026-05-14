//! Трансформаторы дерева разбора.
//!
//! Пайплайн: [`Tree::normalized`](crate::tree::Tree::normalized) → [`collect_relations`] →
//! [`FormsRelationsGraph::validate`](crate::relations::FormsRelationsGraph::validate) →
//! [`apply_relations`].

use std::sync::Arc;

use crate::morph::models::Form;
use crate::predicates::constructors::TokenView;
use crate::relations::FormsRelationsGraph;
use crate::rule::constructors::RuleKind;
use crate::rule::registry::RuleRegistry;
use crate::token::AnyToken;

use super::{Leaf, Node, ParseChild, Tree};

/// Собирает отношения из дерева для последующей валидации.
///
/// Обходит дерево и находит узлы с `RuleKind::Relation`, добавляя
/// их главный лист и формы в граф.
pub fn collect_relations<'r>(
    tree: &Tree,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> FormsRelationsGraph {
    let mut graph = FormsRelationsGraph::new();
    collect_from_node(&tree.root, registry, tokens, &mut graph);
    graph
}

fn collect_from_node<'r>(
    node: &Arc<Node>,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
    graph: &mut FormsRelationsGraph,
) {
    if let Some(rule) = registry.get(node.rule_id) {
        if let RuleKind::Relation { relation, .. } = &rule.kind {
            if let Some((token_index, forms)) = find_main_leaf_forms(node, registry, tokens) {
                graph.add(relation, token_index, forms);
            }
        }
    }

    for child in &node.children {
        if let ParseChild::Node(child_node) = child {
            collect_from_node(child_node, registry, tokens, graph);
        }
    }
}

/// Находит главный лист (через `production.main`) и его морфоформы.
fn find_main_leaf_forms<'r>(
    node: &Arc<Node>,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> Option<(usize, Vec<Form>)> {
    let production = registry.production(node.rule_id, node.production_index)?;
    let main_idx = production.main;

    let main_child = node.children.get(main_idx)?;
    match main_child {
        ParseChild::Leaf(leaf) => {
            let forms = leaf_forms(leaf, tokens);
            Some((leaf.token_index, forms))
        }
        ParseChild::Node(child_node) => find_main_leaf_forms(child_node, registry, tokens),
    }
}

/// Возвращает формы для листа: суженные предикатом или полные из токена.
fn leaf_forms(leaf: &Leaf, tokens: &[AnyToken<'_>]) -> Vec<Form> {
    if let Some(narrowed) = &leaf.matched_forms {
        return narrowed.to_vec();
    }
    tokens
        .get(leaf.token_index)
        .and_then(|t| t.forms())
        .map(|f| f.to_vec())
        .unwrap_or_default()
}

/// Применяет ограничения к дереву, обновляя `matched_forms` листьев.
pub fn apply_relations(tree: Tree, graph: &FormsRelationsGraph) -> Tree {
    if graph.is_empty() {
        return tree;
    }
    let root = apply_to_node(&tree.root, graph);
    Tree::new(root, tree.range)
}

fn apply_to_node(node: &Arc<Node>, graph: &FormsRelationsGraph) -> Arc<Node> {
    let mut changed = false;
    let mut children = Vec::with_capacity(node.children.len());

    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => {
                if let Some(constrained) = graph.constrained_forms(leaf.token_index) {
                    children.push(ParseChild::Leaf(Arc::new(Leaf::with_forms(
                        leaf.predicate_id,
                        leaf.token_index,
                        leaf.span,
                        Some(constrained.to_vec().into()),
                    ))));
                    changed = true;
                } else {
                    children.push(ParseChild::Leaf(leaf.clone()));
                }
            }
            ParseChild::Node(child_node) => {
                let new_child = apply_to_node(child_node, graph);
                if !Arc::ptr_eq(&new_child, child_node) {
                    changed = true;
                }
                children.push(ParseChild::Node(new_child));
            }
        }
    }

    if changed {
        Arc::new(Node {
            rule_id: node.rule_id,
            production_index: node.production_index,
            rank: node.rank,
            children,
        })
    } else {
        node.clone()
    }
}

/// Полный пайплайн подготовки одного дерева: [`Tree::normalized`](crate::tree::Tree::normalized) →
/// [`collect_relations`] → [`FormsRelationsGraph::validate`](crate::relations::FormsRelationsGraph::validate) →
/// [`apply_relations`].
///
/// Возвращает `None`, если дерево полностью пустое после нормализации
/// или если отношения невалидны.
pub fn prepare_tree<'r>(
    tree: Tree,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> Option<Tree> {
    let tree = tree.normalized()?;
    let mut relations = collect_relations(&tree, registry, tokens);
    if relations.is_empty() {
        return Some(tree);
    }
    if relations.validate() {
        Some(apply_relations(tree, &relations))
    } else {
        None
    }
}
