//! Реестр верхнеуровневых правил грамматики из модуля `rule`.
//!
//! Реестр назначает последовательные [`RuleId`](crate::rule::builder::RuleId) и предоставляет:
//! - поиск правила по id;
//! - структурную валидацию;
//! - консервативную фильтрацию продукций по lookahead для шага `predict` в Earley.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::RuleValidationError;
use crate::predicates::constructors::TokenView;
pub use crate::rule::builder::RuleId;
use crate::rule::constructors::{EmptyProduction, Production, Rule, RuleKind, Term};

/// Реестр нормализованных правил, индексируемых последовательными [`RuleId`].
///
/// Реестр хранит:
/// - верхнеуровневые правила, добавленные через [`Self::add`];
/// - все достижимые вложенные узлы правил, найденные BFS-обходом.
#[derive(Debug)]
pub struct RuleRegistry<'a> {
    /// Все зарегистрированные узлы графа правил: [`RuleId`] → нормализованное [`Rule`].
    rules: HashMap<RuleId, Arc<Rule<'a>>>,
    /// Дедупликация по идентичности `Arc`: указатель на узел → назначенный [`RuleId`].
    rule_ids: HashMap<usize, RuleId>,
    /// Следующий свободный идентификатор (монотонно растёт при [`Self::add`]).
    next_id: RuleId,
    /// Число вызовов [`Self::add`] (только верхний уровень, без учёта вложенных узлов BFS).
    top_level_len: usize,
}

