//! Граф отношений.
//!
//! Модуль повторяет идею Python-реализации:
//! - [`RelationsGraph`] хранит элементы, сгруппированные по отношению;
//! - [`TokenRelationsGraph`] валидирует и ограничивает формы токенов.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::morph::models::Form;
use crate::token::MorphToken;

/// Бинарное отношение между двумя морфологическими формами.
pub trait Relation: Send + Sync {
    /// Возвращает `true`, если формы совместимы по данному отношению.
    fn check(&self, first: &Form, second: &Form) -> bool;

    /// Возвращает человекочитаемую метку отношения.
    fn label(&self) -> String {
        "relation(...)".into()
    }
}

fn relation_key(relation: &Arc<dyn Relation>) -> usize {
    Arc::as_ptr(relation) as *const () as usize
}

fn token_key(token: &Arc<MorphToken<'static>>) -> usize {
    Arc::as_ptr(token) as usize
}

/// Сужает две коллекции форм, оставляя только совместимые по `relation` пары.
///
/// Порядок результата определяется порядком первого появления в декартовом произведении.
fn narrow_forms_for_pair(
    relation: &dyn Relation,
    first_forms: &[Form],
    second_forms: &[Form],
) -> (Vec<Form>, Vec<Form>) {
    let mut seen_first = HashSet::new();
    let mut seen_second = HashSet::new();
    let mut checked_first = Vec::new();
    let mut checked_second = Vec::new();

    for (i, ff) in first_forms.iter().enumerate() {
        for (j, sf) in second_forms.iter().enumerate() {
            if relation.check(ff, sf) {
                if seen_first.insert(i) {
                    checked_first.push(ff.clone());
                }
                if seen_second.insert(j) {
                    checked_second.push(sf.clone());
                }
            }
        }
    }

    (checked_first, checked_second)
}

/// Ребро графа: отношение и пара элементов.
#[derive(Clone)]
pub struct Edge<T> {
    /// Отношение, которому принадлежит ребро.
    pub relation: Arc<dyn Relation>,
    /// Первый элемент пары.
    pub first: T,
    /// Второй элемент пары.
    pub second: T,
}

/// Базовый граф отношений для произвольных элементов.
pub struct RelationsGraph<T> {
    relations: HashMap<usize, Arc<dyn Relation>>,
    relation_items: HashMap<usize, Vec<T>>,
    insertion_order: Vec<usize>,
    insertion_set: HashSet<usize>,
}

impl<T: Clone> RelationsGraph<T> {
    /// Создает пустой граф.
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
            relation_items: HashMap::new(),
            insertion_order: Vec::new(),
            insertion_set: HashSet::new(),
        }
    }

    /// Добавляет `item` в группу отношения `relation`.
    pub fn add(&mut self, relation: Arc<dyn Relation>, item: T) {
        let key = relation_key(&relation);
        self.relations.entry(key).or_insert(relation);
        self.relation_items.entry(key).or_default().push(item);
        if self.insertion_set.insert(key) {
            self.insertion_order.push(key);
        }
    }

    /// Итератор по ребрам (все 2-комбинации внутри каждой группы отношения).
    pub fn edges(&self) -> impl Iterator<Item = Edge<&T>> + '_ {
        self.insertion_order.iter().flat_map(|key| {
            let relation = self.relations[key].clone();
            let items = &self.relation_items[key];
            (0..items.len()).flat_map(move |i| {
                (i + 1..items.len()).map({
                    let relation = relation.clone();
                    move |j| Edge {
                        relation: relation.clone(),
                        first: &items[i],
                        second: &items[j],
                    }
                })
            })
        })
    }
}

impl<T: Clone> Default for RelationsGraph<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Граф отношений для морфологических токенов.
pub struct TokenRelationsGraph {
    base: RelationsGraph<Arc<MorphToken<'static>>>,
    tokens: HashMap<usize, Arc<MorphToken<'static>>>,
    token_forms: HashMap<usize, Vec<Form>>,
}

