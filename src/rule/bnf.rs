//! Преобразование графа правил в BNF-представление.
//!
//! Здесь используется **ориентированный граф нетерминалов**:
//! - вершина: [`BnfRule`](crate::rule::bnf::BnfRule);
//! - ребро: [`BnfTerm::Rule`](crate::rule::bnf::BnfTerm::Rule) внутри [`BnfProduction`](crate::rule::bnf::BnfProduction), указывающее на другую вершину.
//!
//! Граф может быть не деревом:
//! - допускаются общие подграфы (shared nodes),
//! - допускаются циклы (в том числе рекурсия через `Forward`).
//!
//! Основные алгоритмы:
//! - подсчет входящих ссылок (`count_parents`) для определения shared-узлов;
//! - рекурсивная трансформация с мемоизацией (`memo`);
//! - двухфазная сборка узлов (сначала создаем узел, потом заполняем),
//!   чтобы корректно поддерживать циклические ссылки;
//! - обход в ширину (`bnf_walk_bfs`) для стабильного порядка вывода;
//! - генерация имен `R0`, `R1`, ... для безымянных нетерминалов.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::predicates::constructors::PredicateKind;
use crate::rule::constructors::{Production, Rule, RuleKind, Term};

// Утилита для корректного экранирования терминальных строк в BNF-источнике.
fn quote_terminal(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

// Рекурсивная функция для получения читаемой метки предиката, учитывая его структуру.
fn format_predicate_label(predicate: &PredicateKind<'_>) -> String {
    match predicate {
        PredicateKind::Eq(eq) => quote_terminal(eq.value.as_ref()),
        PredicateKind::And(list) => {
            let inner = list
                .iter()
                .map(format_predicate_label)
                .collect::<Vec<_>>()
                .join(" & ");
            format!("and({inner})")
        }
        PredicateKind::Or(list) => {
            let inner = list
                .iter()
                .map(format_predicate_label)
                .collect::<Vec<_>>()
                .join(" | ");
            format!("or({inner})")
        }
        PredicateKind::Not(inner) => format!("not({})", format_predicate_label(inner)),
        _ => predicate.label(),
    }
}

/// Термин BNF-продукции.
#[derive(Debug, Clone)]
pub enum BnfTerm<'a> {
    /// Предикатный терминал.
    Pred(PredicateKind<'a>),
    /// Ссылка на другой нетерминал BNF-графа.
    Rule(Arc<BnfRule<'a>>),
}

impl<'a> BnfTerm<'a> {
    fn label(&self) -> String {
        match self {
            BnfTerm::Pred(p) => format_predicate_label(p),
            BnfTerm::Rule(r) => r.label(),
        }
    }
}

/// Одна продукция (правая часть) BNF-правила.
#[derive(Debug, Clone)]
pub struct BnfProduction<'a> {
    /// Последовательность терминов продукции.
    pub terms: Vec<BnfTerm<'a>>,
}

impl<'a> fmt::Display for BnfProduction<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.terms.is_empty() {
            return write!(f, "e");
        }
        let s = self
            .terms
            .iter()
            .map(|t| t.label())
            .collect::<Vec<_>>()
            .join(" ");
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone)]
struct BnfRuleData<'a> {
    productions: Vec<BnfProduction<'a>>,
    name: Option<String>,
    // future: interpretator/relation/pipeline labels
    // interpretator: Option<String>,
    // relation: Option<String>,
}

/// Узел графа BNF (нетерминал).
///
/// Данные хранятся под [`RwLock`], чтобы поддержать двухфазную сборку графа
/// при рекурсивных и forward-ссылках.
#[derive(Debug)]
pub struct BnfRule<'a> {
    data: RwLock<BnfRuleData<'a>>,
}

impl<'a> BnfRule<'a> {
    fn new_empty() -> Self {
        Self {
            data: RwLock::new(BnfRuleData {
                productions: Vec::new(),
                name: None,
            }),
        }
    }

    fn set_productions(&self, prods: Vec<BnfProduction<'a>>) {
        self.data.write().productions = prods;
    }

    fn productions(&self) -> Vec<BnfProduction<'a>> {
        self.data.read().productions.clone()
    }

    fn name(&self) -> Option<String> {
        self.data.read().name.clone()
    }

    fn set_name(&self, name: String) {
        self.data.write().name = Some(name);
    }

    fn label(&self) -> String {
        self.name().unwrap_or_else(|| "<unnamed>".into())
    }
}

