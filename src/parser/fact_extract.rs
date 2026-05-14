//! Извлечение фактов из дерева разбора (extraction pipeline).
//!
//! Этот модуль содержит внутреннюю машинерию для рекурсивного обхода
//! дерева разбора и материализации [`FactRecord`] на основе
//! `interpretation`-аннотаций в правилах.
//!
//! Общий конвейер:
//! `Tree -> find_fact_descriptor -> extract_from_node -> PendingFact -> FactRecord`.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::internal::{FactRecordRaw, FactScheme};
use crate::interpretation::{FactValue, Interpretation};
use crate::morph::morph::parse_opencorpora_grammemes;
use crate::rule::constructors::{Rule, RuleKind, RuleTransform as Transform, Term};
use crate::rule::registry::RuleRegistry;
use crate::span::Span;
use crate::token::{
    join_any_inflected_tokens, join_any_normalized_tokens, join_any_tokens, join_inflected_tokens,
    join_normalized_tokens, join_tokens, AnyToken, AnyTokenOwnedLite, Token,
};
use crate::tree::{Leaf, Node, ParseChild, Tree};

use super::match_result::ExtractedFact;

// ---------------------------------------------------------------------------
// PendingFact — mutable-аккумулятор
// ---------------------------------------------------------------------------

/// Внутренний mutable-аккумулятор факта на этапе извлечения из дерева разбора.
///
/// Этот тип не является частью API библиотеки и нужен только как рабочая
/// структура parser-side extraction:
/// в него постепенно складываются значения полей, после чего он один раз
/// материализуется в [`FactRecord`].
///
/// `PendingFact` хранится на `BTreeMap`, чтобы:
/// - иметь стабильный порядок полей при преобразовании в `FactValue::Object`
///   и при построении временной схемы.
#[derive(Debug, Clone, Default, PartialEq)]
struct PendingFact {
    /// Имя факта, которому соответствует текущий черновик.
    name: String,
    /// Уже извлечённые поля факта в стабильном порядке.
    fields: BTreeMap<String, FactValue>,
}

impl PendingFact {
    /// Создаёт пустой черновик факта для текущего parser-side pipeline.
    #[inline]
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Записывает или перезаписывает значение поля в черновике факта.
    #[inline]
    fn set(&mut self, field: impl Into<String>, value: FactValue) {
        self.fields.insert(field.into(), value);
    }

    /// Возвращает текущее значение поля, если оно уже было извлечено.
    #[inline]
    fn get(&self, field: &str) -> Option<&FactValue> {
        self.fields.get(field)
    }

    /// Даёт доступ к накопленным полям для финальной материализации.
    #[inline]
    fn fields(&self) -> &BTreeMap<String, FactValue> {
        &self.fields
    }

    /// Даёт mutable-доступ к карте полей для merge/repeatable-логики.
    #[inline]
    fn fields_mut(&mut self) -> &mut BTreeMap<String, FactValue> {
        &mut self.fields
    }
}

// ---------------------------------------------------------------------------
// NodeTextValue — текстовые значения узла
// ---------------------------------------------------------------------------

/// Временная структура для предоставления текстовых значений узлам при извлечении фактов.
#[derive(Debug, Clone)]
pub(super) struct NodeTextValue<'a> {
    tokens: Vec<Token<'a>>,
    any_tokens: Option<Vec<AnyToken<'a>>>,
    pipeline_key: Option<String>,
}

impl<'a> NodeTextValue<'a> {
    pub(super) fn new(
        tokens: Vec<Token<'a>>,
        any_tokens: Option<Vec<AnyToken<'a>>>,
        pipeline_key: Option<String>,
    ) -> Self {
        Self {
            tokens,
            any_tokens,
            pipeline_key,
        }
    }

    /// Возвращает исходный текст, покрытый токенами узла, через простое объединение.
    fn raw_text(&self) -> String {
        if let Some(tokens) = &self.any_tokens {
            join_any_tokens(tokens.iter())
        } else {
            join_tokens(self.tokens.iter())
        }
    }

    /// Возвращает нормализованный текст узла, объединяя нормализованные формы токенов.
    fn normalized_text(&self) -> String {
        if let Some(key) = &self.pipeline_key {
            return key.clone();
        }

        if let Some(tokens) = &self.any_tokens {
            join_any_normalized_tokens(tokens.iter())
        } else {
            join_normalized_tokens(self.tokens.iter())
        }
    }