impl TokenRelationsGraph {
    /// Создает пустой граф токенов.
    pub fn new() -> Self {
        Self {
            base: RelationsGraph::new(),
            tokens: HashMap::new(),
            token_forms: HashMap::new(),
        }
    }

    /// Добавляет токен под указанное отношение.
    pub fn add(&mut self, relation: Arc<dyn Relation>, token: Arc<MorphToken<'static>>) {
        let key = token_key(&token);
        self.base.add(relation, token.clone());
        self.tokens.entry(key).or_insert_with(|| token.clone());
        self.token_forms
            .entry(key)
            .or_insert_with(|| token.forms.clone());
    }

    /// Валидирует связи и сужает формы токенов.
    ///
    /// Возвращает `false`, если хотя бы у одного токена не осталось форм.
    pub fn validate(&mut self) -> bool {
        let edges: Vec<Edge<Arc<MorphToken<'static>>>> = self
            .base
            .edges()
            .map(|edge| Edge {
                relation: edge.relation.clone(),
                first: edge.first.clone(),
                second: edge.second.clone(),
            })
            .collect();

        for edge in &edges {
            let first_key = token_key(&edge.first);
            let second_key = token_key(&edge.second);

            let first_forms = self.token_forms[&first_key].clone();
            let second_forms = self.token_forms[&second_key].clone();

            let (checked_first, checked_second) =
                narrow_forms_for_pair(edge.relation.as_ref(), &first_forms, &second_forms);

            self.token_forms.insert(first_key, checked_first);
            self.token_forms.insert(second_key, checked_second);
        }

        self.tokens.keys().all(|token_id| {
            self.token_forms
                .get(token_id)
                .is_some_and(|forms| !forms.is_empty())
        })
    }

