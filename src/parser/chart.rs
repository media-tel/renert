//! Чарт Earley: колонки состояний и вспомогательная индексация.
//!
//! Этот модуль хранит промежуточные результаты разбора:
//! - [`Chart`] — весь набор колонок для входной последовательности;
//! - [`Column`] — состояния для одной позиции (колонки) чарта.
//!
//! Основные задачи:
//! - дедупликация состояний внутри колонки;
//! - быстрый доступ к "родителям", ожидающим конкретный нетерминал;
//! - выбор завершенных состояний по правилу.
//!
//! Дедупликация реализована как двухступенчатая:
//! 1. бакет по `stable_hash64` (`hashes: HashMap<u64, Vec<usize>>`);
//! 2. точное сравнение `State` через `Eq`.
//!
//! Это защищает от ложных срабатываний при коллизиях `u64`-хеша.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::rule::builder::RuleId;
use crate::rule::constructors::Term;
use crate::rule::registry::RuleRegistry;
use crate::token::Token;

use super::item::State;

/// Одна колонка чарта Earley.
///
/// Каждая колонка соответствует позиции в токенах:
/// - `index = 0` — стартовая колонка до первого токена;
/// - `index = i > 0` — позиция после токена `i - 1`.
#[derive(Debug, Clone)]
pub struct Column {
    /// Индекс колонки в чарте.
    pub index: usize,
    /// Индекс токена, соответствующего колонке (если есть).
    ///
    /// - `None` для стартовой колонки;
    /// - `Some(i)` для колонок, связанных с `chart.tokens[i]`.
    ///
    /// Также может быть `None` в служебном режиме `Chart::from_token_count`,
    /// где чарт строится без привязки к реальным токенам.
    pub token: Option<usize>,
    /// Состояния колонки в порядке добавления.
    states: Vec<State>,
    /// Бакеты хешей для дедупликации: `hash64 -> индексы состояний в states`.
    hashes: HashMap<u64, Vec<usize>>,
    /// Индекс ожидаемых нетерминалов:
    /// `rule_id -> индексы состояний, у которых next_term = NonTerminal(rule_id)`.
    states_index: HashMap<RuleId, Vec<usize>>,
}

impl Column {
    /// Создает пустую колонку.
    pub fn new(index: usize, token: Option<usize>) -> Self {
        Self {
            index,
            token,
            states: Vec::new(),
            hashes: HashMap::new(),
            states_index: HashMap::new(),
        }
    }

    /// Возвращает `true`, если это стартовая колонка (`index == 0`).
    #[inline]
    pub fn first(&self) -> bool {
        self.index == 0
    }

