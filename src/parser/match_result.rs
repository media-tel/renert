//! Подготовка результатов парсинга и извлечение фактов из дерева разбора.
//!
//! Модуль закрывает "последнюю милю" после Earley-чарта:
//! - [`prepare_trees`] переводит итоговые [`State`] в [`Tree`] (без нормализации отношений);
//! - [`prepare_match`], [`prepare_matches`] и [`prepare_resolved_matches`] нормализуют деревья
//!   и при необходимости прогоняют отношения (см. [`prepare_tree`](crate::tree::transform::prepare_tree));
//! - [`resolve_spans`] устраняет пересечения диапазонов `(start, stop)` токенов;
//! - [`MatchBorrowed`] и [`MatchOwned`] — представления совпадения с деревом и токенами;
//! - методы `fact` / `facts` на матчах извлекают [`FactRecord`] по `interpretation` в терминалах.
//!
//! [`Parser`](crate::parser::Parser) строит деревья, вызывает функции подготовки из этого модуля,
//! затем оборачивает результат в `Match*` — сами `prepare_*` тип `Match` не возвращают.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::internal::FactRecord;
pub use crate::interpretation::FactValue;
use crate::rule::builder::RuleId;
use crate::rule::registry::RuleRegistry;
use crate::span::Span;
use crate::token::{AnyToken, AnyTokenOwnedLite, Token};

use super::fact_extract::{
    extract_all_facts_from_tree, extract_fact_from_tree, node_leaf_any_tokens_from_owned,
    node_leaf_tokens_from_owned, node_leaf_tokens_from_plain, NodeTextValue,
};
use super::item::State;
use crate::tree::{Node, ParseChild, Tree};

/// Извлечённый факт: [`FactRecord`] и связанные с ним символьные диапазоны в тексте.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedFact {
    /// Запись факта после извлечения из дерева разбора.
    pub fact: FactRecord,
    /// Снимок [`FactRecord::spans`] на момент [`ExtractedFact::new`]; при ручной инициализации полей
    /// может не совпадать с последующими вызовами `fact.spans()`.
    pub spans: Vec<Span>,
}

impl ExtractedFact {
    #[inline]
    pub fn new(fact: FactRecord) -> Self {
        let spans = fact.spans();
        Self { fact, spans }
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.fact.name()
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<&FactValue> {
        self.fact.get(key)
    }

    /// Упорядоченная проекция полей факта (делегат [`FactRecord::as_json`]).
    #[inline]
    pub fn as_json(&self) -> Vec<(String, FactValue)> {
        self.fact.as_json()
    }

    /// JSON-снимок для API/БД: имя факта, поля (как [`FactRecord::to_json_value`]) и диапазоны.
    ///
    /// Формат: `{"name": "...", "fields": { ... }, "spans": [ {"start": n, "stop": m}, ... ]}`.
    /// Поле `fields` сохраняет порядок ключей из [`Self::as_json`]. Массив `spans` берётся из
    /// снимка [`Self::spans`], зафиксированного при создании факта, а не пересобирается из
    /// текущих атрибутов [`FactRecord`].
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        let spans: Vec<serde_json::Value> = self
            .spans
            .iter()
            .map(|s| serde_json::json!({ "start": s.start, "stop": s.stop }))
            .collect();
        serde_json::json!({
            "name": self.name(),
            "fields": self.fact.to_json_value(),
            "spans": spans,
        })
    }

    #[inline]
    pub fn into_record(self) -> FactRecord {
        self.fact
    }
}

impl fmt::Display for ExtractedFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fact.fmt(f)
    }
}

/// Заимствованное представление совпадения.
///
/// Содержит:
/// - дерево разбора `tree`;
/// - диапазон токенов `(start, stop)` в исходной последовательности;
/// - ссылку на исходный срез токенов, по индексам которого восстанавливаются листья.
#[derive(Debug, Clone)]
pub struct MatchBorrowed<'t> {
    tree: Tree,
    token_range: (usize, usize),
    tokens: &'t [Token<'t>],
}