impl<'a> fmt::Display for BnfRule<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.label();
        let prods = self.productions();
        let rhs = prods
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        write!(f, "{name} -> {rhs}")
    }
}

/// Итоговая BNF-грамматика как набор достижимых правил.
#[derive(Debug, Clone)]
pub struct Bnf<'a> {
    /// Список правил в порядке BFS от стартового узла.
    pub rules: Vec<Arc<BnfRule<'a>>>,
}

impl<'a> Bnf<'a> {
    /// Возвращает стартовое правило грамматики.
    pub fn start(&self) -> Arc<BnfRule<'a>> {
        self.rules[0].clone()
    }

    /// Возвращает человекочитаемый BNF-исходник построчно.
    pub fn source(&self) -> impl Iterator<Item = String> {
        self.rules
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Возвращает BNF как многострочную строку.
    pub fn as_string(&self) -> String {
        self.source().collect::<Vec<_>>().join("\n")
    }

    /// Печатает BNF в stdout.
    pub fn print(&self) {
        println!("{}", self.as_string());
    }
}

/// Обходит BNF-граф в ширину, возвращая уникальные вершины.
fn bnf_walk_bfs<'a>(root: Arc<BnfRule<'a>>) -> Vec<Arc<BnfRule<'a>>> {
    let mut out = Vec::new();
    let mut q = VecDeque::new();
    let mut visited: HashSet<usize> = HashSet::new();

    // Стартуем BFS от корня и сразу помечаем его посещенным, чтобы не зациклиться
    // при само-рекурсивных или взаимно-рекурсивных правилах.
    let root_id = Arc::as_ptr(&root) as usize;
    visited.insert(root_id);
    q.push_back(root);

    while let Some(node) = q.pop_front() {
        out.push(node.clone());

        // Ребра графа: только BnfTerm::Rule внутри каждой продукции.
        for prod in node.productions() {
            for t in prod.terms {
                if let BnfTerm::Rule(child) = t {
                    let id = Arc::as_ptr(&child) as usize;
                    // Каждый узел попадает в очередь максимум один раз.
                    if visited.insert(id) {
                        q.push_back(child);
                    }
                }
            }
        }
    }

    out
}

/// Назначает автоимена (`R0`, `R1`, ...) безымянным правилам.
fn generate_names<'a>(rules: &mut [Arc<BnfRule<'a>>]) {
    let mut count = 0usize;
    for r in rules.iter() {
        if r.name().is_none() {
            r.set_name(format!("R{count}"));
            count += 1;
        }
    }
}

/// Поднимает узел в дополнительный уровень нетерминала: `Lift -> Node`.
///
/// Используется для корректного представления shared/recursive оберток.
fn lift<'a>(item: Arc<BnfRule<'a>>) -> Arc<BnfRule<'a>> {
    let lifted = Arc::new(BnfRule::new_empty());
    lifted.set_productions(vec![BnfProduction {
        terms: vec![BnfTerm::Rule(item)],
    }]);
    lifted
}

/// Трансформатор `Rule`-графа в BNF-граф.
///
/// Архитектура:
/// - `parents`: количество входящих ссылок на `Rule`-узел;
/// - `memo`: соответствие `Rule* -> BnfRule` для мемоизации и сохранения идентичности узлов;
/// - `building`: множество узлов, которые сейчас строятся, для безопасной рекурсии.
pub struct BnfTransformator<'a> {
    /// parents[rule_ptr] = сколько раз на него сослались как на ребёнка
    parents: HashMap<usize, usize>,
    /// memo map: исходный Rule* -> BnfRule node
    memo: HashMap<usize, Arc<BnfRule<'a>>>,
    /// nodes currently being built (for recursive wrappers/forwards)
    building: HashSet<usize>,
}

impl<'a> BnfTransformator<'a> {
    /// Создает пустой трансформатор.
    pub fn new() -> Self {
        Self {
            parents: HashMap::new(),
            memo: HashMap::new(),
            building: HashSet::new(),
        }
    }

    /// Возвращает ключ узла `Rule` по адресу `Arc`.
    #[inline]
    fn key_rule(rule: &Arc<Rule<'a>>) -> usize {
        Arc::as_ptr(rule) as usize
    }