    /// Итератор по всем состояниям колонки.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &State> {
        self.states.iter()
    }

    /// Количество состояний в колонке.
    #[inline]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Возвращает состояние по индексу.
    #[inline]
    pub fn get(&self, idx: usize) -> Option<&State> {
        self.states.get(idx)
    }

    /// Возвращает завершенные состояния колонки для конкретного правила.
    ///
    /// Состояние считается подходящим, если:
    /// - `st.rule_id == rule_id`;
    /// - `st.completed(registry) == true`.
    pub fn matches<'b, 'r>(
        &'b self,
        rule_id: RuleId,
        registry: &'b RuleRegistry<'r>,
    ) -> impl Iterator<Item = &'b State> + use<'b, 'r> {
        self.states
            .iter()
            .filter(move |st| st.rule_id == rule_id && st.completed(registry))
    }

    /// Возвращает "родительские" состояния, ожидающие нетерминал `rule_id`.
    ///
    /// Это быстрый доступ к подмножеству `states`, построенный через `states_index`.
    /// Используется на шаге `complete` алгоритма Earley.
    pub fn parents<'b>(&'b self, rule_id: RuleId) -> impl Iterator<Item = &'b State> + 'b {
        self.states_index
            .get(&rule_id)
            .into_iter()
            .flat_map(|idxs| idxs.iter().map(|&i| &self.states[i]))
    }

    /// Добавляет состояние в колонку, если оно не является дубликатом.
    ///
    /// Алгоритм:
    /// 1. вычисляется `stable_hash64`;
    /// 2. проверяются кандидаты из того же хеш-бакета;
    /// 3. при точном равенстве (`State::eq`) вставка пропускается;
    /// 4. иначе состояние добавляется, и обновляется `states_index`.
    pub fn append<'g, 'r>(&mut self, state: State, registry: &'g RuleRegistry<'r>) {
        let h = state.stable_hash64();

        if let Some(candidates) = self.hashes.get(&h) {
            for &idx in candidates {
                if self.states[idx] == state {
                    return;
                }
            }
        }

        let idx = self.states.len();
        self.states.push(state);
        self.hashes.entry(h).or_default().push(idx);
        self.update_index(idx, registry);
    }

    /// Обновляет `states_index` для только что добавленного состояния.
    ///
    /// В индекс попадают только состояния, у которых:
    /// - состояние не завершено;
    /// - следующий символ — `NonTerminal(next_rule_id)`.
    fn update_index<'g, 'r>(&mut self, state_idx: usize, registry: &'g RuleRegistry<'r>) {
        let st = &self.states[state_idx];
        if st.completed(registry) {
            return;
        }

        let Some(next_term) = st.next_term(registry) else {
            return;
        };

        if let Term::Rule(next_rule) = next_term {
            if let Some(next_rule_id) = registry.rule_id_for_arc(next_rule) {
                self.states_index
                    .entry(next_rule_id)
                    .or_default()
                    .push(state_idx);
            }
        }
    }
}

/// Полный чарт Earley для входной последовательности.
///
/// Чарт содержит `N + 1` колонок для `N` токенов:
/// - колонка `0` — старт;
/// - колонки `1..=N` — позиции после соответствующих токенов.
#[derive(Debug, Clone)]
pub struct Chart<'a> {
    /// Исходные токены (owned или borrowed).
    pub tokens: Cow<'a, [Token<'a>]>,
    /// Колонки чарта.
    pub columns: Vec<Column>,
}