impl<'t> MatchBorrowed<'t> {
    /// Создает новое заимствованное совпадение.
    pub fn new(tree: Tree, token_range: (usize, usize), tokens: &'t [Token<'t>]) -> Self {
        Self {
            tree,
            token_range,
            tokens,
        }
    }

    /// Идентификатор правила корня дерева.
    #[inline]
    pub fn rule_id(&self) -> RuleId {
        self.tree.rule_id()
    }

    /// Алиас для [`Self::rule_id`].
    #[inline]
    pub fn rule(&self) -> RuleId {
        self.rule_id()
    }

    /// Возвращает листовые токены в порядке обхода дерева.
    ///
    /// Токены восстанавливаются через `tree.leaf_indices()` из исходного среза `tokens`.
    ///
    /// Паника:
    /// паникует, если в дереве встретился индекс листа вне `self.tokens`.
    #[inline]
    pub fn leaf_tokens(&self) -> Vec<Token<'t>> {
        self.tree
            .leaf_indices()
            .into_iter()
            .map(|idx| {
                self.tokens
                    .get(idx)
                    .cloned()
                    .expect("leaf token index out of bounds")
            })
            .collect()
    }

    /// Алиас для [`Self::leaf_tokens`].
    #[inline]
    pub fn tokens(&self) -> Vec<Token<'t>> {
        self.leaf_tokens()
    }

    /// Возвращает листовые токены, приведенные к `AnyToken::Plain`.
    pub fn tokens_any(&self) -> Vec<AnyToken<'t>> {
        self.leaf_tokens()
            .into_iter()
            .map(AnyToken::Plain)
            .collect()
    }

    /// Символьный диапазон совпадения в исходном тексте.
    #[inline]
    pub fn span(&self) -> Span {
        self.tree.span()
    }

    /// Диапазон токенов `(start, stop)` в исходной токенизации.
    #[inline]
    pub fn token_range(&self) -> (usize, usize) {
        self.token_range
    }

    /// Доступ к дереву разбора.
    #[inline]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Возвращает срез `source`, покрытый диапазоном [`Self::span`].
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        let s = self.span();
        &source[s.start..s.stop]
    }

    /// Применяет пользовательский экстрактор к дереву разбора.
    pub fn extract<F, T>(&self, extractor: F) -> T
    where
        F: FnOnce(&Tree) -> T,
    {
        extractor(&self.tree)
    }

    /// Возвращает все узлы дерева с указанным `rule_id`.
    pub fn nodes_by_rule(&self, rule_id: RuleId) -> Vec<Arc<Node>> {
        self.tree
            .walk()
            .filter_map(|it| match it {
                ParseChild::Node(n) if n.rule_id == rule_id => Some(n),
                _ => None,
            })
            .collect()
    }

    /// Возвращает первый найденный узел дерева с указанным `rule_id`.
    pub fn node_by_rule(&self, rule_id: RuleId) -> Option<Arc<Node>> {
        self.tree.walk().find_map(|it| match it {
            ParseChild::Node(n) if n.rule_id == rule_id => Some(n),
            _ => None,
        })
    }

    /// Извлекает один факт (по корневому правилу), если у правила задан `fact_name`.
    pub fn fact<'r>(&self, registry: &RuleRegistry<'r>) -> Option<ExtractedFact> {
        let provider = |node: &Node| {
            NodeTextValue::new(
                node_leaf_tokens_from_plain(node, self.tokens),
                None,
                registry
                    .pipeline_value(node.rule_id, node.production_index)
                    .map(str::to_string),
            )
        };
        extract_fact_from_tree(registry, &self.tree, &provider)
    }

    /// Извлекает все факты из дерева, включая вложенные fact-правила.
    pub fn facts<'r>(&self, registry: &RuleRegistry<'r>) -> Vec<ExtractedFact> {
        let provider = |node: &Node| {
            NodeTextValue::new(
                node_leaf_tokens_from_plain(node, self.tokens),
                None,
                registry
                    .pipeline_value(node.rule_id, node.production_index)
                    .map(str::to_string),
            )
        };
        extract_all_facts_from_tree(registry, &self.tree, &provider)
    }
}