    /// Подсчитывает количество входящих ссылок на каждый узел исходного `Rule`-графа.
    ///
    /// Используется для решения, нужно ли "поднимать" shared-узлы через [`lift`].
    fn count_parents(&mut self, root: &Arc<Rule<'a>>) {
        // Считаем входящие ссылки на Rule-узлы.
        // Важно считать ссылки из productions terms.
        for node in root.walk_bfs() {
            match &node.kind {
                RuleKind::Base { productions } => {
                    for p in productions {
                        for t in &p.terms {
                            if let Term::Rule(child) = t {
                                let k = Self::key_rule(child);
                                *self.parents.entry(k).or_insert(0) += 1;
                            }
                        }
                    }
                }
                // для wrapper'ов тоже учитываем child, чтобы shared работал
                RuleKind::Optional { rule, .. }
                | RuleKind::Repeatable { rule, .. }
                | RuleKind::RepeatableOptional { rule, .. }
                | RuleKind::MinBounded { rule, .. }
                | RuleKind::MaxBounded { rule, .. }
                | RuleKind::MinMaxBounded { rule, .. }
                | RuleKind::Named { rule, .. }
                | RuleKind::Interpretation { rule, .. }
                | RuleKind::Relation { rule, .. }
                | RuleKind::Pipeline { rule, .. } => {
                    let k = Self::key_rule(rule);
                    *self.parents.entry(k).or_insert(0) += 1;
                }
                RuleKind::Or { rules } => {
                    // Учитываем каждую альтернативу как отдельную входящую ссылку.
                    for r in rules {
                        let k = Self::key_rule(r);
                        *self.parents.entry(k).or_insert(0) += 1;
                    }
                }
                RuleKind::Forward { inner } => {
                    // Для forward считаем ссылку на текущее определение inner (если задано).
                    if let Some(r) = inner.read().as_ref() {
                        let k = Self::key_rule(r);
                        *self.parents.entry(k).or_insert(0) += 1;
                    }
                }
                RuleKind::Empty => {}
            }
        }
    }

    /// Возвращает `true`, если узел имеет более одного родителя во входном графе.
    fn is_shared(&self, rule: &Arc<Rule<'a>>) -> bool {
        self.parents
            .get(&Self::key_rule(rule))
            .copied()
            .unwrap_or(0)
            > 1
    }

