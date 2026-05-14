//! Реализует сборщик правил с удобными комбинаторами для создания сложных грамматик.
//!
//! Этот модуль предоставляет [`RuleBuilder`](crate::rule::builder::RuleBuilder) — фасад для создания
//! правил с помощью цепочки методов.
//! Он поддерживает создание терминальных правил, предикатов, опциональных и повторяющихся правил,
//! а также объединение правил через конкатенацию и дизъюнкцию.
//! Также определён [`RuleId`](crate::rule::builder::RuleId) — тип для идентификации правил в грамматике.

use std::sync::Arc;

use crate::internal::FactScheme;
use crate::predicates::constructors::{eq, PredicateKind};
use crate::rule::constructors::{Production, Rule, RuleKind, TermOrMain};

/// Уникальный идентификатор правила в грамматике.
///
/// Это newtype-обёртка над `usize` для повышения типобезопасности и читаемости.
/// Позволяет легко отличать ID правил от обычных чисел.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(pub usize);

impl RuleId {
    /// Создаёт новый `RuleId` из `usize`.
    pub fn new(value: usize) -> Self {
        Self(value)
    }
}

impl std::ops::AddAssign<usize> for RuleId {
    /// Добавляет `usize` к значению `RuleId`.
    ///
    /// Полезно для относительных сдвигов ID в некоторых алгоритмах.
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

/// Фасад-строитель вокруг [`Rule`] с цепочечными комбинаторами.
#[derive(Debug, Clone)]
pub struct RuleBuilder<'a> {
    inner: Arc<Rule<'a>>,
    /// Маркер для `main()`: помечает терм как главный в группе согласования.
    pub(crate) is_main: bool,
}

impl<'a> Default for RuleBuilder<'a> {
    fn default() -> Self {
        Self::epsilon()
    }
}

impl<'a> RuleBuilder<'a> {
    /// Создаёт builder из готового указателя на правило.
    pub fn from_arc(inner: Arc<Rule<'a>>) -> Self {
        Self {
            inner,
            is_main: false,
        }
    }

    /// Возвращает внутренний указатель на правило.
    pub fn into_arc(self) -> Arc<Rule<'a>> {
        self.inner
    }

    /// Создаёт терминальное правило, эквивалентное `eq(s)`.
    pub fn term(s: &'a str) -> Self {
        Self::pred(eq(s))
    }

    /// Создаёт правило из одной продукции по предикату.
    pub fn pred(predicate: PredicateKind<'a>) -> Self {
        let production = Production::new(vec![TermOrMain::from(predicate)], None);
        Self::from_arc(Arc::new(Rule::new(vec![production])))
    }

    /// Создаёт epsilon-правило.
    pub fn epsilon() -> Self {
        Self::from_arc(Arc::new(Rule::empty()))
    }

    /// Создаёт forward-правило (объявление с отложенным определением).
    pub fn forward() -> Self {
        Self::from_arc(Rule::forward())
    }

    /// Задаёт целевое правило для forward-правила.
    ///
    /// # Panics
    /// Паникует, если `self` не содержит forward-правило.
    pub fn define(&self, rule: RuleBuilder<'a>) {
        Rule::define_forward(&self.inner, rule.inner);
    }

    /// Оборачивает правило в `optional` с порядком альтернатив по умолчанию.
    pub fn optional(self) -> Self {
        Self::from_arc(self.inner.optional(false))
    }

    /// Оборачивает правило в `optional` с явным управлением порядком альтернатив.
    pub fn optional_with(self, reverse: bool) -> Self {
        Self::from_arc(self.inner.optional(reverse))
    }

    /// Оборачивает правило в безграничный вариант `repeatable`.
    pub fn repeatable(self) -> Self {
        Self::from_arc(self.inner.repeatable(None, None, false))
    }

    /// Оборачивает правило в bounded/unbounded вариант `repeatable`.
    ///
    /// # Panics
    /// Паникует при некорректных границах (например, `min == 0`, `max == 0`, `max < min`).
    pub fn repeatable_with(self, min: Option<usize>, max: Option<usize>, reverse: bool) -> Self {
        Self::from_arc(self.inner.repeatable(min, max, reverse))
    }

    /// Оборачивает правило в именованную оболочку.
    pub fn named(self, name: impl Into<String>) -> Self {
        Self::from_arc(self.inner.named(name))
    }

    /// Оборачивает правило в факт с полной схемой.
    pub fn named_fact(self, scheme: FactScheme) -> Self {
        Self::from_arc(self.inner.named_fact(scheme))
    }

    /// Возвращает нормализованный граф правила.
    pub fn normalized(self) -> Arc<Rule<'a>> {
        self.inner.normalized()
    }

    /// Возвращает многострочную BNF-строку для текущего правила.
    ///
    /// Граф должен быть нормализован ([`Self::normalized`]): сырой [`RuleKind::Or`]
    /// от [`or_`] и расширенные узлы (`optional`, …) до BNF не допускаются.
    pub fn as_bnf(&self) -> String {
        self.inner.as_bnf()
    }

    /// Возвращает собранное правило.
    ///
    /// Аргумент `_id` сохранён для обратной совместимости.
    pub fn build<I>(self, _id: I) -> Arc<Rule<'a>> {
        self.inner
    }

    /// Конкатенирует два базовых правила через декартово произведение продукций.
    fn concat_base(lhs: &Arc<Rule<'a>>, rhs: &Arc<Rule<'a>>) -> Option<Arc<Rule<'a>>> {
        let (RuleKind::Base { productions: lp }, RuleKind::Base { productions: rp }) =
            (&lhs.kind, &rhs.kind)
        else {
            return None;
        };

        let mut out = Vec::with_capacity(lp.len().saturating_mul(rp.len()));
        for l in lp {
            for r in rp {
                let mut terms = l.terms.clone();
                terms.extend(r.terms.clone());
                out.push(Production { terms, main: 0 });
            }
        }
        Some(Arc::new(Rule::new(out)))
    }
}