    /// Возвращает inflected-форму текста узла, объединяя inflected-формы токенов.
    ///
    /// Когда доступны `any_tokens` (с суженными формами от предиката),
    /// используется `join_any_inflected_tokens`, которая опирается на `form.inflect()`
    /// из первой формы каждого `AnyToken::Morph` — а не на повторный `parse` слова.
    fn inflected_text(&self, grams: Option<&[String]>) -> String {
        if let Some(any_tokens) = &self.any_tokens {
            let mt = crate::token::global_morph_tokenizer();
            let analyzer = match &mt.morph {
                crate::token::Analyser::MorphAnalyzer(inner) => &inner.analyzer,
                crate::token::Analyser::CachedMorphAnalyzer(inner) => inner.analyzer(),
            };
            let parsed_grams = parse_opencorpora_grammemes(grams);
            join_any_inflected_tokens(any_tokens.iter(), analyzer, parsed_grams)
        } else {
            join_inflected_tokens(self.tokens.iter(), grams.map(|forms| forms.to_vec()))
        }
    }
}

// ---------------------------------------------------------------------------
// Leaf / token helpers
// ---------------------------------------------------------------------------

pub(super) fn collect_node_leaves(node: &Node, out: &mut Vec<Arc<Leaf>>) {
    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => out.push(leaf.clone()),
            ParseChild::Node(child_node) => collect_node_leaves(child_node, out),
        }
    }
}

pub(super) fn node_leaf_tokens_from_plain<'a>(node: &Node, tokens: &[Token<'a>]) -> Vec<Token<'a>> {
    let mut leaves = Vec::new();
    collect_node_leaves(node, &mut leaves);
    leaves
        .into_iter()
        .filter_map(|leaf| tokens.get(leaf.token_index).cloned())
        .collect()
}

pub(super) fn node_leaf_tokens_from_owned<'a>(
    node: &Node,
    tokens: &[AnyTokenOwnedLite],
    text: &'a str,
) -> Vec<Token<'a>> {
    let mut leaves = Vec::new();
    collect_node_leaves(node, &mut leaves);
    leaves
        .into_iter()
        .filter_map(|leaf| {
            let token = tokens.get(leaf.token_index)?;
            let span = token.span();
            let value = &text[span.start..span.stop];
            Some(Token {
                value: std::borrow::Cow::Borrowed(value),
                span,
                token_type: token.token_type(),
            })
        })
        .collect()
}

pub(super) fn node_leaf_any_tokens_from_owned<'a>(
    node: &Node,
    tokens: &[AnyTokenOwnedLite],
    text: &'a str,
) -> Vec<AnyToken<'a>> {
    let mut leaves = Vec::new();
    collect_node_leaves(node, &mut leaves);
    leaves
        .into_iter()
        .filter_map(|leaf| {
            let token = tokens.get(leaf.token_index)?;
            let span = token.span();
            let value = std::borrow::Cow::Borrowed(&text[span.start..span.stop]);
            let plain = Token {
                value,
                span,
                token_type: token.token_type(),
            };

            Some(match token {
                AnyTokenOwnedLite::Plain { .. } => AnyToken::Plain(plain),
                AnyTokenOwnedLite::Morph { forms, .. } => {
                    let effective_forms = if let Some(narrowed) = &leaf.matched_forms {
                        let surface = plain.value.to_lowercase();
                        let mut sorted = narrowed.to_vec();
                        sorted.sort_by_key(|f| f.normalized == surface);
                        sorted
                    } else {
                        forms.clone()
                    };
                    AnyToken::Morph(plain.morphed(effective_forms))
                }
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Span helpers
// ---------------------------------------------------------------------------

fn normalize_spans(mut spans: Vec<Span>) -> Vec<Span> {
    if spans.len() > 1 {
        spans.sort_by_key(|span| span.start);
        spans.dedup();
    }
    spans
}

fn node_span(node: &Node) -> Span {
    let mut spans = Vec::new();
    collect_node_spans(node, &mut spans);
    if spans.is_empty() {
        Span::new(0, 0)
    } else {
        let mut start = spans[0].start;
        let mut stop = spans[0].stop;
        for span in spans.iter().skip(1) {
            start = start.min(span.start);
            stop = stop.max(span.stop);
        }
        Span::new(start, stop)
    }
}

fn collect_node_spans(node: &Node, out: &mut Vec<Span>) {
    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => out.push(leaf.span),
            ParseChild::Node(child_node) => collect_node_spans(child_node, out),
        }
    }
}

// ---------------------------------------------------------------------------
// FactDescriptor
// ---------------------------------------------------------------------------

/// Описание факта.
#[derive(Debug, Clone)]
struct FactDescriptor {
    name: String,
    scheme: Option<Arc<FactScheme>>,
}

/// Возвращает схему факта для узла, если у соответствующего правила она задана.
fn find_fact_descriptor<'r>(registry: &RuleRegistry<'r>, node: &Node) -> Option<FactDescriptor> {
    let rule = registry.get(node.rule_id)?;
    find_rule_fact_descriptor(rule)
}

fn find_rule_fact_descriptor<'r>(rule: &Arc<Rule<'r>>) -> Option<FactDescriptor> {
    match &rule.kind {
        RuleKind::Named {
            name, fact_scheme, ..
        } => Some(FactDescriptor {
            name: name.clone(),
            scheme: fact_scheme.clone(),
        }),
        RuleKind::Interpretation { rule, .. }
        | RuleKind::Relation { rule, .. }
        | RuleKind::Optional { rule, .. }
        | RuleKind::Repeatable { rule, .. }
        | RuleKind::RepeatableOptional { rule, .. }
        | RuleKind::MinBounded { rule, .. }
        | RuleKind::MaxBounded { rule, .. }
        | RuleKind::MinMaxBounded { rule, .. } => find_rule_fact_descriptor(rule),
        RuleKind::Forward { inner } => inner
            .read()
            .clone()
            .and_then(|inner_rule| find_rule_fact_descriptor(&inner_rule)),
        RuleKind::Base { .. }
        | RuleKind::Or { .. }
        | RuleKind::Empty
        | RuleKind::Pipeline { .. } => None,
    }
}