    /// Возвращает существующий или создает новый BNF-узел для исходного `Rule`-узла.
    fn get_or_create_node(&mut self, rule: &Arc<Rule<'a>>) -> Arc<BnfRule<'a>> {
        let k = Self::key_rule(rule);
        if let Some(n) = self.memo.get(&k) {
            // Повторно используем уже созданный BNF-узел для сохранения топологии графа.
            return n.clone();
        }
        // Узел создается "пустым"; продукции будут заполнены позже в visit_rule.
        let node = Arc::new(BnfRule::new_empty());
        self.memo.insert(k, node.clone());
        node
    }

    /// Преобразует термин исходного правила в BNF-термин.
    fn visit_term(&mut self, t: Term<'a>) -> BnfTerm<'a> {
        match t {
            Term::Pred(p) => BnfTerm::Pred(p),
            Term::Rule(r) => BnfTerm::Rule(self.visit_rule(r)),
        }
    }

    /// Преобразует продукцию исходного правила в BNF-продукцию.
    fn visit_production(&mut self, p: &Production<'a>) -> BnfProduction<'a> {
        BnfProduction {
            terms: p
                .terms
                .iter()
                .cloned()
                .map(|t| self.visit_term(t))
                .collect(),
        }
    }

    /// Обрабатывает wrapper-подобные узлы (`Named`, `Interpretation`, `Relation`).
    ///
    /// Для shared/recursive внутреннего узла применяет [`lift`], чтобы сохранить
    /// корректную форму BNF-графа и именование.
    fn visit_wrapper_like(&mut self, inner: Arc<Rule<'a>>) -> Arc<BnfRule<'a>> {
        // - forward: вернуть lift(forward) (там потом forward снимают)
        // У нас forward уже полноценный nonterminal, поэтому просто visit(forward).
        // - shared: если inner shared -> lift(visit(inner))
        // - иначе -> visit(inner)
        let shared = self.is_shared(&inner);
        let recursive = self.building.contains(&Self::key_rule(&inner));
        let node = self.visit_rule(inner);
        // Для shared/recursive узла добавляем промежуточный нетерминал (lift),
        // чтобы избежать "слипания" имен и неоднозначного повторного использования.
        if shared || recursive {
            lift(node)
        } else {
            node
        }
    }

    /// Рекурсивно преобразует `Rule`-узел в `BnfRule`-узел.
    ///
    /// Использует двухфазную сборку:
    /// 1. создать/получить узел из `memo`,
    /// 2. заполнить его продукции после обработки потомков.
    ///
    /// # Panics
    /// Паникует, если входной граф не нормализован или встречен undefined `Forward`.
    fn visit_rule(&mut self, rule: Arc<Rule<'a>>) -> Arc<BnfRule<'a>> {
        // Двухфазно: узел создаём сразу, затем заполняем productions,
        // чтобы рекурсивные ссылки работали.
        let key = Self::key_rule(&rule);
        let node = self.get_or_create_node(&rule);

        // Если уже заполнено — можно не пересобирать.
        // (но аккуратно: name может быть выставлен позже wrapper'ом)
        if !node.productions().is_empty() {
            // Узел уже полностью собран ранее (мемоизация).
            return node;
        }
        if self.building.contains(&key) {
            // Узел находится в стеке текущей рекурсии: возвращаем ранний placeholder.
            return node;
        }
        self.building.insert(key);

        match &rule.kind {
            RuleKind::Base { productions } => {
                let prods = productions
                    .iter()
                    .map(|p| self.visit_production(p))
                    .collect();
                node.set_productions(prods);
            }

            // В BNFTransformator python это запрещено: Or/Extended/Empty
            // (ожидается, что до этого был normalized()).
            RuleKind::Or { .. } => {
                panic!("BNFTransformator: Or must be replaced before BNF (call normalized())")
            }
            RuleKind::Optional { .. }
            | RuleKind::Repeatable { .. }
            | RuleKind::RepeatableOptional { .. }
            | RuleKind::MinBounded { .. }
            | RuleKind::MaxBounded { .. }
            | RuleKind::MinMaxBounded { .. } => {
                panic!("BNFTransformator: Extended rules must be replaced before BNF (call normalized())")
            }
            RuleKind::Empty => {
                panic!("BNFTransformator: Empty must be replaced before BNF (call normalized())")
            }

            RuleKind::Forward { inner } => {
                let inner_rule = inner
                    .read()
                    .clone()
                    .unwrap_or_else(|| panic!("BNFTransformator: forward not defined"));

                // Forward становится nonterminal'ом с productions как у inner.
                // Внутри inner могут быть ссылки на forward — они попадут на тот же node через memo.
                let inner_bnf = self.visit_rule(inner_rule);
                let prods = inner_bnf.productions();
                if prods.is_empty() {
                    // Защита на случай временно пустого inner: связываем через ссылочную продукцию.
                    node.set_productions(vec![BnfProduction {
                        terms: vec![BnfTerm::Rule(inner_bnf.clone())],
                    }]);
                } else {
                    node.set_productions(prods);
                }
                if node.name().is_none() {
                    if let Some(name) = inner_bnf.name() {
                        node.set_name(name);
                    }
                }
            }

            RuleKind::Named {
                rule: inner, name, ..
            } => {
                let mut out = self.visit_wrapper_like(inner.clone());

                // Если out уже имеет имя -> lift(out) (как python)
                if out.name().is_some() {
                    out = lift(out);
                }
                out.set_name(name.clone());

                // Важно: node должен представлять именно этот wrapper rule.
                // Поэтому "node" (созданный по адресу wrapper) делаем алиасом out:
                // проще всего — скопировать productions и name в node.
                node.set_productions(out.productions());
                if let Some(n) = out.name() {
                    node.set_name(n);
                }
            }

            RuleKind::Interpretation {
                rule: inner,
                interpretation: _,
            } => {
                let out = self.visit_wrapper_like(inner.clone());
                node.set_productions(out.productions());
            }

            RuleKind::Relation { rule: inner, .. } => {
                let out = self.visit_wrapper_like(inner.clone());
                node.set_productions(out.productions());
            }

            RuleKind::Pipeline {
                rule: inner,
                pipeline,
                ..
            } => {
                let out = self.visit_wrapper_like(inner.clone());
                // Pipeline-обертка остается видимой в BNF, но использует productions внутреннего правила.
                node.set_name(format!("pipeline({})", pipeline));
                node.set_productions(out.productions());
            }
        }

        // Выходим из "строящегося" множества только после полной сборки узла.
        self.building.remove(&key);

        node
    }

    /// Главная точка входа преобразования `Rule` -> `Bnf`.
    ///
    /// Этапы:
    /// 1. `count_parents`;
    /// 2. рекурсивная сборка BNF-графа от корня;
    /// 3. BFS-упорядочивание достижимых узлов;
    /// 4. генерация имен для безымянных правил.
    pub fn transform(&mut self, root: Arc<Rule<'a>>) -> Bnf<'a> {
        // В python BNFTransformator сначала считает parents, затем строит.
        self.count_parents(&root);

        // Строим BNF из root
        let root_bnf = self.visit_rule(root);

        // Собираем все BNFRule узлы в порядке BFS
        let mut rules = bnf_walk_bfs(root_bnf);

        // Генерируем имена для тех, у кого name None
        generate_names(&mut rules);

        // Итог содержит только узлы, достижимые из стартового правила.
        Bnf { rules }
    }
}

impl<'a> Default for BnfTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{BnfProduction, BnfTerm, BnfTransformator};
    use crate::predicates::constructors::eq;
    use crate::rule::constructors::{Production, Rule, TermOrMain};

    fn pred_rule<'a>(value: &'a str) -> Arc<Rule<'a>> {
        Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(eq(value))],
            None,
        )]))
    }

    #[test]
    fn bnf_production_display_for_empty_is_epsilon() {
        let p = BnfProduction { terms: vec![] };
        assert_eq!(p.to_string(), "e");
    }

    #[test]
    fn bnf_production_display_for_predicate_terms() {
        let p = BnfProduction {
            terms: vec![BnfTerm::Pred(eq("a")), BnfTerm::Pred(eq("b"))],
        };
        assert_eq!(p.to_string(), "'a' 'b'");
    }

    #[test]
    fn transform_base_rule_assigns_generated_name() {
        let root = pred_rule("x");
        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        let lines: Vec<String> = bnf.source().collect();

        assert_eq!(lines, vec!["R0 -> 'x'".to_string()]);
    }

    #[test]
    fn transform_named_rule_preserves_name() {
        let root = pred_rule("x").named("X");
        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        let lines: Vec<String> = bnf.source().collect();

        assert_eq!(lines, vec!["X -> 'x'".to_string()]);
    }

    #[test]
    fn transform_forward_rule_uses_defined_inner() {
        let fwd = Rule::forward();
        let target = pred_rule("x").named("X");
        Rule::define_forward(&fwd, target);

        let mut t = BnfTransformator::new();
        let bnf = t.transform(fwd);
        let lines: Vec<String> = bnf.source().collect();

        assert_eq!(lines, vec!["X -> 'x'".to_string()]);
    }

    #[test]
    fn transform_rule_graph_keeps_bfs_source_order() {
        let child = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(child)],
            None,
        )]));

        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        let lines: Vec<String> = bnf.source().collect();

        assert_eq!(lines, vec!["R0 -> R1".to_string(), "R1 -> 'x'".to_string()]);
    }

    #[test]
    fn bnf_start_returns_first_rule() {
        let child = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(child)],
            None,
        )]));

        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        assert!(Arc::ptr_eq(&bnf.start(), &bnf.rules[0]));
    }

    #[test]
    fn bnf_as_string_joins_lines_with_newline() {
        let child = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(child)],
            None,
        )]));

        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        assert_eq!(bnf.as_string(), "R0 -> R1\nR1 -> 'x'");
    }

    #[test]
    fn transform_pipeline_rule_preserves_wrapper_name_and_inner_productions() {
        let root = pred_rule("x").pipeline("demo", vec!["KEY".to_string()]);

        let mut t = BnfTransformator::new();
        let bnf = t.transform(root);
        let lines: Vec<String> = bnf.source().collect();

        assert_eq!(lines, vec!["pipeline(demo) -> 'x'".to_string()]);
    }

    #[test]
    #[should_panic(expected = "BNFTransformator: forward not defined")]
    fn transform_undefined_forward_panics() {
        let mut t = BnfTransformator::new();
        let _ = t.transform(Rule::forward());
    }

    #[test]
    #[should_panic(expected = "BNFTransformator: Or must be replaced before BNF")]
    fn transform_or_rule_without_normalization_panics() {
        let root = Arc::new(Rule::or(vec![pred_rule("a"), pred_rule("b")]));
        let mut t = BnfTransformator::new();
        let _ = t.transform(root);
    }

    #[test]
    #[should_panic(expected = "BNFTransformator: Extended rules must be replaced before BNF")]
    fn transform_extended_rule_without_normalization_panics() {
        let root = pred_rule("x").optional(false);
        let mut t = BnfTransformator::new();
        let _ = t.transform(root);
    }
}
