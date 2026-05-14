//! Earley-состояние (`State`) и его семантика равенства/хеширования.
//!
//! Этот модуль задает ключевую единицу чарта Earley: элемент вида
//! `(rule_id, production_index, dot, start_col, stop_col, node)`.
//!
//! Важно: в проекте дедупликация состояний опирается на `Eq`/`Hash` для `State`.
//! Поэтому здесь намеренно сравнивается не весь `node` рекурсивно, а:
//! - структурные поля состояния (`rule_id`, `production_index`, `dot`, диапазон колонок);
//! - идентичность детей узла по указателям `Arc` (адреса через `Arc::as_ptr`).
//!
//! Такой подход повторяет практическую семантику yargy-подобного пайплайна:
//! важен не "текстово одинаковый" узел, а конкретная структура вывода,
//! собранная из конкретных дочерних ссылок.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::rule::builder::RuleId;
use crate::rule::constructors::Term;
use crate::rule::registry::RuleRegistry;

use crate::tree::{Node, ParseChild};

/// Состояние Earley (item) в чарте.
///
/// Состояние описывает "прогресс" разбора конкретной продукции:
/// - `rule_id` и `production_index` определяют продукцию;
/// - `dot` — позиция в этой продукции;
/// - `start_col` и `stop_col` задают покрываемый диапазон колонок;
/// - `node` хранит уже собранную часть дерева разбора.
#[derive(Debug, Clone)]
pub struct State {
    /// Идентификатор правила, к которому относится состояние.
    pub rule_id: RuleId,
    /// Индекс продукции внутри правила.
    pub production_index: usize,
    /// Позиция "точки" в продукции (сколько терминов уже покрыто).
    pub dot: usize,
    /// Колонка, в которой состояние началось.
    pub start_col: usize,
    /// Текущая колонка, в которой состояние заканчивается.
    pub stop_col: usize,
    /// Частично собранный узел дерева разбора.
    pub node: Arc<Node>,
}

impl State {
    /// Создает новое состояние Earley.
    #[inline]
    pub fn new(
        rule_id: RuleId,
        production_index: usize,
        dot: usize,
        start_col: usize,
        stop_col: usize,
        node: Arc<Node>,
    ) -> Self {
        Self {
            rule_id,
            production_index,
            dot,
            start_col,
            stop_col,
            node,
        }
    }

    /// Проверяет, завершено ли состояние (`dot >= len(production.terms)`).
    ///
    /// Паника:
    /// паникует, если `rule_id` не найден в `RuleRegistry`.
    #[inline]
    pub fn completed<'r>(&self, registry: &RuleRegistry<'r>) -> bool {
        let prod = registry
            .production(self.rule_id, self.production_index)
            .unwrap_or_else(|| {
                panic!(
                    "invalid rule_id in State: rule_id={:?}, production_index={}",
                    self.rule_id, self.production_index
                )
            });
        self.dot >= prod.terms.len()
    }

    /// Возвращает следующий ожидаемый символ продукции, если он есть.
    ///
    /// В отличие от [`Self::completed`], не паникует:
    /// - `None`, если правило не найдено;
    /// - `None`, если `production_index` некорректен;
    /// - `None`, если `dot` уже в конце продукции.
    #[inline]
    pub fn next_term<'g, 'r>(&self, registry: &'g RuleRegistry<'r>) -> Option<&'g Term<'r>> {
        let prod = registry.production(self.rule_id, self.production_index)?;
        prod.terms.get(self.dot)
    }

    /// Возвращает диапазон колонок, покрытый состоянием: `(start_col, stop_col)`.
    #[inline]
    pub fn range(&self) -> (usize, usize) {
        (self.start_col, self.stop_col)
    }

    /// Итератор по "идентичностям" детей `node` через адреса `Arc`.
    ///
    /// Это внутренняя основа для `Eq`/`Hash`: сравнение выполняется по
    /// идентичности ссылок на детей, а не по глубокому рекурсивному равенству.
    #[inline]
    fn node_children_ptrs(&self) -> impl Iterator<Item = usize> + '_ {
        self.node.children.iter().map(|ch| match ch {
            ParseChild::Leaf(l) => Arc::as_ptr(l) as usize,
            ParseChild::Node(n) => Arc::as_ptr(n) as usize,
        })
    }

    /// Вычисляет стабильный `u64`-хеш состояния на основе реализации [`Hash`].
    ///
    /// Удобно для вспомогательной дедупликации по хеш-ведрам.
    /// Не является криптографическим хешем.
    #[inline]
    pub fn stable_hash64(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

/// Равенство состояния для дедупликации в чарте.
///
/// Сравниваются:
/// - `rule_id`, `production_index`, `dot`, `start_col`, `stop_col`;
/// - указатели `Arc` всех детей `node` в том же порядке.
///
/// Не сравниваются рекурсивно:
/// - внутренние поля `node`, если при этом идентичности детей уже различны;
/// - "текстовое совпадение" деревьев при разных `Arc`.
impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        if self.rule_id != other.rule_id
            || self.production_index != other.production_index
            || self.dot != other.dot
            || self.start_col != other.start_col
            || self.stop_col != other.stop_col
        {
            return false;
        }

        // Порядок детей важен: это часть ключа состояния.
        let mut a = self.node_children_ptrs();
        let mut b = other.node_children_ptrs();

        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (Some(x), Some(y)) if x == y => continue,
                _ => return false,
            }
        }
    }
}