impl<'a> Default for RuleRegistry<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> RuleRegistry<'a> {
    /// Создает пустой реестр.
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            rule_ids: HashMap::new(),
            next_id: RuleId(0),
            top_level_len: 0,
        }
    }

    /// Добавляет верхнеуровневое правило и возвращает назначенный `RuleId`.
    ///
    /// В текущем graph-based API `RuleBuilder::build(id)` не встраивает id в объект правила,
    /// поэтому реестр является источником истины для верхнеуровневых идентификаторов.
    pub fn add(&mut self, rule: Arc<Rule<'a>>) -> RuleId {
        // Keep registry-facing rules normalized to simplify prediction/validation logic.
        let root = rule.normalized();
        let id = self.alloc_id();
        self.rules.insert(id, root.clone());
        self.rule_ids.insert(Self::rule_key(&root), id);
        self.top_level_len += 1;

        for node in root.walk_bfs() {
            let key = Self::rule_key(&node);
            if self.rule_ids.contains_key(&key) {
                continue;
            }
            let node_id = self.alloc_id();
            self.rule_ids.insert(key, node_id);
            self.rules.insert(node_id, node);
        }
        id
    }

    /// Возвращает следующий id, который будет назначен в [`Self::add`].
    pub fn next_id(&self) -> RuleId {
        self.next_id
    }

    /// Возвращает правило по id.
    pub fn get(&self, id: RuleId) -> Option<&Arc<Rule<'a>>> {
        self.rules.get(&id)
    }

    /// Возвращает количество верхнеуровневых правил.
    pub fn len(&self) -> usize {
        self.top_level_len
    }

    /// Возвращает `true`, если в реестре нет ни одного правила.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Возвращает `true`, если правило с таким id существует.
    pub fn contains(&self, id: RuleId) -> bool {
        self.rules.contains_key(&id)
    }

    /// Возвращает id узла правила по его `Arc`-идентичности.
    pub(crate) fn rule_id_for_arc(&self, rule: &Arc<Rule<'a>>) -> Option<RuleId> {
        self.rule_ids.get(&Self::rule_key(rule)).copied()
    }

    /// Возвращает продукцию по id правила и индексу продукции.
    ///
    /// Для оберток (`Named`, `Forward`, bounded-варианты и т.п.) поиск делегируется
    /// во внутреннее правило.
    pub(crate) fn production(&self, rule_id: RuleId, index: usize) -> Option<&Production<'a>> {
        let rule = self.get(rule_id)?;
        self.production_for_rule(rule, index)
    }

    /// Возвращает pipeline-key для продукции, если правило является pipeline-оберткой.
    pub(crate) fn pipeline_value(&self, rule_id: RuleId, index: usize) -> Option<&str> {
        let rule = self.get(rule_id)?;
        self.pipeline_value_for_rule(rule, index)
    }

    /// Возвращает индексы продукций, которые стоит предсказывать.
    ///
    /// Для нормализованного `Base` это прямые индексы в `productions`.
    /// Для ряда оберток (`Named`, `Interpretation` и др.) индексы делегируются
    /// во внутреннее правило. Для непрозрачных случаев метод работает консервативно.
    pub(crate) fn predicted_production_indices(
        &self,
        rule_id: RuleId,
        lookahead: Option<&dyn TokenView>,
    ) -> Vec<usize> {
        let Some(rule) = self.get(rule_id) else {
            return Vec::new();
        };

        let mut visiting = HashSet::new();
        self.predicted_indices_for_rule(rule, lookahead, &mut visiting)
    }

    /// Рекурсивно вычисляет индексы предсказываемых продукций для конкретного узла правила.
    fn predicted_indices_for_rule(
        &self,
        rule: &Arc<Rule<'a>>,
        lookahead: Option<&dyn TokenView>,
        visiting: &mut HashSet<usize>,
    ) -> Vec<usize> {
        let key = Self::rule_key(rule);
        if !visiting.insert(key) {
            return Vec::new();
        }

        let result = if let Some(inner) = rule.kind.transparent_inner_rule() {
            self.predicted_indices_for_rule(inner, lookahead, visiting)
        } else {
            match &rule.kind {
                RuleKind::Base { productions } => match lookahead {
                    None => (0..productions.len()).collect(),
                    Some(token) => productions
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, prod)| {
                            self.production_matches_lookahead(prod, token)
                                .then_some(idx)
                        })
                        .collect(),
                },
                // Usually removed by `normalized()`, but handle conservatively.
                RuleKind::Or { rules } => match lookahead {
                    None => (0..rules.len()).collect(),
                    Some(token) => rules
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, child)| {
                            let mut start_visiting = HashSet::new();
                            let mut nullable_visiting = HashSet::new();
                            (self.rule_can_start_with_token(child, token, &mut start_visiting)
                                || self.rule_is_nullable(child, &mut nullable_visiting))
                            .then_some(idx)
                        })
                        .collect(),
                },
                RuleKind::Forward { inner } => inner
                    .read()
                    .clone()
                    .map(|r| self.predicted_indices_for_rule(&r, lookahead, visiting))
                    .unwrap_or_default(),
                RuleKind::Empty => vec![0],
                _ => Vec::new(),
            }
        };

        visiting.remove(&key);
        result
    }

    /// Внутренняя реализация поиска продукции с обходом оберток.
    fn production_for_rule<'b>(
        &'b self,
        rule: &'b Arc<Rule<'a>>,
        index: usize,
    ) -> Option<&'b Production<'a>> {
        if let Some(inner) = rule.kind.transparent_inner_rule() {
            return self.production_for_rule(inner, index);
        }
        match &rule.kind {
            RuleKind::Base { productions } => productions.get(index),
            RuleKind::Forward { inner } => {
                let inner_rule = inner.read().clone()?;
                let inner_id = self.rule_id_for_arc(&inner_rule)?;
                let inner_rule = self.get(inner_id)?;
                self.production_for_rule(inner_rule, index)
            }
            _ => None,
        }
    }

    fn pipeline_value_for_rule<'b>(
        &'b self,
        rule: &'b Arc<Rule<'a>>,
        index: usize,
    ) -> Option<&'b str> {
        // Pipeline has special logic: check own values first, then delegate deeper.
        if let RuleKind::Pipeline { rule, values, .. } = &rule.kind {
            return values
                .get(index)
                .map(|v| v.as_str())
                .or_else(|| self.pipeline_value_for_rule(rule, index));
        }
        if let Some(inner) = rule.kind.transparent_inner_rule() {
            return self.pipeline_value_for_rule(inner, index);
        }
        match &rule.kind {
            RuleKind::Forward { inner } => {
                let inner_rule = inner.read().clone()?;
                let inner_id = self.rule_id_for_arc(&inner_rule)?;
                let inner_rule = self.get(inner_id)?;
                self.pipeline_value_for_rule(inner_rule, index)
            }
            _ => None,
        }
    }

    /// Проверяет, может ли продукция соответствовать текущему lookahead-токену.
    fn production_matches_lookahead(
        &self,
        production: &Production<'a>,
        token: &dyn TokenView,
    ) -> bool {
        for term in &production.terms {
            match term {
                Term::Pred(predicate) => return predicate.check(token),
                Term::Rule(rule) => {
                    let mut start_visiting = HashSet::new();
                    if self.rule_can_start_with_token(rule, token, &mut start_visiting) {
                        return true;
                    }

                    let mut nullable_visiting = HashSet::new();
                    if self.rule_is_nullable(rule, &mut nullable_visiting) {
                        continue;
                    }

                    return false;
                }
            }
        }

        // Entire production is nullable (or epsilon) -> keep conservatively.
        true
    }

    /// Проверяет, может ли продукция начинаться с переданного токена.
    fn production_can_start_with_token(
        &self,
        production: &Production<'a>,
        token: &dyn TokenView,
        visiting: &mut HashSet<usize>,
    ) -> bool {
        for term in &production.terms {
            match term {
                Term::Pred(predicate) => return predicate.check(token),
                Term::Rule(rule) => {
                    if self.rule_can_start_with_token(rule, token, visiting) {
                        return true;
                    }

                    let mut nullable_visiting = HashSet::new();
                    if self.rule_is_nullable(rule, &mut nullable_visiting) {
                        continue;
                    }

                    return false;
                }
            }
        }

        false
    }

    /// Проверяет, является ли продукция nullable (может вывести пустую цепочку).
    fn production_is_nullable(
        &self,
        production: &Production<'a>,
        visiting: &mut HashSet<usize>,
    ) -> bool {
        production.terms.iter().all(|term| match term {
            Term::Pred(_) => false,
            Term::Rule(rule) => self.rule_is_nullable(rule, visiting),
        })
    }

    /// Проверяет, может ли правило начинаться с заданного токена.
    fn rule_can_start_with_token(
        &self,
        rule: &Arc<Rule<'a>>,
        token: &dyn TokenView,
        visiting: &mut HashSet<usize>,
    ) -> bool {
        let key = Self::rule_key(rule);
        if !visiting.insert(key) {
            return false;
        }

        let result = if let Some(inner) = rule.kind.transparent_inner_rule() {
            self.rule_can_start_with_token(inner, token, visiting)
        } else {
            match &rule.kind {
                RuleKind::Base { productions } => productions
                    .iter()
                    .any(|prod| self.production_can_start_with_token(prod, token, visiting)),
                RuleKind::Or { rules } => rules
                    .iter()
                    .any(|child| self.rule_can_start_with_token(child, token, visiting)),
                RuleKind::Forward { inner } => inner
                    .read()
                    .clone()
                    .map(|r| self.rule_can_start_with_token(&r, token, visiting))
                    .unwrap_or(false),
                _ => false,
            }
        };

        visiting.remove(&key);
        result
    }

    /// Проверяет, является ли правило nullable.
    fn rule_is_nullable(&self, rule: &Arc<Rule<'a>>, visiting: &mut HashSet<usize>) -> bool {
        let key = Self::rule_key(rule);
        if !visiting.insert(key) {
            return false;
        }

        let result = if matches!(
            &rule.kind,
            RuleKind::Optional { .. } | RuleKind::RepeatableOptional { .. } | RuleKind::Empty
        ) {
            true
        } else if let Some(inner) = rule.kind.transparent_inner_rule() {
            self.rule_is_nullable(inner, visiting)
        } else {
            match &rule.kind {
                RuleKind::Base { productions } => productions
                    .iter()
                    .any(|prod| self.production_is_nullable(prod, visiting)),
                RuleKind::Or { rules } => rules
                    .iter()
                    .any(|child| self.rule_is_nullable(child, visiting)),
                RuleKind::Forward { inner } => inner
                    .read()
                    .clone()
                    .map(|r| self.rule_is_nullable(&r, visiting))
                    .unwrap_or(false),
                _ => false,
            }
        };

        visiting.remove(&key);
        result
    }

    /// Проверяет структурные инварианты всех зарегистрированных правил.
    ///
    /// Валидация включает:
    /// - непустую форму правила;
    /// - отсутствие некорректных пустых продукций (каноническая ε разрешена, см. `EmptyProduction::matches_production`);
    /// - определенность forward-ссылок.
    pub fn validate(&self) -> Result<(), RuleValidationError> {
        self.validate_non_empty_rules()?;
        self.validate_non_empty_productions()?;
        // Graph-based rules use direct `Arc` references; there are no unresolved RuleId refs here.
        self.validate_rule_references()?;
        Ok(())
    }

    /// Проверяет, что у каждого правила есть хотя бы одна продукционная форма.
    fn validate_non_empty_rules(&self) -> Result<(), RuleValidationError> {
        for (id, rule) in &self.rules {
            let mut visiting = HashSet::new();
            if !self.rule_has_shape(rule, &mut visiting) {
                return Err(RuleValidationError::RuleHasNoProductions { rule_id: *id });
            }
        }
        Ok(())
    }

    /// Проверяет, что у `Base`-правил нет некорректных пустых продукций (ε см. [`validate_rule_non_empty_productions`]).
    fn validate_non_empty_productions(&self) -> Result<(), RuleValidationError> {
        for (id, rule) in &self.rules {
            self.validate_rule_non_empty_productions(id, rule, &mut HashSet::new())?;
        }
        Ok(())
    }

    /// Рекурсивно проверяет, что у `Base`-узлов нет некорректных пустых продукций.
    ///
    /// Пустой список термов допустим только для ε-продукции
    /// ([`EmptyProduction::matches_production`]) — так нормализуется `Empty` и ветка
    /// `optional` / `or(empty, …)`.
    fn validate_rule_non_empty_productions(
        &self,
        top_id: &RuleId,
        rule: &Arc<Rule<'a>>,
        visiting: &mut HashSet<usize>,
    ) -> Result<(), RuleValidationError> {
        let key = Self::rule_key(rule);
        if !visiting.insert(key) {
            return Ok(());
        }

        let result = if let Some(inner) = rule.kind.transparent_inner_rule() {
            self.validate_rule_non_empty_productions(top_id, inner, visiting)
        } else {
            match &rule.kind {
                RuleKind::Base { productions } => {
                    for (i, prod) in productions.iter().enumerate() {
                        if prod.terms.is_empty() && !EmptyProduction::matches_production(prod) {
                            return Err(RuleValidationError::EmptyProduction {
                                rule_id: *top_id,
                                production_idx: i,
                            });
                        }
                    }
                    Ok(())
                }
                RuleKind::Or { rules } => {
                    for child in rules {
                        self.validate_rule_non_empty_productions(top_id, child, visiting)?;
                    }
                    Ok(())
                }
                RuleKind::Forward { inner } => inner
                    .read()
                    .clone()
                    .map(|r| self.validate_rule_non_empty_productions(top_id, &r, visiting))
                    .unwrap_or_else(|| {
                        Err(RuleValidationError::UndefinedForward { rule_id: *top_id })
                    }),
                // `Empty` is a valid graph node; `add()` normalizes it anyway.
                _ => Ok(()),
            }
        };

        visiting.remove(&key);
        result
    }

    /// Заглушка валидации ссылок.
    ///
    /// Для graph-based представления с `Arc` явные id-ссылки не используются.
    fn validate_rule_references(&self) -> Result<(), RuleValidationError> {
        Ok(())
    }

    /// Проверяет, имеет ли узел правила структурно корректную непустую форму.
    fn rule_has_shape(&self, rule: &Arc<Rule<'a>>, visiting: &mut HashSet<usize>) -> bool {
        let key = Self::rule_key(rule);
        if !visiting.insert(key) {
            return true;
        }

        let result = if let Some(inner) = rule.kind.transparent_inner_rule() {
            self.rule_has_shape(inner, visiting)
        } else {
            match &rule.kind {
                RuleKind::Base { productions } => !productions.is_empty(),
                RuleKind::Or { rules } => !rules.is_empty(),
                RuleKind::Forward { inner } => inner
                    .read()
                    .clone()
                    .map(|r| self.rule_has_shape(&r, visiting))
                    .unwrap_or(false),
                _ => true,
            }
        };

        visiting.remove(&key);
        result
    }

    #[inline]
    /// Возвращает уникальный ключ узла правила на основе адреса `Arc`.
    fn rule_key(rule: &Arc<Rule<'a>>) -> usize {
        Arc::as_ptr(rule) as usize
    }

    #[inline]
    /// Выделяет следующий последовательный идентификатор правила.
    fn alloc_id(&mut self) -> RuleId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::RuleRegistry;
    use crate::error::RuleValidationError;
    use crate::predicates::constructors::eq;
    use crate::rule::builder::RuleId;
    use crate::rule::constructors::{Production, Rule, RuleKind, Term, TermOrMain};
    use crate::span::Span;
    use crate::token::{Token, TokenType};

    fn pred_rule<'a>(value: &'a str) -> Arc<Rule<'a>> {
        Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(eq(value))],
            None,
        )]))
    }

    fn token<'a>(value: &'a str) -> Token<'a> {
        Token::new(value, Span::new(0, value.len()), TokenType::Latin)
    }

    #[test]
    fn add_tracks_top_level_and_internal_rules() {
        let child = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(eq("x")), TermOrMain::from(eq("y"))],
            None,
        )]));
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(child.clone()), TermOrMain::from(eq("z"))],
            None,
        )]));

        let mut registry = RuleRegistry::new();
        let root_id = registry.add(root);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains(root_id));
        assert!(registry.next_id().0 >= 2);

        let stored_root = registry.get(root_id).expect("root rule must be present");
        let child_id = match &stored_root.kind {
            RuleKind::Base { productions } => match &productions[0].terms[0] {
                Term::Rule(r) => registry
                    .rule_id_for_arc(r)
                    .expect("child rule id must be assigned"),
                _ => panic!("expected nested rule term"),
            },
            _ => panic!("expected base root"),
        };
        assert!(registry.contains(child_id));
        assert_ne!(child_id, root_id);
    }

    #[test]
    fn predicted_production_indices_filter_by_lookahead() {
        let rule = Arc::new(Rule::new(vec![
            Production::new(vec![TermOrMain::from(eq("a"))], None),
            Production::new(vec![TermOrMain::from(eq("b"))], None),
        ]));

        let mut registry = RuleRegistry::new();
        let id = registry.add(rule);

        assert_eq!(registry.predicted_production_indices(id, None), vec![0, 1]);

        let tok_a = token("a");
        let tok_c = token("c");
        assert_eq!(
            registry.predicted_production_indices(id, Some(&tok_a)),
            vec![0]
        );
        assert_eq!(
            registry.predicted_production_indices(id, Some(&tok_c)),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn production_delegates_through_named_rule() {
        let named = pred_rule("x").named("X");
        let mut registry = RuleRegistry::new();
        let id = registry.add(named);

        let prod = registry
            .production(id, 0)
            .expect("production 0 must be reachable via named wrapper");
        assert_eq!(prod.terms.len(), 1);
        assert_eq!(prod.terms[0].label(), "eq(x)");
    }

    #[test]
    fn production_delegates_through_forward_rule() {
        let fwd = Rule::forward();
        Rule::define_forward(&fwd, pred_rule("x"));

        let mut registry = RuleRegistry::new();
        let id = registry.add(fwd);

        let prod = registry
            .production(id, 0)
            .expect("production 0 must be reachable via forward");
        assert_eq!(prod.terms.len(), 1);
        assert_eq!(prod.terms[0].label(), "eq(x)");
    }

    #[test]
    fn pipeline_delegates_productions_and_exposes_values() {
        let inner = pred_rule("anything");
        let pipeline = inner.pipeline("demo", vec!["KEY".to_string()]);

        let mut registry = RuleRegistry::new();
        let id = registry.add(pipeline);

        let tok = token("anything");
        assert_eq!(registry.predicted_production_indices(id, None), vec![0]);
        assert_eq!(
            registry.predicted_production_indices(id, Some(&tok)),
            vec![0]
        );
        let prod = registry
            .production(id, 0)
            .expect("pipeline must delegate to inner production");
        assert_eq!(prod.terms.len(), 1);
        assert_eq!(registry.pipeline_value(id, 0), Some("KEY"));
    }

    #[test]
    fn validate_passes_for_simple_registry() {
        let mut registry = RuleRegistry::new();
        registry.add(pred_rule("x"));
        assert_eq!(registry.validate(), Ok(()));
    }

    #[test]
    fn validate_fails_for_undefined_forward() {
        let mut registry = RuleRegistry::new();
        registry.add(Rule::forward());

        let err = registry
            .validate()
            .expect_err("undefined forward must fail validation");
        assert!(matches!(
            err,
            RuleValidationError::RuleHasNoProductions { .. }
        ));
    }

    #[test]
    fn validate_fails_for_empty_base_production_with_bad_main() {
        // Пустые `terms` при `main == 0` — каноническая ε ([`EmptyProduction`]); `main > 0` при пустых terms — ошибка.
        let bad = Arc::new(Rule::new(vec![Production {
            terms: vec![],
            main: 1,
        }]));
        let mut registry = RuleRegistry::new();
        let id = registry.add(bad);

        let err = registry
            .validate()
            .expect_err("empty production with invalid main must fail validation");
        assert_eq!(
            err,
            RuleValidationError::EmptyProduction {
                rule_id: id,
                production_idx: 0
            }
        );
    }

    #[test]
    fn validate_passes_when_normalized_graph_has_epsilon_productions() {
        use crate::rule::builder::term;

        let optional_x = term("x").optional().into_arc();
        let mut registry = RuleRegistry::new();
        registry.add(optional_x);
        assert_eq!(registry.validate(), Ok(()));
    }

    #[test]
    fn validate_fails_for_rule_without_productions_shape() {
        let bad = Arc::new(Rule::or(vec![]));
        let mut registry = RuleRegistry::new();
        let id = registry.add(bad);

        let err = registry
            .validate()
            .expect_err("rule with no productions should fail shape validation");
        assert_eq!(
            err,
            RuleValidationError::RuleHasNoProductions { rule_id: id }
        );
    }

    #[test]
    fn unknown_id_queries_are_safe() {
        let registry: RuleRegistry<'static> = RuleRegistry::new();
        let unknown = RuleId(999);

        assert!(!registry.contains(unknown));
        assert!(registry.get(unknown).is_none());
        assert!(registry.production(unknown, 0).is_none());
        assert_eq!(
            registry.predicted_production_indices(unknown, None),
            Vec::<usize>::new()
        );
    }
}