/// Owning-представление совпадения.
///
/// В отличие от [`MatchBorrowed`], не зависит от внешних ссылок:
/// хранит `text` и `tokens` внутри себя.
#[derive(Debug, Clone)]
pub struct MatchOwned {
    tree: Tree,
    token_range: (usize, usize),
    text: Arc<str>,
    tokens: Arc<[AnyTokenOwnedLite]>,
}

impl MatchOwned {
    /// Создает `MatchOwned` из `MatchBorrowed`.
    pub fn from_borrowed<'t>(
        m: &MatchBorrowed<'t>,
        text: Arc<str>,
        tokens: Vec<AnyTokenOwnedLite>,
    ) -> Self {
        Self {
            tree: m.tree.clone(),
            token_range: m.token_range,
            text,
            tokens: tokens.into(),
        }
    }

    /// Создает `MatchOwned` только из дерева.
    ///
    /// `text` и `tokens` будут пустыми.
    pub fn from_tree(tree: Tree) -> Self {
        let token_range = tree.range;
        Self {
            tree,
            token_range,
            text: Arc::from(""),
            tokens: Arc::from(Vec::<AnyTokenOwnedLite>::new()),
        }
    }

    /// Создает `MatchOwned` из готовых компонентов.
    pub fn new<T>(tree: Tree, token_range: (usize, usize), text: Arc<str>, tokens: T) -> Self
    where
        T: Into<Arc<[AnyTokenOwnedLite]>>,
    {
        Self {
            tree,
            token_range,
            text,
            tokens: tokens.into(),
        }
    }

    /// Идентификатор правила корня дерева.
    #[inline]
    pub fn rule_id(&self) -> RuleId {
        self.tree.rule_id()
    }

    /// Алиас для [`Self::rule_id`].
    #[inline]
    pub fn rule(&self) -> RuleId {
        self.rule_id()
    }

    /// Диапазон токенов `(start, stop)` в исходной токенизации.
    #[inline]
    pub fn token_range(&self) -> (usize, usize) {
        self.token_range
    }

    /// Доступ к дереву разбора.
    #[inline]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Полный исходный текст, связанный с совпадением.
    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Восстанавливает листовые токены из `AnyTokenOwnedLite` и `text`.
    ///
    /// Для каждого leaf-индекса берется span токена и строится `Token` с borrowed `value`.
    ///
    /// Паника:
    /// паникует, если leaf-индекс выходит за границы `self.tokens`.
    #[inline]
    pub fn leaf_tokens(&self) -> Vec<Token<'_>> {
        self.tree
            .leaf_indices()
            .into_iter()
            .map(|idx| {
                let t = self
                    .tokens
                    .get(idx)
                    .expect("leaf token index out of bounds for owned match");
                let span = t.span();
                let value = &self.text[span.start..span.stop];
                Token {
                    value: Cow::Borrowed(value),
                    span,
                    token_type: t.token_type(),
                }
            })
            .collect()
    }

    /// Алиас для [`Self::leaf_tokens`].
    #[inline]
    pub fn tokens(&self) -> Vec<Token<'_>> {
        self.leaf_tokens()
    }

    /// Потребляет совпадение и возвращает владение поверхностными значениями листовых токенов.
    ///
    /// Эквивалентно `self.tokens().into_iter().map(|t| t.value.into_owned()).collect()`.
    pub fn into_token_strings(self) -> Vec<String> {
        self.tokens()
            .into_iter()
            .map(|t| t.value.into_owned())
            .collect()
    }

    /// Символьный диапазон совпадения.
    #[inline]
    pub fn span(&self) -> Span {
        self.tree.span()
    }

    /// Подстрока `text`, покрытая диапазоном [`Self::span`].
    #[inline]
    pub fn matched_text(&self) -> &str {
        let s = self.span();
        &self.text[s.start..s.stop]
    }

    /// Возвращает все узлы дерева с указанным `rule_id`.
    pub fn nodes_by_rule(&self, rule_id: RuleId) -> Vec<Arc<Node>> {
        self.tree
            .walk()
            .filter_map(|it| match it {
                ParseChild::Node(n) if n.rule_id == rule_id => Some(n),
                _ => None,
            })
            .collect()
    }

    /// Возвращает первый найденный узел дерева с указанным `rule_id`.
    pub fn node_by_rule(&self, rule_id: RuleId) -> Option<Arc<Node>> {
        self.tree.walk().find_map(|it| match it {
            ParseChild::Node(n) if n.rule_id == rule_id => Some(n),
            _ => None,
        })
    }

    /// Возвращает все токены, прикрепленные к совпадению.
    #[inline]
    pub fn all_tokens(&self) -> &[AnyTokenOwnedLite] {
        &self.tokens
    }

    /// Возвращает срез токенов в `token_range`.
    #[inline]
    pub fn range_tokens(&self) -> &[AnyTokenOwnedLite] {
        &self.tokens[self.token_range.0..self.token_range.1]
    }

    /// Извлекает один факт (по корневому правилу), если у правила задан `fact_name`.
    pub fn fact<'r>(&self, registry: &RuleRegistry<'r>) -> Option<ExtractedFact> {
        let provider = |node: &Node| {
            NodeTextValue::new(
                node_leaf_tokens_from_owned(node, &self.tokens, &self.text),
                Some(node_leaf_any_tokens_from_owned(
                    node,
                    &self.tokens,
                    &self.text,
                )),
                registry
                    .pipeline_value(node.rule_id, node.production_index)
                    .map(str::to_string),
            )
        };
        extract_fact_from_tree(registry, &self.tree, &provider)
    }

    /// Извлекает все факты из дерева, включая вложенные fact-правила.
    pub fn facts<'r>(&self, registry: &RuleRegistry<'r>) -> Vec<ExtractedFact> {
        let provider = |node: &Node| {
            NodeTextValue::new(
                node_leaf_tokens_from_owned(node, &self.tokens, &self.text),
                Some(node_leaf_any_tokens_from_owned(
                    node,
                    &self.tokens,
                    &self.text,
                )),
                registry
                    .pipeline_value(node.rule_id, node.production_index)
                    .map(str::to_string),
            )
        };
        extract_all_facts_from_tree(registry, &self.tree, &provider)
    }
}

