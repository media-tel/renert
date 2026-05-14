use std::sync::Arc;

use crate::morph::models::Form;
use crate::rule::builder::RuleId;
use crate::span::Span;

use super::normalize::normalize_node;
use super::walker::TreeWalker;

/// Лист дерева разбора (терминальное совпадение).
#[derive(Debug, Clone)]
pub struct Leaf {
    /// Идентификатор предиката, породившего лист.
    pub predicate_id: usize,
    /// Индекс токена в исходной последовательности токенов.
    pub token_index: usize,
    /// Символьный диапазон листа в исходном тексте.
    pub span: Span,
    /// Морфоформы, суженные предикатом при успешном `scan`.
    ///
    /// `Some(forms)` — формы, оставшиеся после фильтрации предикатом (например `gram("Surn")`).
    /// `None` — предикат не производит сужения форм (plain-токен или предикат без морфологии).
    pub matched_forms: Option<Arc<[Form]>>,
}

impl Leaf {
    /// Создает лист дерева без суженных морфоформ.
    #[inline]
    pub fn new(predicate_id: usize, token_index: usize, span: Span) -> Self {
        Self {
            predicate_id,
            token_index,
            span,
            matched_forms: None,
        }
    }

    /// Создает лист дерева с суженными морфоформами.
    #[inline]
    pub fn with_forms(
        predicate_id: usize,
        token_index: usize,
        span: Span,
        matched_forms: Option<Arc<[Form]>>,
    ) -> Self {
        Self {
            predicate_id,
            token_index,
            span,
            matched_forms,
        }
    }
}

/// Нетерминальный узел дерева разбора.
#[derive(Debug, Clone)]
pub struct Node {
    /// Идентификатор правила, которому соответствует узел.
    pub rule_id: RuleId,
    /// Индекс продукции в правиле.
    pub production_index: usize,
    /// Ранг узла (используется при сравнении альтернатив).
    pub rank: usize,
    /// Дочерние элементы узла в порядке продукции.
    pub children: Vec<ParseChild>,
}

impl Node {
    /// Создает пустой узел без детей.
    #[inline]
    pub fn new(rule_id: RuleId, production_index: usize, rank: usize) -> Self {
        Self {
            rule_id,
            production_index,
            rank,
            children: Vec::new(),
        }
    }

    /// Возвращает новый узел с добавленным `child`.
    ///
    /// Метод персистентный: исходный узел не изменяется, создается новый `Arc<Node>`.
    #[inline]
    pub fn attached(&self, child: ParseChild) -> Arc<Node> {
        let mut children = self.children.clone();
        children.push(child);
        Arc::new(Node {
            rule_id: self.rule_id,
            production_index: self.production_index,
            rank: self.rank,
            children,
        })
    }
}

/// Ребенок узла: либо лист, либо вложенный узел.
#[derive(Debug, Clone)]
pub enum ParseChild {
    /// Листовой терминальный элемент.
    Leaf(Arc<Leaf>),
    /// Вложенный нетерминальный узел.
    Node(Arc<Node>),
}

/// Готовое дерево разбора.
#[derive(Debug, Clone)]
pub struct Tree {
    /// Корневой узел дерева.
    pub root: Arc<Node>,
    /// Диапазон токенов `(start, stop)`, покрываемый деревом.
    pub range: (usize, usize),
}

impl Tree {
    /// Создает дерево по корневому узлу и токенному диапазону.
    #[inline]
    pub fn new(root: Arc<Node>, range: (usize, usize)) -> Self {
        Self { root, range }
    }

    /// Возвращает идентификатор правила корня.
    #[inline]
    pub fn rule_id(&self) -> RuleId {
        self.root.rule_id
    }

    /// Собирает индексы листовых токенов в порядке обхода дерева.
    pub fn leaf_indices(&self) -> Vec<usize> {
        let mut out = Vec::new();
        collect_leaf_indices(&self.root, &mut out);
        out
    }

    /// Вычисляет общий символьный диапазон дерева.
    ///
    /// Берется минимум `start` и максимум `stop` среди всех листьев.
    /// Если листьев нет, возвращается `Span(0, 0)`.
    pub fn span(&self) -> Span {
        let mut spans = Vec::new();
        collect_leaf_spans(&self.root, &mut spans);
        if spans.is_empty() {
            return Span::new(0, 0);
        }

        let mut start = spans[0].start;
        let mut stop = spans[0].stop;
        for s in spans.iter().skip(1) {
            start = start.min(s.start);
            stop = stop.max(s.stop);
        }
        Span::new(start, stop)
    }

    /// Возвращает итератор обхода дерева в глубину (preorder).
    pub fn walk(&self) -> TreeWalker {
        TreeWalker::new(self.root.clone())
    }

    /// Нормализует дерево, удаляя пустые нетерминальные ветки.
    ///
    /// Если после нормализации дерево полностью пустое, возвращает `None`.
    pub fn normalized(&self) -> Option<Tree> {
        let root = normalize_node(self.root.clone())?;
        Some(Tree::new(root, self.range))
    }
}

/// Рекурсивно собирает индексы листьев из поддерева.
fn collect_leaf_indices(node: &Arc<Node>, out: &mut Vec<usize>) {
    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => out.push(leaf.token_index),
            ParseChild::Node(node) => collect_leaf_indices(node, out),
        }
    }
}

/// Рекурсивно собирает span всех листьев из поддерева.
fn collect_leaf_spans(node: &Arc<Node>, out: &mut Vec<Span>) {
    for child in &node.children {
        match child {
            ParseChild::Leaf(leaf) => out.push(leaf.span),
            ParseChild::Node(node) => collect_leaf_spans(node, out),
        }
    }
}