impl<'a> std::ops::Add for RuleBuilder<'a> {
    type Output = Self;

    /// Конкатенирует два правила (`AND` в стиле EBNF).
    fn add(self, rhs: Self) -> Self::Output {
        if let Some(rule) = Self::concat_base(&self.inner, &rhs.inner) {
            return Self::from_arc(rule);
        }

        let production = Production::new(
            vec![TermOrMain::from(self.inner), TermOrMain::from(rhs.inner)],
            None,
        );
        Self::from_arc(Arc::new(Rule::new(vec![production])))
    }
}

impl<'a> std::ops::BitOr for RuleBuilder<'a> {
    type Output = Self;

    /// Создаёт дизъюнкцию двух правил (`OR` в стиле EBNF).
    fn bitor(self, rhs: Self) -> Self::Output {
        match (&self.inner.kind, &rhs.inner.kind) {
            (RuleKind::Base { productions: lp }, RuleKind::Base { productions: rp }) => {
                let mut productions = lp.clone();
                productions.extend(rp.clone());
                Self::from_arc(Arc::new(Rule::new(productions)))
            }
            _ => Self::from_arc(Arc::new(Rule::or(vec![self.inner, rhs.inner]))),
        }
    }
}

impl<'a> From<RuleBuilder<'a>> for Arc<Rule<'a>> {
    fn from(value: RuleBuilder<'a>) -> Self {
        value.into_arc()
    }
}

/// Короткий алиас для [`RuleBuilder::term`].
pub fn term<'a>(s: &'a str) -> RuleBuilder<'a> {
    RuleBuilder::term(s)
}

/// Короткий алиас для [`RuleBuilder::pred`].
pub fn pred<'a>(predicate: PredicateKind<'a>) -> RuleBuilder<'a> {
    RuleBuilder::pred(predicate)
}

/// Короткий алиас для [`RuleBuilder::epsilon`].
pub fn eps<'a>() -> RuleBuilder<'a> {
    RuleBuilder::epsilon()
}

/// Короткий алиас для [`RuleBuilder::forward`].
pub fn forward<'a>() -> RuleBuilder<'a> {
    RuleBuilder::forward()
}

/// Помечает терм как главный в группе согласования.
pub fn main_term<'a>(b: RuleBuilder<'a>) -> RuleBuilder<'a> {
    RuleBuilder {
        inner: b.inner,
        is_main: true,
    }
}