/// Основной алиас результата совпадения во внешнем API.
pub type Match = MatchOwned;

/// Преобразует финальные состояния Earley в деревья разбора.
pub fn prepare_trees(states: impl IntoIterator<Item = State>) -> Vec<Tree> {
    states
        .into_iter()
        .map(|state| {
            let range = state.range();
            Tree::new(state.node, range)
        })
        .collect()
}

/// Подготавливает одно дерево через [`prepare_tree`](crate::tree::transform::prepare_tree).
///
/// Сначала [`Tree::normalized`](crate::tree::Tree::normalized), затем [`collect_relations`](crate::tree::transform::collect_relations).
/// Если граф отношений пуст, возвращается нормализованное дерево **без**
/// [`FormsRelationsGraph::validate`](crate::relations::FormsRelationsGraph::validate) и
/// [`apply_relations`](crate::tree::transform::apply_relations). Иначе при успешной проверке графа
/// вызывается `apply_relations`.
///
/// Возвращает `None`, если после нормализации дерево пустое, либо если граф отношений непустой,
/// но [`FormsRelationsGraph::validate`](crate::relations::FormsRelationsGraph::validate) даёт `false`.
pub fn prepare_match<'r>(
    tree: Tree,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> Option<Tree> {
    crate::tree::transform::prepare_tree(tree, registry, tokens)
}

/// Подготавливает все деревья и отбрасывает невалидные.
pub fn prepare_matches<'r>(
    trees: impl IntoIterator<Item = Tree>,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> Vec<Tree> {
    trees
        .into_iter()
        .filter_map(|t| prepare_match(t, registry, tokens))
        .collect()
}