impl<'a> Chart<'a> {
    /// Создает чарт из владения вектором токенов.
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self::from_tokens(Cow::Owned(tokens))
    }

    /// Создает чарт из заимствованного среза токенов.
    pub fn from_slice(tokens: &'a [Token<'a>]) -> Self {
        Self::from_tokens(Cow::Borrowed(tokens))
    }

    /// Создает чарт только по количеству токенов.
    ///
    /// Используется во внутренних сценариях, когда нужны колонки,
    /// но сами токены не требуются.
    pub(crate) fn from_token_count(token_count: usize) -> Self {
        let mut columns = Vec::with_capacity(token_count + 1);
        columns.push(Column::new(0, None));
        for i in 0..token_count {
            columns.push(Column::new(i + 1, None));
        }
        Self {
            tokens: Cow::Owned(Vec::new()),
            columns,
        }
    }

    /// Общий конструктор чарта из `Cow` токенов.
    ///
    /// Для каждой колонки `i > 0` сохраняется `token = Some(i - 1)`.
    fn from_tokens(tokens: Cow<'a, [Token<'a>]>) -> Self {
        let mut columns = Vec::with_capacity(tokens.len() + 1);
        columns.push(Column::new(0, None));
        for i in 0..tokens.len() {
            columns.push(Column::new(i + 1, Some(i)));
        }
        Self { tokens, columns }
    }

    /// Количество колонок в чарте.
    #[inline]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Возвращает `true`, если в чарте нет колонок.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Возвращает последнюю колонку чарта.
    #[inline]
    pub fn last_column(&self) -> &Column {
        self.columns
            .last()
            .expect("Chart must have at least one column")
    }

    /// Возвращает последнюю колонку чарта (mutable).
    #[inline]
    pub fn last_column_mut(&mut self) -> &mut Column {
        self.columns
            .last_mut()
            .expect("Chart must have at least one column")
    }

    /// Возвращает колонку по индексу.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// Возвращает колонку по индексу (mutable).
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Column> {
        self.columns.get_mut(index)
    }

    /// Возвращает токен, связанный с колонкой `column_index`.
    ///
    /// Для стартовой колонки (и некоторых служебных колонок) вернет `None`.
    #[inline]
    pub fn token_at(&self, column_index: usize) -> Option<&Token<'a>> {
        let token_idx = self.columns.get(column_index)?.token?;
        self.tokens.get(token_idx)
    }

    /// Итератор по парам `(текущая_колонка, следующая_колонка)`.
    ///
    /// Для последней колонки второе значение — `None`.
    pub fn iter_indices(&self) -> impl Iterator<Item = (usize, Option<usize>)> {
        let len = self.columns.len();
        (0..len).map(move |i| {
            let next = if i + 1 < len { Some(i + 1) } else { None };
            (i, next)
        })
    }

    /// Возвращает завершенные состояния для `rule_id` по всем колонкам.
    pub fn matches<'b, 'r>(
        &'b self,
        rule_id: RuleId,
        registry: &'b RuleRegistry<'r>,
    ) -> impl Iterator<Item = &'b State> + use<'b, 'r> {
        self.columns
            .iter()
            .flat_map(move |col| col.matches(rule_id, registry))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::rule::builder::term;
    use crate::rule::constructors::{Production, Rule, Term};
    use crate::span::Span;
    use crate::token::TokenType;
    use crate::tree::{Leaf, Node, ParseChild};

    fn make_token(value: &'static str, start: usize) -> Token<'static> {
        Token::new(
            value,
            Span::new(start, start + value.len()),
            TokenType::Russian,
        )
    }

    struct TestRegistry {
        registry: RuleRegistry<'static>,
        rule0: RuleId,
        rule1: RuleId,
        rule2: RuleId,
    }

    fn make_registry() -> TestRegistry {
        let mut registry = RuleRegistry::new();
        let rule1_arc = term("a").build(()).named("rule1");
        let rule2_arc = term("b").build(());
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
        let rule2 = registry.add(rule2_arc);

        TestRegistry {
            registry,
            rule0,
            rule1,
            rule2,
        }
    }

    fn mk_leaf(idx: usize, value: &'static str, start: usize) -> Arc<Leaf> {
        let t = make_token(value, start);
        Arc::new(Leaf::new(0, idx, t.span))
    }

    #[test]
    fn chart_new_builds_columns_and_tokens() {
        let tokens = vec![make_token("one", 0), make_token("two", 4)];
        let chart = Chart::new(tokens.clone());

        assert_eq!(chart.len(), 3);
        assert_eq!(chart.tokens.as_ref(), tokens.as_slice());

        assert!(chart.columns[0].first());
        assert!(chart.columns[0].token.is_none());
        assert_eq!(chart.token_at(1).map(|t| t.value.as_ref()), Some("one"));
        assert_eq!(chart.token_at(2).map(|t| t.value.as_ref()), Some("two"));
    }

    #[test]
    fn column_append_deduplicates_equal_states() {
        let fx = make_registry();
        let mut col = Column::new(0, None);

        let st1 = State::new(fx.rule0, 0, 0, 0, 0, Arc::new(Node::new(fx.rule0, 0, 0)));
        let st2 = State::new(fx.rule0, 0, 0, 0, 0, Arc::new(Node::new(fx.rule0, 0, 0)));

        col.append(st1, &fx.registry);
        col.append(st2, &fx.registry);

        assert_eq!(col.len(), 1);
        assert_eq!(col.parents(fx.rule1).count(), 1);
    }

    #[test]
    fn column_append_keeps_states_with_different_child_identity() {
        let fx = make_registry();
        let mut col = Column::new(0, None);

        let base = Arc::new(Node::new(fx.rule0, 0, 0));
        let shared_leaf = mk_leaf(0, "a", 0);

        let node_with_shared_leaf = base.attached(ParseChild::Leaf(shared_leaf.clone()));
        let node_with_other_leaf = base.attached(ParseChild::Leaf(mk_leaf(1, "a", 0)));

        let st1 = State::new(fx.rule0, 0, 0, 0, 0, node_with_shared_leaf);
        let st2 = State::new(fx.rule0, 0, 0, 0, 0, node_with_other_leaf);

        col.append(st1, &fx.registry);
        col.append(st2, &fx.registry);

        assert_eq!(col.len(), 2);
        assert_eq!(col.parents(fx.rule1).count(), 2);
    }

    #[test]
    fn parents_index_ignores_completed_and_terminal_next_term() {
        let fx = make_registry();
        let mut col = Column::new(0, None);

        // Indexed: non-completed state with next non-terminal (Rule 0 -> Rule 1, dot=0)
        col.append(
            State::new(fx.rule0, 0, 0, 0, 0, Arc::new(Node::new(fx.rule0, 0, 0))),
            &fx.registry,
        );

        // Not indexed: completed state (Rule 1 has one term, dot=1)
        col.append(
            State::new(fx.rule1, 0, 1, 0, 1, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );

        // Not indexed: next term is terminal (Rule 2 -> "b", dot=0)
        col.append(
            State::new(fx.rule2, 0, 0, 0, 0, Arc::new(Node::new(fx.rule2, 0, 0))),
            &fx.registry,
        );

        assert_eq!(col.parents(fx.rule1).count(), 1);
        assert_eq!(col.parents(fx.rule2).count(), 0);
    }

    #[test]
    fn column_matches_returns_only_completed_states_for_rule() {
        let fx = make_registry();
        let mut col = Column::new(0, None);

        col.append(
            State::new(fx.rule1, 0, 0, 0, 0, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );
        col.append(
            State::new(fx.rule1, 0, 1, 0, 1, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );
        col.append(
            State::new(fx.rule2, 0, 1, 0, 1, Arc::new(Node::new(fx.rule2, 0, 0))),
            &fx.registry,
        );

        let matches: Vec<&State> = col.matches(fx.rule1, &fx.registry).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_id, fx.rule1);
        assert_eq!(matches[0].dot, 1);
    }

    #[test]
    fn chart_iter_indices_and_accessors_work() {
        let mut chart = Chart::new(vec![make_token("a", 0), make_token("b", 2)]);

        let pairs: Vec<(usize, Option<usize>)> = chart.iter_indices().collect();
        assert_eq!(pairs, vec![(0, Some(1)), (1, Some(2)), (2, None)]);

        assert_eq!(chart.last_column().index, 2);
        assert_eq!(chart.token_at(2).map(|t| t.value.as_ref()), Some("b"));

        assert!(chart.get(3).is_none());
        assert_eq!(chart.get(1).map(|c| c.index), Some(1));
        assert_eq!(chart.get_mut(1).map(|c| c.index), Some(1));
        assert_eq!(chart.last_column_mut().index, 2);
    }

    #[test]
    fn chart_matches_aggregates_completed_states_from_all_columns() {
        let fx = make_registry();
        let mut chart = Chart::new(vec![make_token("a", 0), make_token("a", 2)]);

        chart.columns[0].append(
            State::new(fx.rule1, 0, 0, 0, 0, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );
        chart.columns[1].append(
            State::new(fx.rule1, 0, 1, 0, 1, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );
        chart.columns[2].append(
            State::new(fx.rule1, 0, 1, 1, 2, Arc::new(Node::new(fx.rule1, 0, 0))),
            &fx.registry,
        );

        let matched: Vec<&State> = chart.matches(fx.rule1, &fx.registry).collect();
        let ranges: Vec<(usize, usize)> = matched.iter().map(|st| st.range()).collect();

        assert_eq!(matched.len(), 2);
        assert_eq!(ranges, vec![(0, 1), (1, 2)]);
    }
}