impl Eq for State {}

/// Хеш состояния согласован с [`PartialEq`] и использует те же компоненты ключа.
impl Hash for State {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rule_id.hash(state);
        self.production_index.hash(state);
        self.dot.hash(state);
        self.start_col.hash(state);
        self.stop_col.hash(state);
        for p in self.node_children_ptrs() {
            p.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::*;
    use crate::rule::builder::term;
    use crate::rule::constructors::{Production, Rule, Term};
    use crate::span::Span;
    use crate::token::{Token, TokenType};
    use crate::tree::{Leaf, ParseChild};

    struct TestRegistry {
        registry: RuleRegistry<'static>,
        rule0: RuleId,
        rule1: RuleId,
    }

    fn make_registry() -> TestRegistry {
        let mut registry = RuleRegistry::new();
        let rule1_arc = term("x").build(()).named("rule1");
        let rule0_arc = Arc::new(Rule::new(vec![Production::new(
            vec![rule1_arc.clone().into()],
            None,
        )]));

        let rule0 = registry.add(rule0_arc);
        let rule1 = match registry.production(rule0, 0).and_then(|p| p.terms.first()) {
            Some(Term::Rule(rule)) => registry
                .rule_id_for_arc(rule)
                .expect("nested rule id for rule1 must be present"),
            _ => panic!("rule0 must start with nested rule1"),
        };

        TestRegistry {
            registry,
            rule0,
            rule1,
        }
    }

    fn token(value: &'static str, start: usize) -> Token<'static> {
        Token::new(
            value,
            Span::new(start, start + value.len()),
            TokenType::Russian,
        )
    }

    fn leaf(value: &'static str, start: usize) -> Arc<Leaf> {
        let tok = token(value, start);
        Arc::new(Leaf::new(0, start, tok.span))
    }

    #[test]
    fn completed_checks_dot_against_production_len() {
        let fx = make_registry();
        let node = Arc::new(Node::new(fx.rule1, 0, 0));

        let not_completed = State::new(fx.rule1, 0, 0, 0, 0, node.clone());
        let completed = State::new(fx.rule1, 0, 1, 0, 1, node.clone());
        let over_completed = State::new(fx.rule1, 0, 2, 0, 1, node);

        assert!(!not_completed.completed(&fx.registry));
        assert!(completed.completed(&fx.registry));
        assert!(over_completed.completed(&fx.registry));
    }

    #[test]
    fn next_term_returns_nonterminal_terminal_and_none() {
        let fx = make_registry();
        let node = Arc::new(Node::new(fx.rule0, 0, 0));

        let nonterminal_first = State::new(fx.rule0, 0, 0, 0, 0, node.clone());
        assert!(matches!(
            nonterminal_first.next_term(&fx.registry),
            Some(Term::Rule(_))
        ));

        let terminal_first = State::new(fx.rule1, 0, 0, 0, 0, node.clone());
        assert!(matches!(
            terminal_first.next_term(&fx.registry),
            Some(Term::Pred(_))
        ));

        let at_end = State::new(fx.rule1, 0, 1, 0, 1, node.clone());
        assert!(at_end.next_term(&fx.registry).is_none());

        let bad_rule = State::new(RuleId(999), 0, 0, 0, 0, node.clone());
        assert!(bad_rule.next_term(&fx.registry).is_none());

        let bad_production = State::new(fx.rule1, 99, 0, 0, 0, node);
        assert!(bad_production.next_term(&fx.registry).is_none());
    }

    #[test]
    fn range_returns_start_and_stop_columns() {
        let rid = make_registry().rule1;
        let state = State::new(rid, 0, 0, 3, 7, Arc::new(Node::new(rid, 0, 0)));
        assert_eq!(state.range(), (3, 7));
    }

    #[test]
    fn equality_uses_child_pointer_identity_not_node_identity() {
        let shared_leaf = leaf("x", 0);
        let base = Arc::new(Node::new(make_registry().rule1, 0, 0));

        let node_a = base.attached(ParseChild::Leaf(shared_leaf.clone()));
        let node_b = base.attached(ParseChild::Leaf(shared_leaf));

        let rid = make_registry().rule1;
        let st_a = State::new(rid, 0, 0, 0, 0, node_a);
        let st_b = State::new(rid, 0, 0, 0, 0, node_b);

        assert_eq!(st_a, st_b);
        assert_eq!(st_a.stable_hash64(), st_b.stable_hash64());
    }

    #[test]
    fn equality_distinguishes_different_child_arc_identity() {
        let rid = make_registry().rule1;
        let base = Arc::new(Node::new(rid, 0, 0));

        let node_a = base.attached(ParseChild::Leaf(leaf("x", 0)));
        let node_b = base.attached(ParseChild::Leaf(leaf("x", 0)));

        let st_a = State::new(rid, 0, 0, 0, 0, node_a);
        let st_b = State::new(rid, 0, 0, 0, 0, node_b);

        assert_ne!(st_a, st_b);
    }

    #[test]
    fn equality_depends_on_child_order() {
        let l1 = leaf("a", 0);
        let l2 = leaf("b", 2);

        let node_ab = Arc::new(Node {
            rule_id: make_registry().rule1,
            production_index: 0,
            rank: 0,
            children: vec![ParseChild::Leaf(l1.clone()), ParseChild::Leaf(l2.clone())],
        });
        let node_ba = Arc::new(Node {
            rule_id: make_registry().rule1,
            production_index: 0,
            rank: 0,
            children: vec![ParseChild::Leaf(l2), ParseChild::Leaf(l1)],
        });

        let rid = make_registry().rule1;
        let st_ab = State::new(rid, 0, 0, 0, 0, node_ab);
        let st_ba = State::new(rid, 0, 0, 0, 0, node_ba);

        assert_ne!(st_ab, st_ba);
    }

    #[test]
    fn hashset_deduplicates_equal_states_and_keeps_distinct_ones() {
        let shared_leaf = leaf("x", 0);
        let rid = make_registry().rule1;
        let base = Arc::new(Node::new(rid, 0, 0));

        let same_a = State::new(
            rid,
            0,
            0,
            0,
            0,
            base.attached(ParseChild::Leaf(shared_leaf.clone())),
        );
        let same_b = State::new(
            rid,
            0,
            0,
            0,
            0,
            base.attached(ParseChild::Leaf(shared_leaf)),
        );
        let different = State::new(
            rid,
            0,
            0,
            0,
            0,
            base.attached(ParseChild::Leaf(leaf("x", 0))),
        );

        let mut set = HashSet::new();
        set.insert(same_a);
        set.insert(same_b);
        set.insert(different);

        assert_eq!(set.len(), 2);
    }

    #[test]
    #[should_panic(expected = "invalid rule_id in State")]
    fn completed_panics_for_unknown_rule_id() {
        let fx = make_registry();
        let st = State::new(
            RuleId(777),
            0,
            0,
            0,
            0,
            Arc::new(Node::new(RuleId(777), 0, 0)),
        );
        let _ = st.completed(&fx.registry);
    }
}