/// Дескриптор факта у первого прямого дочернего `Node`, если у корня его нет.
///
/// Нужен для корня вида `Or` → нормализованного в `Base` с одной альтернативой на продукцию:
/// факт объявлен на выбранной ветке, а не на обёртке-диспетчере.
fn find_child_fact_descriptor<'r>(
    registry: &RuleRegistry<'r>,
    node: &Node,
) -> Option<FactDescriptor> {
    node.children.iter().find_map(|child| {
        if let ParseChild::Node(child_node) = child {
            find_fact_descriptor(registry, child_node)
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Interpretation steps
// ---------------------------------------------------------------------------

enum RulePipelineState<'a> {
    PendingFact(PendingFact),
    Value(FactValue, Option<NodeTextValue<'a>>),
}

enum RuleInterpretationStep {
    Attribute(Interpretation),
    Fact(String),
}

fn collect_rule_interpretation_steps<'r>(
    rule: &Arc<Rule<'r>>,
    out: &mut Vec<RuleInterpretationStep>,
) {
    match &rule.kind {
        RuleKind::Named { rule, name, .. } => {
            collect_rule_interpretation_steps(rule, out);
            out.push(RuleInterpretationStep::Fact(name.clone()));
        }
        RuleKind::Interpretation {
            rule,
            interpretation,
        } => {
            collect_rule_interpretation_steps(rule, out);
            if matches!(out.last(), Some(RuleInterpretationStep::Attribute(_))) {
                let _ = out.pop();
            }
            out.push(RuleInterpretationStep::Attribute(interpretation.clone()));
        }
        RuleKind::Relation { rule, .. }
        | RuleKind::Optional { rule, .. }
        | RuleKind::Repeatable { rule, .. }
        | RuleKind::RepeatableOptional { rule, .. }
        | RuleKind::MinBounded { rule, .. }
        | RuleKind::MaxBounded { rule, .. }
        | RuleKind::MinMaxBounded { rule, .. } => collect_rule_interpretation_steps(rule, out),
        RuleKind::Forward { inner } => {
            if let Some(inner_rule) = inner.read().clone() {
                collect_rule_interpretation_steps(&inner_rule, out);
            }
        }
        RuleKind::Base { .. }
        | RuleKind::Or { .. }
        | RuleKind::Empty
        | RuleKind::Pipeline { .. } => {}
    }
}

fn node_interpretation_steps<'r>(
    registry: &RuleRegistry<'r>,
    node: &Node,
) -> Vec<RuleInterpretationStep> {
    let Some(rule) = registry.get(node.rule_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_rule_interpretation_steps(rule, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Public extraction API
// ---------------------------------------------------------------------------

/// Извлекает один факт из дерева.
///
/// Использует `fact_name` корневого правила (или, если у корня нет факта, первого дочернего
/// узла с дескриптором — типично ветка `or_` после нормализации в `Base`) и рекурсивно
/// обходит дерево, применяя `interpretation` терминалов к полям факта.
pub(super) fn extract_fact_from_tree<'r, 'a, P>(
    registry: &RuleRegistry<'r>,
    tree: &Tree,
    provider: &P,
) -> Option<ExtractedFact>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    ExtractionContext::new(registry, provider).extract_fact_from_root(tree)
}

/// Извлекает все факты из дерева (включая вложенные fact-правила).
pub(super) fn extract_all_facts_from_tree<'r, 'a, P>(
    registry: &RuleRegistry<'r>,
    tree: &Tree,
    provider: &P,
) -> Vec<ExtractedFact>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    ExtractionContext::new(registry, provider).extract_all_facts_from_root(tree)
}

// ---------------------------------------------------------------------------
// Internal extraction logic
// ---------------------------------------------------------------------------

fn build_fact_scheme(name: &str, mut keys: Vec<String>) -> FactScheme {
    keys.sort();
    crate::interpretation::fact(name.to_string(), keys)
}

fn build_extracted_fact(
    descriptor: FactDescriptor,
    fact: &PendingFact,
    spans: Vec<Span>,
) -> Option<ExtractedFact> {
    let scheme = descriptor
        .scheme
        .map(|scheme| scheme.as_ref().clone())
        .unwrap_or_else(|| {
            build_fact_scheme(&descriptor.name, fact.fields().keys().cloned().collect())
        });
    let kwargs: BTreeMap<String, FactValue> = fact
        .fields()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let spans = normalize_spans(spans);
    let mut record = scheme.try_new_record(kwargs).ok()?;
    let as_json = record.as_json();
    record.raw = Some(FactRecordRaw { as_json, spans });
    Some(ExtractedFact::new(record))
}

/// Контекст parser-side extraction, чтобы не протаскивать одни и те же зависимости
/// через каждый рекурсивный вызов.
///
/// Здесь намеренно хранятся только readonly-ссылки:
/// - `registry` для доступа к production/правилам;
/// - `provider` для построения `NodeTextValue` конкретного узла.
struct ExtractionContext<'r, 'a, 'ctx, P>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    registry: &'ctx RuleRegistry<'r>,
    provider: &'ctx P,
}

impl<'r, 'a, 'ctx, P> ExtractionContext<'r, 'a, 'ctx, P>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    #[inline]
    fn new(registry: &'ctx RuleRegistry<'r>, provider: &'ctx P) -> Self {
        Self { registry, provider }
    }

    fn extract_fact_from_root(&self, tree: &Tree) -> Option<ExtractedFact> {
        // Для корня сначала пытаемся взять собственный fact-descriptor.
        // Если его нет (например, обёртка-диспетчер), ищем дескриптор у первого
        // подходящего дочернего узла.
        let descriptor = find_fact_descriptor(self.registry, &tree.root)
            .or_else(|| find_child_fact_descriptor(self.registry, &tree.root))?;
        let mut fact = PendingFact::new(descriptor.name.clone());
        let mut spans = Vec::new();
        self.extract_from_node(&tree.root, &mut fact, &mut spans);
        build_extracted_fact(descriptor, &fact, spans)
    }

    fn extract_all_facts_from_root(&self, tree: &Tree) -> Vec<ExtractedFact> {
        // В отличие от extract_fact_from_root, здесь идём по всему дереву и
        // собираем каждый встреченный факт (включая вложенные fact-правила).
        let mut facts = Vec::new();
        self.collect_facts(&tree.root, &mut facts);
        facts
    }

    /// Рекурсивно заполняет `fact` по дереву и продукции текущего узла.
    ///
    /// Поведение:
    /// - для `Terminal` применяет `interpretation` к текущему ребенку;
    /// - для `NonTerminal` уходит рекурсивно в дочерний узел.
    fn extract_from_node(&self, node: &Node, fact: &mut PendingFact, spans: &mut Vec<Span>) {
        let steps = node_interpretation_steps(self.registry, node);
        if !steps.is_empty() {
            // Если на узле есть интерпретационный pipeline, применяем его как
            // единый шаг и не спускаемся в стандартный production-loop.
            self.apply_rule_interpretation_pipeline(node, &steps, fact, spans);
            return;
        }

        if let Some(production) = self
            .registry
            .production(node.rule_id, node.production_index)
        {
            let mut child_iter = node.children.iter();

            for symbol in &production.terms {
                match symbol {
                    Term::Pred(_) => {
                        // Pred-символу соответствует leaf в children: продвигаем итератор,
                        // но в факт ничего напрямую не добавляем.
                        let _ = child_iter.next();
                    }
                    Term::Rule(_) => {
                        if let Some(ParseChild::Node(child_node)) = child_iter.next() {
                            // Только вложенные Node продолжают extraction-рекурсию.
                            self.extract_from_node(child_node, fact, spans);
                        } else {
                            // Структура children и terms должна быть синхронной, но
                            // fallback оставлен для устойчивости к частично битому дереву.
                            let _ = child_iter.next();
                        }
                    }
                }
            }
        }
    }
}

fn apply_interpretation_value<'a>(
    interp: &Interpretation,
    base_value: FactValue,
    text_value: Option<&NodeTextValue<'a>>,
    value_span: Span,
    fact: &mut PendingFact,
    spans: &mut Vec<Span>,
) {
    let mut current = if let Some(const_val) = &interp.const_value {
        const_val.clone()
    } else {
        base_value
    };

    for t in &interp.transforms {
        match t {
            Transform::Normalized => {
                if let Some(text) = text_value {
                    current = FactValue::Str(text.normalized_text());
                }
            }
            Transform::Inflected(forms) => {
                if let Some(text) = text_value {
                    current = FactValue::Str(text.inflected_text(Some(forms.as_slice())));
                }
            }
            Transform::Custom(f) => {
                if let Some(next) = f(&current) {
                    current = next;
                }
            }
        }
    }

    set_fact_field(fact, &interp.field_name, current, interp.repeatable);
    spans.push(value_span);
}