/// Конкатенирует все элементы (`AND`) слева направо.
///
/// Возвращает epsilon, если `items` пуст.
/// Если один из элементов помечен через [`main_term`], его индекс будет записан
/// как `Production::main`.
pub fn rule<'a>(items: impl Into<Vec<RuleBuilder<'a>>>) -> RuleBuilder<'a> {
    let items: Vec<RuleBuilder<'a>> = items.into();
    if items.is_empty() {
        return RuleBuilder::epsilon();
    }

    let has_main = items.iter().any(|b| b.is_main);
    if !has_main {
        let mut it = items.into_iter();
        let first = it.next().unwrap();
        return it.fold(first, |acc, item| acc + item);
    }

    use crate::rule::constructors::Main;

    let term_items: Vec<TermOrMain<'a>> = items
        .into_iter()
        .map(|b| {
            if b.is_main {
                TermOrMain::from(Main::new(b.inner))
            } else {
                TermOrMain::from(b.inner)
            }
        })
        .collect();

    let production = Production::new(term_items, None);
    RuleBuilder::from_arc(Arc::new(Rule::new(vec![production])))
}

/// Строит дизъюнкцию (`OR`) для всех переданных элементов.
///
/// Возвращает epsilon, если `items` пуст; один элемент — без обёртки.
///
/// Для двух и более элементов создаётся **плоский** [`RuleKind::Or`] со всеми
/// альтернативами сразу,
/// без вложенных бинарных `Or` и без O(N²) слияния продукций `Base`.
pub fn or_<'a>(items: impl Into<Vec<RuleBuilder<'a>>>) -> RuleBuilder<'a> {
    let items: Vec<RuleBuilder<'a>> = items.into();
    match items.len() {
        0 => RuleBuilder::epsilon(),
        1 => items.into_iter().next().expect("len checked"),
        _ => {
            let rules: Vec<Arc<Rule<'a>>> = items.into_iter().map(RuleBuilder::into_arc).collect();
            RuleBuilder::from_arc(Arc::new(Rule::or(rules)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{eps, forward, or_, pred, rule, term, RuleBuilder, RuleId};
    use crate::predicates::constructors::eq;
    use crate::rule::constructors::{Rule, RuleKind, Term};

    fn one_term_base<'a>(builder: RuleBuilder<'a>) -> bool {
        let arc = builder.into_arc();
        match &arc.kind {
            RuleKind::Base { productions } => {
                productions.len() == 1 && productions[0].terms.len() == 1
            }
            _ => false,
        }
    }

    #[test]
    fn rule_id_new_and_add_assign_work() {
        let mut id = RuleId::new(10);
        id += 2;
        assert_eq!(id, RuleId(12));
    }

    #[test]
    fn constructors_term_pred_eps_forward_create_expected_kinds() {
        assert!(one_term_base(term("a")));
        assert!(one_term_base(pred(eq("a"))));
        assert!(matches!(eps().into_arc().kind, RuleKind::Empty));
        assert!(matches!(
            forward().into_arc().kind,
            RuleKind::Forward { .. }
        ));
    }

    #[test]
    fn define_sets_forward_target() {
        let fwd = forward();
        let target = term("x");
        let fwd_arc = fwd.clone().into_arc();
        fwd.define(target);

        match &fwd_arc.kind {
            RuleKind::Forward { inner } => {
                let mapped = inner.read().clone().expect("forward must be defined");
                assert!(matches!(mapped.kind, RuleKind::Base { .. }));
            }
            _ => panic!("expected forward rule"),
        }
    }

    #[test]
    fn optional_and_repeatable_build_expected_wrappers() {
        let base = term("x");
        assert!(matches!(
            base.clone().optional().into_arc().kind,
            RuleKind::Optional { reverse: false, .. }
        ));
        assert!(matches!(
            base.clone().optional_with(true).into_arc().kind,
            RuleKind::Optional { reverse: true, .. }
        ));
        assert!(matches!(
            base.clone().repeatable().into_arc().kind,
            RuleKind::Repeatable { reverse: false, .. }
        ));
        assert!(matches!(
            base.clone()
                .repeatable_with(Some(2), None, true)
                .into_arc()
                .kind,
            RuleKind::MinBounded {
                min: 2,
                reverse: true,
                ..
            }
        ));
        assert!(matches!(
            base.clone()
                .repeatable_with(None, Some(3), false)
                .into_arc()
                .kind,
            RuleKind::MaxBounded {
                max: 3,
                reverse: false,
                ..
            }
        ));
        assert!(matches!(
            base.repeatable_with(Some(2), Some(4), false)
                .into_arc()
                .kind,
            RuleKind::MinMaxBounded {
                min: 2,
                max: 4,
                reverse: false,
                ..
            }
        ));
    }

    #[test]
    fn named_and_build_work() {
        let b = term("x").named("X");
        assert!(matches!(b.clone().into_arc().kind, RuleKind::Named { .. }));

        let built = b.clone().build(());
        let built2 = b.build(RuleId(1));
        assert!(matches!(built.kind, RuleKind::Named { .. }));
        assert!(matches!(built2.kind, RuleKind::Named { .. }));
    }

    #[test]
    fn as_bnf_is_available_on_builder() {
        let size = rule(vec![
            or_(vec![term("р"), term("размер")]).named("KEY"),
            or_(vec![term("S"), term("M"), term("L")]).named("VALUE"),
        ])
        .named("SIZE");

        // `as_bnf` требует нормализованный граф (см. `Rule::bnf`); `or_` даёт сырой `Or`.
        let bnf = size.normalized().as_bnf();
        assert!(bnf.contains("SIZE -> KEY VALUE"));
        assert!(bnf.contains("KEY ->"));
        assert!(bnf.contains("VALUE ->"));
    }

    #[test]
    fn add_concatenates_base_rules() {
        let built = (term("a") + term("b")).into_arc();
        match &built.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert_eq!(productions[0].terms.len(), 2);
            }
            _ => panic!("expected Base from base+base"),
        }
    }

    #[test]
    fn add_falls_back_to_rule_terms_when_not_both_base() {
        let built = (forward() + term("b")).into_arc();
        match &built.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert_eq!(productions[0].terms.len(), 2);
                assert!(matches!(productions[0].terms[0], Term::Rule(_)));
                assert!(matches!(productions[0].terms[1], Term::Rule(_)));
            }
            _ => panic!("expected Base fallback from non-base + base"),
        }
    }

    #[test]
    fn bitor_merges_base_productions_or_builds_or_node() {
        let base_or = (term("a") | term("b")).into_arc();
        match &base_or.kind {
            RuleKind::Base { productions } => assert_eq!(productions.len(), 2),
            _ => panic!("expected Base from base|base"),
        }

        let non_base_or = (forward() | term("b")).into_arc();
        match &non_base_or.kind {
            RuleKind::Or { rules } => assert_eq!(rules.len(), 2),
            _ => panic!("expected Or from non-base|base"),
        }
    }

    #[test]
    fn helper_rule_and_or_handle_empty_and_non_empty_inputs() {
        let empty: Vec<RuleBuilder<'static>> = vec![];
        assert!(matches!(
            rule(empty.clone()).into_arc().kind,
            RuleKind::Empty
        ));
        assert!(matches!(or_(empty).into_arc().kind, RuleKind::Empty));

        let concatenated = rule(vec![term("a"), term("b")]).into_arc();
        match &concatenated.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert_eq!(productions[0].terms.len(), 2);
            }
            _ => panic!("expected Base from rule([...])"),
        }

        let disjunction = or_(vec![term("a"), term("b")]).into_arc();
        match &disjunction.kind {
            RuleKind::Or { rules } => assert_eq!(rules.len(), 2),
            _ => panic!("expected Or from or_([...])"),
        }
    }

    /// `or_` — плоский `Or`; после нормализации — один `Base` с N продукциями.
    #[test]
    fn or_builds_flat_or_and_normalizes_to_single_base() {
        let mixed = or_([term("a"), term("b").named("B"), term("c")]).into_arc();
        match &mixed.kind {
            RuleKind::Or { rules } => assert_eq!(rules.len(), 3),
            _ => panic!("expected flat Or with 3 branches"),
        }

        let norm = mixed.normalized();
        match &norm.kind {
            RuleKind::Base { productions } => assert_eq!(productions.len(), 3),
            _ => panic!("expected single Base with 3 productions after normalize"),
        }
    }

    #[test]
    fn into_arc_and_from_arc_roundtrip() {
        let arc = term("x").into_arc();
        let builder = RuleBuilder::from_arc(arc.clone());
        let arc2: Arc<Rule<'_>> = builder.into();
        assert!(Arc::ptr_eq(&arc, &arc2));
    }
}