/// Подготавливает деревья и устраняет конфликтующие диапазоны.
///
/// Для одинаковых диапазонов сохраняется первое успешное дерево,
/// затем применяется жадное разрешение пересечений через `resolve_spans` в этом же модуле.
pub fn prepare_resolved_matches<'r>(
    trees: impl IntoIterator<Item = Tree>,
    registry: &RuleRegistry<'r>,
    tokens: &[AnyToken<'_>],
) -> Vec<Tree> {
    let mut spans = Vec::new();
    let mut span_trees: HashMap<(usize, usize), Tree> = HashMap::new();

    for tree in trees {
        let span = tree.range;
        if span_trees.contains_key(&span) {
            continue;
        }

        if let Some(tree) = prepare_match(tree, registry, tokens) {
            spans.push(span);
            span_trees.insert(span, tree);
        }
    }

    resolve_spans(spans)
        .into_iter()
        .filter_map(|span| span_trees.remove(&span))
        .collect()
}

/// Разрешает пересекающиеся диапазоны `(start, stop)`.
///
/// Стратегия:
/// - сортировка по `start`, при равном `start` — по убыванию `stop`;
/// - затем жадный проход слева направо.
///
/// Это дает предпочтение более длинному диапазону при одинаковом старте.
pub fn resolve_spans(mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    spans.sort_by(|a, b| {
        if a.0 != b.0 {
            a.0.cmp(&b.0)
        } else {
            b.1.cmp(&a.1)
        }
    });

    let mut out = Vec::new();
    let mut cur: Option<(usize, usize)> = None;

    for s in spans {
        match cur {
            None => cur = Some(s),
            Some(c) => {
                if s.0 >= c.1 {
                    out.push(c);
                    cur = Some(s);
                } else if s.1 > c.1 {
                    cur = Some(s);
                }
            }
        }
    }

    if let Some(c) = cur {
        out.push(c);
    }

    out
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::rule::builder::RuleId;
    use crate::span::Span;
    use crate::token::TokenType;
    use crate::tree::{Leaf, ParseChild};

    fn make_token(value: &'static str, start: usize) -> Token<'static> {
        Token {
            value: Cow::Borrowed(value),
            span: Span::new(start, start + value.len()),
            token_type: TokenType::Russian,
        }
    }

    fn make_simple_tree() -> Tree {
        let l1 = Arc::new(Leaf::new(0, 0, Span::new(0, 5)));
        let l2 = Arc::new(Leaf::new(0, 1, Span::new(6, 11)));
        let root = Arc::new(Node {
            rule_id: RuleId(1),
            production_index: 0,
            rank: 0,
            children: vec![ParseChild::Leaf(l1), ParseChild::Leaf(l2)],
        });
        Tree::new(root, (0, 2))
    }

    #[test]
    fn borrowed_match_resolves_tokens_by_indices() {
        let tokens = vec![make_token("hello", 0), make_token("world", 6)];
        let tree = make_simple_tree();
        let m = MatchBorrowed::new(tree, (0, 2), &tokens);
        let leaves = m.leaf_tokens();
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].value.as_ref(), "hello");
        assert_eq!(leaves[1].value.as_ref(), "world");
        assert_eq!(m.span(), Span::new(0, 11));
    }

    #[test]
    fn owned_match_materializes_text_from_spans() {
        let tree = make_simple_tree();
        let owned = MatchOwned::new(
            tree,
            (0, 2),
            Arc::<str>::from("hello world"),
            vec![
                AnyTokenOwnedLite::Plain {
                    span: Span::new(0, 5),
                    token_type: TokenType::Russian,
                },
                AnyTokenOwnedLite::Plain {
                    span: Span::new(6, 11),
                    token_type: TokenType::Russian,
                },
            ],
        );

        assert_eq!(owned.matched_text(), "hello world");
        let toks = owned.tokens();
        assert_eq!(toks[0].value.as_ref(), "hello");
        assert_eq!(toks[1].value.as_ref(), "world");
    }

    #[test]
    fn resolve_spans_prefers_longest_from_same_start() {
        let resolved = resolve_spans(vec![(0, 1), (0, 2), (3, 4)]);
        assert_eq!(resolved, vec![(0, 2), (3, 4)]);
    }
}
