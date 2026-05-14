use std::sync::Arc;

use crate::rule::builder::RuleId;
use crate::span::Span;

use super::{Leaf, Node, ParseChild, Tree};

fn leaf(idx: usize, start: usize, stop: usize) -> Arc<Leaf> {
    Arc::new(Leaf::new(0, idx, Span::new(start, stop)))
}

#[test]
fn leaf_indices_collects_depth_first_order() {
    let nested = Arc::new(Node {
        rule_id: RuleId(2),
        production_index: 0,
        rank: 0,
        children: vec![ParseChild::Leaf(leaf(2, 6, 9))],
    });

    let root = Arc::new(Node {
        rule_id: RuleId(1),
        production_index: 0,
        rank: 0,
        children: vec![
            ParseChild::Leaf(leaf(0, 0, 2)),
            ParseChild::Node(nested),
            ParseChild::Leaf(leaf(1, 3, 5)),
        ],
    });

    let tree = Tree::new(root, (0, 3));
    assert_eq!(tree.leaf_indices(), vec![0, 2, 1]);
}

#[test]
fn span_uses_min_start_max_stop() {
    let root = Arc::new(Node {
        rule_id: RuleId(1),
        production_index: 0,
        rank: 0,
        children: vec![
            ParseChild::Leaf(leaf(0, 5, 7)),
            ParseChild::Leaf(leaf(1, 1, 3)),
            ParseChild::Leaf(leaf(2, 8, 10)),
        ],
    });
    let tree = Tree::new(root, (0, 3));
    assert_eq!(tree.span(), Span::new(1, 10));
}

#[test]
fn normalized_returns_none_when_all_branches_are_empty() {
    let root = Arc::new(Node {
        rule_id: RuleId(1),
        production_index: 0,
        rank: 0,
        children: vec![ParseChild::Node(Arc::new(Node::new(RuleId(2), 0, 0)))],
    });
    let tree = Tree::new(root, (0, 0));

    assert!(tree.normalized().is_none());
}

#[test]
fn tree_ordering_by_range_start_and_length() {
    let t_start0_short = Tree::new(Arc::new(Node::new(RuleId(1), 0, 0)), (0, 1));
    let t_start0_long = Tree::new(Arc::new(Node::new(RuleId(1), 0, 0)), (0, 3));
    let t_start1 = Tree::new(Arc::new(Node::new(RuleId(1), 0, 0)), (1, 2));

    assert!(t_start0_long < t_start0_short); // longer first if same start
    assert!(t_start0_short < t_start1); // smaller start first
}

#[test]
fn tree_ordering_by_rule_id_and_rank_for_same_range() {
    let same_range = (0, 1);

    let rule1 = Tree::new(Arc::new(Node::new(RuleId(1), 0, 0)), same_range);
    let rule2 = Tree::new(Arc::new(Node::new(RuleId(2), 0, 0)), same_range);
    assert!(rule1 < rule2);

    let rank0 = Tree::new(Arc::new(Node::new(RuleId(5), 0, 0)), same_range);
    let rank1 = Tree::new(Arc::new(Node::new(RuleId(5), 0, 1)), same_range);
    assert!(rank0 < rank1);
}