fn set_fact_field(fact: &mut PendingFact, field_name: &str, value: FactValue, repeatable: bool) {
    if repeatable {
        if let Some(existing) = fact.fields_mut().get_mut(field_name) {
            match (existing, value) {
                (FactValue::List(items), FactValue::List(new_items)) => {
                    // repeatable + list/list => дописываем элементы без лишнего оборачивания.
                    items.extend(new_items);
                }
                (FactValue::List(items), new_value) => {
                    // repeatable + list/scalar => scalar становится очередным элементом.
                    items.push(new_value);
                }
                (slot, new_value) => {
                    // repeatable + scalar/scalar => конвертируем существующее значение в список.
                    let prev = std::mem::replace(slot, FactValue::List(Vec::new()));
                    *slot = prev.push_into_list(new_value);
                }
            }
        } else {
            fact.fields_mut()
                .insert(field_name.to_string(), FactValue::List(vec![value]));
        }
    } else {
        fact.fields_mut().insert(field_name.to_string(), value);
    }
}

impl<'r, 'a, 'ctx, P> ExtractionContext<'r, 'a, 'ctx, P>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    fn apply_rule_interpretation_pipeline(
        &self,
        node: &Node,
        steps: &[RuleInterpretationStep],
        fact: &mut PendingFact,
        spans: &mut Vec<Span>,
    ) {
        if steps.is_empty() {
            return;
        }

        // Все spans внутри этого pipeline сначала копим локально, а затем одним
        // блоком переносим наружу — так проще сохранять атомарность шага.
        let value_span = node_span(node);
        let mut local_spans = Vec::new();

        let mut state = if matches!(steps.first(), Some(RuleInterpretationStep::Fact(_))) {
            let initial_name = match steps.first() {
                Some(RuleInterpretationStep::Fact(name)) => name.clone(),
                _ => "__pipeline__".to_string(),
            };
            let mut pending = PendingFact::new(initial_name);
            // Ветвь Fact(...) стартует не с raw-текста, а с уже извлечённых children-полей.
            self.extract_children_into_fact(node, &mut pending, &mut local_spans);
            RulePipelineState::PendingFact(pending)
        } else {
            // Ветвь Attribute(...) стартует со строкового представления узла.
            let text = (self.provider)(node);
            RulePipelineState::Value(FactValue::Str(text.raw_text()), Some(text))
        };

        for step in steps {
            match step {
                RuleInterpretationStep::Attribute(interp) => {
                    let (input, text_value) = match state {
                        RulePipelineState::Value(value, text_value) => (value, text_value),
                        RulePipelineState::PendingFact(pending) => (
                            // Атрибут поверх PendingFact получает object-слепок текущих полей.
                            FactValue::Object(pending_fact_object_entries(&pending)),
                            None,
                        ),
                    };

                    let mut next_fact = PendingFact::new(interp.fact_name.clone());
                    apply_interpretation_value(
                        interp,
                        input,
                        text_value.as_ref(),
                        value_span,
                        &mut next_fact,
                        &mut local_spans,
                    );
                    state = RulePipelineState::PendingFact(next_fact);
                }
                RuleInterpretationStep::Fact(name) => {
                    let pending = match state {
                        RulePipelineState::PendingFact(pending) => pending,
                        RulePipelineState::Value(value, _) => match value {
                            FactValue::Object(items) => {
                                // Значение уже object — просто фиксируем как Value и идём дальше.
                                state = RulePipelineState::Value(FactValue::Object(items), None);
                                continue;
                            }
                            _ => PendingFact::new(name.clone()),
                        },
                    };
                    state = RulePipelineState::Value(
                        FactValue::Object(pending_fact_object_entries(&pending)),
                        None,
                    );
                }
            }
        }

        match state {
            RulePipelineState::PendingFact(pending) => {
                for (key, value) in pending.fields() {
                    if let Some(existing) = fact.get(key) {
                        if matches!(existing, FactValue::List(_))
                            || matches!(value, FactValue::List(_))
                        {
                            // На merge приоритет у repeatable-семантики: объединяем в list.
                            let merged = existing.clone().push_into_list(value.clone());
                            fact.set(key.clone(), merged);
                            continue;
                        }
                    }
                    fact.set(key.clone(), value.clone());
                }
            }
            RulePipelineState::Value(FactValue::Object(items), _) => {
                for (key, value) in items {
                    if let Some(existing) = fact.get(&key) {
                        if matches!(existing, FactValue::List(_))
                            || matches!(value, FactValue::List(_))
                        {
                            // Аналогичный merge для object-ветки пайплайна.
                            let merged = existing.clone().push_into_list(value);
                            fact.set(key, merged);
                            continue;
                        }
                    }
                    fact.set(key, value);
                }
            }
            RulePipelineState::Value(_, _) => {}
        }

        spans.extend(local_spans);
    }

    fn extract_children_into_fact(
        &self,
        node: &Node,
        fact: &mut PendingFact,
        spans: &mut Vec<Span>,
    ) {
        let Some(_rule) = self.registry.get(node.rule_id) else {
            return;
        };
        let Some(production) = self
            .registry
            .production(node.rule_id, node.production_index)
        else {
            return;
        };

        let mut child_iter = node.children.iter();
        for symbol in &production.terms {
            match symbol {
                Term::Pred(_) => {
                    // Pred-элементы только синхронизируют child-итератор.
                    let _ = child_iter.next();
                }
                Term::Rule(_) => {
                    if let Some(ParseChild::Node(child_node)) = child_iter.next() {
                        // Для fact-start pipeline собираем вклад каждого rule-ребёнка.
                        self.extract_from_node(child_node, fact, spans);
                    } else {
                        let _ = child_iter.next();
                    }
                }
            }
        }
    }
}

fn pending_fact_object_entries(fact: &PendingFact) -> Vec<(String, FactValue)> {
    fact.fields()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

impl<'r, 'a, 'ctx, P> ExtractionContext<'r, 'a, 'ctx, P>
where
    P: Fn(&Node) -> NodeTextValue<'a>,
{
    /// Рекурсивно обходит дерево и собирает все факты в `facts`.
    fn collect_facts(&self, node: &Node, facts: &mut Vec<ExtractedFact>) {
        if let Some(descriptor) = find_fact_descriptor(self.registry, node) {
            // Каждый узел с факт-дескриптором материализуется независимо,
            // поэтому вложенные факты не затирают родительские.
            let mut fact = PendingFact::new(descriptor.name.clone());
            let mut spans = Vec::new();
            self.extract_from_node(node, &mut fact, &mut spans);
            if let Some(extracted) = build_extracted_fact(descriptor, &fact, spans) {
                facts.push(extracted);
            }
        }

        for child in &node.children {
            if let ParseChild::Node(n) = child {
                self.collect_facts(n, facts);
            }
        }
    }
}