    /// Возвращает constrained-версию токена, если он участвует в графе.
    pub fn constrain(&self, token: &Arc<MorphToken<'static>>) -> MorphToken<'static> {
        let key = token_key(token);
        if let Some(stored) = self.tokens.get(&key) {
            let forms = self.token_forms.get(&key).cloned().unwrap_or_default();
            (**stored).clone().constrained(forms)
        } else {
            (**token).clone()
        }
    }
}

impl Default for TokenRelationsGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Граф отношений для валидации в дереве разбора.
///
/// В отличие от [`TokenRelationsGraph`], работает по `token_index` (а не `Arc`-указателям)
/// и не зависит от [`MorphToken`]. Используется в pipeline `collect_relations → validate → constrain`.
pub struct FormsRelationsGraph {
    groups: HashMap<usize, (Arc<dyn Relation>, Vec<usize>)>,
    token_forms: HashMap<usize, Vec<Form>>,
    insertion_order: Vec<usize>,
}

impl FormsRelationsGraph {
    /// Создаёт пустой граф.
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            token_forms: HashMap::new(),
            insertion_order: Vec::new(),
        }
    }

    /// Добавляет токен в группу отношения.
    ///
    /// `relation_key` связывает несколько вызовов `add` для одного экземпляра отношения
    /// (используется `Arc`-указатель на relation как ключ).
    pub fn add(&mut self, relation: &Arc<dyn Relation>, token_index: usize, forms: Vec<Form>) {
        let key = relation_key(relation);
        let entry = self.groups.entry(key).or_insert_with(|| {
            self.insertion_order.push(key);
            (relation.clone(), Vec::new())
        });
        entry.1.push(token_index);
        self.token_forms.entry(token_index).or_insert_with(|| forms);
    }

    /// Сужает формы через попарную проверку.
    ///
    /// Возвращает `false`, если хотя бы у одного токена не осталось совместимых форм.
    pub fn validate(&mut self) -> bool {
        for &key in &self.insertion_order {
            let (ref relation, ref items) = self.groups[&key];
            for i in 0..items.len() {
                for j in (i + 1)..items.len() {
                    let first_idx = items[i];
                    let second_idx = items[j];

                    let first_forms = self.token_forms[&first_idx].clone();
                    let second_forms = self.token_forms[&second_idx].clone();

                    let (checked_first, checked_second) =
                        narrow_forms_for_pair(relation.as_ref(), &first_forms, &second_forms);

                    self.token_forms.insert(first_idx, checked_first);
                    self.token_forms.insert(second_idx, checked_second);
                }
            }
        }

        self.token_forms.values().all(|forms| !forms.is_empty())
    }

    /// Возвращает суженные формы для токена, если токен участвует в графе.
    pub fn constrained_forms(&self, token_index: usize) -> Option<&[Form]> {
        self.token_forms.get(&token_index).map(|v| v.as_slice())
    }

    /// Возвращает `true`, если граф пуст (нет ни одного отношения).
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl Default for FormsRelationsGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Grams;
    use crate::span::Span;
    use crate::token::{Token, TokenType};

    #[derive(Debug)]
    struct SameCaseRelation;

    impl Relation for SameCaseRelation {
        fn check(&self, first: &Form, second: &Form) -> bool {
            first.grams.case() == second.grams.case()
        }
    }

    fn make_form(normalized: &str, grams: &[&str]) -> Form {
        Form::new(
            normalized.to_string(),
            Grams::new(grams.iter().copied()),
            None,
        )
    }

    fn make_token(value: &'static str, forms: Vec<Form>) -> Arc<MorphToken<'static>> {
        Arc::new(Token::new(value, Span::new(0, value.len()), TokenType::Russian).morphed(forms))
    }

    #[test]
    fn relations_graph_yields_pairwise_edges() {
        let relation: Arc<dyn Relation> = Arc::new(SameCaseRelation);
        let mut graph = RelationsGraph::new();
        graph.add(relation.clone(), 1usize);
        graph.add(relation, 2usize);
        graph.add(Arc::new(SameCaseRelation), 3usize);

        let edges: Vec<(usize, usize)> = graph.edges().map(|e| (*e.first, *e.second)).collect();
        assert_eq!(edges, vec![(1, 2)]);
    }

    #[test]
    fn token_relations_graph_validate_and_constrain() {
        let nomn = make_form("дом", &["NOUN", "nomn", "sing"]);
        let gent = make_form("дома", &["NOUN", "gent", "sing"]);
        let t1 = make_token("Дом", vec![nomn.clone(), gent.clone()]);
        let t2 = make_token("улица", vec![nomn.clone()]);

        let relation: Arc<dyn Relation> = Arc::new(SameCaseRelation);
        let mut graph = TokenRelationsGraph::new();
        graph.add(relation.clone(), t1.clone());
        graph.add(relation, t2.clone());

        assert!(graph.validate());

        let c1 = graph.constrain(&t1);
        let c2 = graph.constrain(&t2);
        assert_eq!(c1.forms, vec![nomn.clone()]);
        assert_eq!(c2.forms, vec![nomn]);
    }

    #[test]
    fn token_relations_graph_validate_fails_when_forms_exhausted() {
        let nomn = make_form("дом", &["NOUN", "nomn", "sing"]);
        let gent = make_form("дома", &["NOUN", "gent", "sing"]);
        let t1 = make_token("Дом", vec![nomn]);
        let t2 = make_token("улицы", vec![gent]);

        let relation: Arc<dyn Relation> = Arc::new(SameCaseRelation);
        let mut graph = TokenRelationsGraph::new();
        graph.add(relation.clone(), t1.clone());
        graph.add(relation, t2);

        assert!(!graph.validate());
        let constrained = graph.constrain(&t1);
        assert!(constrained.forms.is_empty());
    }

    #[test]
    fn token_relations_graph_constrain_unknown_token_returns_as_is() {
        let nomn = make_form("дом", &["NOUN", "nomn", "sing"]);
        let known = make_token("Дом", vec![nomn.clone()]);
        let unknown = make_token("улица", vec![nomn.clone()]);

        let relation: Arc<dyn Relation> = Arc::new(SameCaseRelation);
        let mut graph = TokenRelationsGraph::new();
        graph.add(relation, known);
        assert!(graph.validate());

        let constrained = graph.constrain(&unknown);
        assert_eq!(constrained.forms, vec![nomn]);
    }
}
