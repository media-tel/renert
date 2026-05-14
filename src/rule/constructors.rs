//! Конструкторы грамматических термов, продукций и правил.
//!
//! Модуль описывает внутреннее представление графа правил и
//! вспомогательные функции для построения и обхода этого графа.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use crate::internal::FactScheme;
use crate::interpretation::{FactValue, Interpretation, Transform};
use crate::predicates::constructors::PredicateKind;
use crate::rule::transformator::{
    FlattenTransformator, ReplaceEmptyTransformator, ReplaceExtendedTransformator,
    ReplaceOrTransformator, SquashExtendedTransformator, Transformator,
};

/// Маркер `Main` для выбора главного терма в [`Production`].
#[derive(Debug, Clone)]
pub struct Main<T> {
    /// Термин, помеченный как главный.
    pub term: T,
}

impl<T> Main<T> {
    /// Создает новый маркер главного терма.
    pub fn new(term: T) -> Self {
        Self { term }
    }
}

/// Термин продукции: либо предикат, либо вложенное правило.
#[derive(Debug, Clone)]
pub enum Term<'a> {
    /// Предикат-термин.
    Pred(PredicateKind<'a>),
    /// Вложенное правило.
    Rule(Arc<Rule<'a>>),
}

impl<'a> Term<'a> {
    /// Возвращает текстовую метку терма.
    pub fn label(&self) -> String {
        match self {
            Term::Pred(p) => p.label(),
            Term::Rule(r) => r.label(),
        }
    }
}

/// Элемент входа для [`Production`]: обычный терм или помеченный как `Main`.
#[derive(Debug, Clone)]
pub enum TermOrMain<'a> {
    /// Обычный терм.
    Term(Term<'a>),
    /// Главный терм.
    Main(Term<'a>),
}

impl<'a> From<PredicateKind<'a>> for TermOrMain<'a> {
    fn from(p: PredicateKind<'a>) -> Self {
        TermOrMain::Term(Term::Pred(p))
    }
}

impl<'a> From<Arc<Rule<'a>>> for TermOrMain<'a> {
    fn from(r: Arc<Rule<'a>>) -> Self {
        TermOrMain::Term(Term::Rule(r))
    }
}

impl<'a> From<Rule<'a>> for TermOrMain<'a> {
    fn from(r: Rule<'a>) -> Self {
        TermOrMain::Term(Term::Rule(Arc::new(r)))
    }
}

impl<'a> From<Main<PredicateKind<'a>>> for TermOrMain<'a> {
    fn from(m: Main<PredicateKind<'a>>) -> Self {
        TermOrMain::Main(Term::Pred(m.term))
    }
}

impl<'a> From<Main<Arc<Rule<'a>>>> for TermOrMain<'a> {
    fn from(m: Main<Arc<Rule<'a>>>) -> Self {
        TermOrMain::Main(Term::Rule(m.term))
    }
}

/// Готовит список термов и индекс главного терма для продукции.
///
/// Если `Main` не указан, `main` будет равен `0`.
///
/// # Panics
/// Паникует, если указан более чем один `Main`.
fn prepare_terms<'a>(items: &[TermOrMain<'a>]) -> (Vec<Term<'a>>, usize) {
    let mut main: Option<usize> = None;
    let mut terms: Vec<Term<'a>> = Vec::with_capacity(items.len());

    for (index, item) in items.iter().enumerate() {
        match item {
            TermOrMain::Main(term) => {
                if main.is_some() {
                    panic!(">1 main");
                }
                main = Some(index);
                terms.push(term.clone());
            }
            TermOrMain::Term(term) => terms.push(term.clone()),
        }
    }

    (terms, main.unwrap_or(0))
}

/// Одна продукция правила: последовательность термов и индекс главного.
#[derive(Debug, Clone)]
pub struct Production<'a> {
    /// Термы продукции.
    pub terms: Vec<Term<'a>>,
    /// Индекс главного терма в [`Self::terms`].
    pub main: usize,
}

impl<'a> Production<'a> {
    /// Создает продукцию из набора термов.
    ///
    /// Приоритет определения `main`:
    /// 1. `main_override`, если задан.
    /// 2. Маркер `Main` во входных термах.
    /// 3. Значение `0`, если `Main` отсутствует.
    ///
    /// Если итоговый индекс выходит за границы непустого списка, он
    /// сбрасывается в `0`.
    ///
    /// # Panics
    /// Паникует, если во входе больше одного `Main`.
    pub fn new(items: impl Into<Vec<TermOrMain<'a>>>, main_override: Option<usize>) -> Self {
        let items = items.into();
        let (terms, detected_main) = prepare_terms(&items);

        let mut main = main_override.unwrap_or(detected_main);
        if !terms.is_empty() && main >= terms.len() {
            main = 0;
        }

        Self { terms, main }
    }

    /// Возвращает итератор по термам продукции.
    pub fn children(&self) -> impl Iterator<Item = &Term<'a>> {
        self.terms.iter()
    }

    /// Формирует строковое представление в стиле
    ///
    /// Главный терм (кроме индекса `0`) отмечается префиксом `^`.
    pub fn to_string_yargy(&self) -> String {
        let mut labels = Vec::with_capacity(self.terms.len());
        for (index, term) in self.terms.iter().enumerate() {
            let mut label = term.label();
            if self.main > 0 && self.main == index {
                label = format!("^{}", label);
            }
            labels.push(label);
        }
        labels.join(" ")
    }
}

/// Маркер пустой продукции.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmptyProduction;

impl EmptyProduction {
    /// Возвращает пустую продукцию с `main = 0`.
    pub fn as_production<'a>() -> Production<'a> {
        Production {
            terms: vec![],
            main: 0,
        }
    }

    /// Возвращает `true`, если `prod` — каноническая ε-продукция после нормализации
    /// ([`ReplaceEmptyTransformator`]).
    #[inline]
    pub fn matches_production(prod: &Production<'_>) -> bool {
        prod.terms.is_empty() && prod.main == 0
    }
}

/// Тип значения факта в интерпретации правил.
pub type RuleFactValue = FactValue;

/// Тип трансформации для интерпретации правил.
pub type RuleTransform = Transform;

/// Разновидности узлов в графе правил.
pub enum RuleKind<'a> {
    /// Базовое правило с фиксированным набором продукций.
    Base { productions: Vec<Production<'a>> },
    /// Альтернатива (`A | B | ...`) из дочерних правил.
    Or { rules: Vec<Arc<Rule<'a>>> },

    /// Опциональное правило.
    Optional { rule: Arc<Rule<'a>>, reverse: bool },
    /// Повторяемое правило без явных границ.
    Repeatable { rule: Arc<Rule<'a>>, reverse: bool },

    /// Комбинированный расширенный вариант "повторяемое + опциональное".
    RepeatableOptional {
        rule: Arc<Rule<'a>>,
        reverse_repeatable: bool,
        reverse_optional: bool,
    },

    /// Повторяемое правило с нижней границей.
    MinBounded {
        rule: Arc<Rule<'a>>,
        min: usize,
        reverse: bool,
    },
    /// Повторяемое правило с верхней границей.
    MaxBounded {
        rule: Arc<Rule<'a>>,
        max: usize,
        reverse: bool,
    },
    /// Повторяемое правило с нижней и верхней границей.
    MinMaxBounded {
        rule: Arc<Rule<'a>>,
        min: usize,
        max: usize,
        reverse: bool,
    },

    /// Именованная обертка вокруг правила.
    Named {
        rule: Arc<Rule<'a>>,
        name: String,
        fact_scheme: Option<Arc<FactScheme>>,
    },

    /// Обертка для payload интерпретации.
    Interpretation {
        rule: Arc<Rule<'a>>,
        interpretation: Interpretation,
    },
    /// Обертка для реляционного payload (согласование морфологических форм).
    Relation {
        rule: Arc<Rule<'a>>,
        relation: Arc<dyn crate::relations::Relation>,
    },

    /// Forward-правило, целевое значение задается позже через [`Rule::define_forward`].
    Forward {
        inner: parking_lot::RwLock<Option<Arc<Rule<'a>>>>,
    },

    /// Пустое правило.
    Empty,
    /// Обертка pipeline-представления с каноническим key по индексам продукций.
    ///
    /// Хранит исходное правило нетронутым, чтобы grammar/runtime могли прозрачно
    /// делегировать в него productions, а extraction мог брать канонический нормализованный key.
    Pipeline {
        rule: Arc<Rule<'a>>,
        pipeline: String,
        values: Arc<[String]>,
    },
}

/// Имплементация `Clone` для `RuleKind` с учетом рекурсивных структур и `Arc`.
impl<'a> Clone for RuleKind<'a> {
    fn clone(&self) -> Self {
        match self {
            RuleKind::Base { productions } => Self::Base {
                productions: productions.clone(),
            },
            RuleKind::Or { rules } => Self::Or {
                rules: rules.clone(),
            },
            RuleKind::Optional { rule, reverse } => Self::Optional {
                rule: rule.clone(),
                reverse: *reverse,
            },
            RuleKind::Repeatable { rule, reverse } => Self::Repeatable {
                rule: rule.clone(),
                reverse: *reverse,
            },
            RuleKind::RepeatableOptional {
                rule,
                reverse_repeatable,
                reverse_optional,
            } => Self::RepeatableOptional {
                rule: rule.clone(),
                reverse_repeatable: *reverse_repeatable,
                reverse_optional: *reverse_optional,
            },
            RuleKind::MinBounded { rule, min, reverse } => Self::MinBounded {
                rule: rule.clone(),
                min: *min,
                reverse: *reverse,
            },
            RuleKind::MaxBounded { rule, max, reverse } => Self::MaxBounded {
                rule: rule.clone(),
                max: *max,
                reverse: *reverse,
            },
            RuleKind::MinMaxBounded {
                rule,
                min,
                max,
                reverse,
            } => Self::MinMaxBounded {
                rule: rule.clone(),
                min: *min,
                max: *max,
                reverse: *reverse,
            },
            RuleKind::Named {
                rule,
                name,
                fact_scheme,
            } => Self::Named {
                rule: rule.clone(),
                name: name.clone(),
                fact_scheme: fact_scheme.clone(),
            },
            RuleKind::Interpretation {
                rule,
                interpretation,
            } => Self::Interpretation {
                rule: rule.clone(),
                interpretation: interpretation.clone(),
            },
            RuleKind::Relation { rule, relation } => Self::Relation {
                rule: rule.clone(),
                relation: relation.clone(),
            },
            RuleKind::Forward { inner } => Self::Forward {
                inner: parking_lot::RwLock::new(inner.read().clone()),
            },
            RuleKind::Empty => Self::Empty,
            RuleKind::Pipeline {
                rule,
                pipeline,
                values,
            } => Self::Pipeline {
                rule: rule.clone(),
                pipeline: pipeline.clone(),
                values: values.clone(),
            },
        }
    }
}

impl<'a> RuleKind<'a> {
    /// Returns the inner `rule` for wrapper variants that transparently delegate
    /// to a single child (`Named`, `Interpretation`, `Relation`, `Optional`,
    /// `Repeatable`, `RepeatableOptional`, `MinBounded`, `MaxBounded`,
    /// `MinMaxBounded`, `Pipeline`).
    ///
    /// Returns `None` for `Base`, `Or`, `Forward`, and `Empty`.
    pub(crate) fn transparent_inner_rule(&self) -> Option<&Arc<Rule<'a>>> {
        match self {
            Self::Named { rule, .. }
            | Self::Interpretation { rule, .. }
            | Self::Relation { rule, .. }
            | Self::Optional { rule, .. }
            | Self::Repeatable { rule, .. }
            | Self::RepeatableOptional { rule, .. }
            | Self::MinBounded { rule, .. }
            | Self::MaxBounded { rule, .. }
            | Self::MinMaxBounded { rule, .. }
            | Self::Pipeline { rule, .. } => Some(rule),
            Self::Base { .. } | Self::Or { .. } | Self::Forward { .. } | Self::Empty => None,
        }
    }
}

impl<'a> std::fmt::Debug for RuleKind<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Base { productions } => f
                .debug_struct("Base")
                .field("productions", productions)
                .finish(),
            Self::Or { rules } => f.debug_struct("Or").field("rules", rules).finish(),
            Self::Optional { rule, reverse } => f
                .debug_struct("Optional")
                .field("rule", rule)
                .field("reverse", reverse)
                .finish(),
            Self::Repeatable { rule, reverse } => f
                .debug_struct("Repeatable")
                .field("rule", rule)
                .field("reverse", reverse)
                .finish(),
            Self::RepeatableOptional {
                rule,
                reverse_repeatable,
                reverse_optional,
            } => f
                .debug_struct("RepeatableOptional")
                .field("rule", rule)
                .field("reverse_repeatable", reverse_repeatable)
                .field("reverse_optional", reverse_optional)
                .finish(),
            Self::MinBounded { rule, min, reverse } => f
                .debug_struct("MinBounded")
                .field("rule", rule)
                .field("min", min)
                .field("reverse", reverse)
                .finish(),
            Self::MaxBounded { rule, max, reverse } => f
                .debug_struct("MaxBounded")
                .field("rule", rule)
                .field("max", max)
                .field("reverse", reverse)
                .finish(),
            Self::MinMaxBounded {
                rule,
                min,
                max,
                reverse,
            } => f
                .debug_struct("MinMaxBounded")
                .field("rule", rule)
                .field("min", min)
                .field("max", max)
                .field("reverse", reverse)
                .finish(),
            Self::Named {
                rule,
                name,
                fact_scheme,
            } => f
                .debug_struct("Named")
                .field("rule", rule)
                .field("name", name)
                .field("fact_scheme", fact_scheme)
                .finish(),
            Self::Interpretation {
                rule,
                interpretation,
            } => f
                .debug_struct("Interpretation")
                .field("rule", rule)
                .field("interpretation", interpretation)
                .finish(),
            Self::Relation { rule, relation } => f
                .debug_struct("Relation")
                .field("rule", rule)
                .field("relation", &relation.label())
                .finish(),
            Self::Forward { inner } => f.debug_struct("Forward").field("inner", inner).finish(),
            Self::Empty => write!(f, "Empty"),
            Self::Pipeline {
                rule,
                pipeline,
                values,
            } => f
                .debug_struct("Pipeline")
                .field("rule", rule)
                .field("pipeline", pipeline)
                .field("values", values)
                .finish(),
        }
    }
}

/// Узел правила в графе грамматики.
#[derive(Debug, Clone)]
pub struct Rule<'a> {
    /// Конкретный вид правила.
    pub kind: RuleKind<'a>,
}

impl<'a> Rule<'a> {
    /// Создает базовое правило из продукций.
    pub fn new(productions: Vec<Production<'a>>) -> Self {
        Self {
            kind: RuleKind::Base { productions },
        }
    }

    /// Создает `or`-правило из нескольких альтернатив.
    pub fn or(rules: Vec<Arc<Rule<'a>>>) -> Self {
        Self {
            kind: RuleKind::Or { rules },
        }
    }

    /// Создает пустое правило.
    pub fn empty() -> Self {
        Self {
            kind: RuleKind::Empty,
        }
    }

    /// Создает forward-правило без целевого узла.
    pub fn forward() -> Arc<Self> {
        Arc::new(Self {
            kind: RuleKind::Forward {
                inner: parking_lot::RwLock::new(None),
            },
        })
    }

    /// Задает целевое правило для forward-узла.
    ///
    /// # Panics
    /// Паникует, если:
    /// - `this` не является forward-правилом;
    /// - `rule` само является forward-правилом.
    pub fn define_forward(this: &Arc<Self>, rule: Arc<Self>) {
        match &this.kind {
            RuleKind::Forward { inner } => {
                if matches!(rule.kind, RuleKind::Forward { .. }) {
                    panic!("forward(forward(...)) not allowed");
                }
                *inner.write() = Some(rule);
            }
            _ => panic!("define_forward called on non-forward rule"),
        }
    }

    /// Возвращает прямых детей текущего правила.
    pub fn children(&self) -> Vec<RuleChild<'_, 'a>> {
        match &self.kind {
            RuleKind::Base { productions } => {
                productions.iter().map(RuleChild::Production).collect()
            }
            RuleKind::Or { rules } => rules.iter().cloned().map(RuleChild::Rule).collect(),

            RuleKind::Optional { rule, .. }
            | RuleKind::Repeatable { rule, .. }
            | RuleKind::MinBounded { rule, .. }
            | RuleKind::MaxBounded { rule, .. }
            | RuleKind::MinMaxBounded { rule, .. }
            | RuleKind::Named { rule, .. }
            | RuleKind::Interpretation { rule, .. }
            | RuleKind::Relation { rule, .. }
            | RuleKind::Pipeline { rule, .. } => vec![RuleChild::Rule(rule.clone())],

            RuleKind::RepeatableOptional { rule, .. } => vec![RuleChild::Rule(rule.clone())],

            RuleKind::Forward { inner } => inner
                .read()
                .clone()
                .map(|r| vec![RuleChild::Rule(r)])
                .unwrap_or_default(),

            RuleKind::Empty => vec![],
        }
    }

    /// Возвращает опциональную обертку вокруг текущего правила.
    pub fn optional(self: &Arc<Self>, reverse: bool) -> Arc<Self> {
        Arc::new(Self {
            kind: RuleKind::Optional {
                rule: self.clone(),
                reverse,
            },
        })
    }

    /// Возвращает одну из repeatable-оберток с учетом границ.
    ///
    /// Соответствие аргументов:
    /// - `(None, None)` -> [`RuleKind::Repeatable`]
    /// - `(Some(min), None)` -> [`RuleKind::MinBounded`]
    /// - `(None, Some(max))` -> [`RuleKind::MaxBounded`]
    /// - `(Some(min), Some(max))` -> [`RuleKind::MinMaxBounded`]
    ///
    /// # Panics
    /// Паникует, если:
    /// - `min == 0`;
    /// - `max == 0`;
    /// - `max < min`.
    pub fn repeatable(
        self: &Arc<Self>,
        min: Option<usize>,
        max: Option<usize>,
        reverse: bool,
    ) -> Arc<Self> {
        match (min, max) {
            (Some(min), Some(max)) => {
                assert!(min >= 1);
                assert!(max >= min);
                Arc::new(Self {
                    kind: RuleKind::MinMaxBounded {
                        rule: self.clone(),
                        min,
                        max,
                        reverse,
                    },
                })
            }
            (Some(min), None) => {
                assert!(min >= 1);
                Arc::new(Self {
                    kind: RuleKind::MinBounded {
                        rule: self.clone(),
                        min,
                        reverse,
                    },
                })
            }
            (None, Some(max)) => {
                assert!(max >= 1);
                Arc::new(Self {
                    kind: RuleKind::MaxBounded {
                        rule: self.clone(),
                        max,
                        reverse,
                    },
                })
            }
            (None, None) => Arc::new(Self {
                kind: RuleKind::Repeatable {
                    rule: self.clone(),
                    reverse,
                },
            }),
        }
    }

    /// Возвращает именованную обертку вокруг правила.
    pub fn named(self: &Arc<Self>, name: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            kind: RuleKind::Named {
                rule: self.clone(),
                name: name.into(),
                fact_scheme: None,
            },
        })
    }

    /// Возвращает fact-обёртку вокруг правила вместе с полной схемой факта.
    pub fn named_fact(self: &Arc<Self>, scheme: FactScheme) -> Arc<Self> {
        Arc::new(Self {
            kind: RuleKind::Named {
                rule: self.clone(),
                name: scheme.name.clone(),
                fact_scheme: Some(Arc::new(scheme)),
            },
        })
    }

    /// Возвращает pipeline-обертку вокруг правила вместе с каноническими key продукций.
    pub fn pipeline(
        self: &Arc<Self>,
        pipeline: impl Into<String>,
        values: impl Into<Vec<String>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            kind: RuleKind::Pipeline {
                rule: self.clone(),
                pipeline: pipeline.into(),
                // values[production_index] используется как yargy-подобная каноническая нормальная форма.
                values: values.into().into(),
            },
        })
    }

    /// Возвращает короткую строковую метку вида правила.
    pub fn label(&self) -> String {
        match &self.kind {
            RuleKind::Named { name, .. } => name.clone(),
            RuleKind::Empty => "e".into(),
            RuleKind::Forward { .. } => "forward(...)".into(),
            RuleKind::Pipeline { pipeline, .. } => format!("pipeline({})", pipeline),
            RuleKind::Or { .. } => "or".into(),
            RuleKind::Base { .. } => "rule".into(),
            RuleKind::Optional { .. } => "optional".into(),
            RuleKind::Repeatable { .. } => "repeatable".into(),
            RuleKind::MinBounded { min, .. } => format!("repeat(min={})", min),
            RuleKind::MaxBounded { max, .. } => format!("repeat(max={})", max),
            RuleKind::MinMaxBounded { min, max, .. } => format!("repeat(min={},max={})", min, max),
            RuleKind::RepeatableOptional { .. } => "repeatable_optional".into(),
            RuleKind::Interpretation { .. } => "interpretation".into(),
            RuleKind::Relation { .. } => "relation".into(),
        }
    }

    /// Выполняет BFS-обход графа правил от текущего узла.
    pub fn walk_bfs(self: &Arc<Self>) -> Vec<Arc<Self>> {
        bfs_rule(self.clone())
    }

    /// Применяет стандартный пайплайн нормализации к правилу.
    pub fn normalized(self: &Arc<Self>) -> Arc<Self> {
        let mut t1 = SquashExtendedTransformator::new();
        let mut t2 = ReplaceExtendedTransformator::new();
        let mut t3 = ReplaceOrTransformator::new();
        let mut t4 = ReplaceEmptyTransformator::new();
        let mut t5 = FlattenTransformator::new();

        let r = t1.apply(self.clone());
        let r = t2.apply(r);
        let r = t3.apply(r);
        let r = t4.apply(r);
        let r = t5.apply(r);
        r
    }

    /// Возвращает BNF-представление правила.
    ///
    /// Ожидается, что граф уже нормализован (через [`Rule::normalized`]).
    pub fn bnf(self: &Arc<Self>) -> crate::rule::bnf::Bnf<'a> {
        let mut t = crate::rule::bnf::BnfTransformator::new();
        t.transform(self.clone())
    }

    /// Возвращает многострочную BNF-строку.
    pub fn as_bnf(self: &Arc<Self>) -> String {
        self.bnf().as_string()
    }
}

/// Тип дочернего элемента, возвращаемого [`Rule::children`].
pub enum RuleChild<'r, 'a> {
    /// Дочернее правило.
    Rule(Arc<Rule<'a>>),
    /// Дочерняя продукция базового правила.
    Production(&'r Production<'a>),
}

/// Выполняет BFS-обход графа правил, возвращая уникальные узлы.
///
/// Обход учитывает:
/// - прямых детей из [`Rule::children`];
/// - вложенные правила в термах продукций.
///
/// Уникальность определяется по идентичности `Arc`-указателей.
pub fn bfs_rule<'a>(root: Arc<Rule<'a>>) -> Vec<Arc<Rule<'a>>> {
    let mut out = Vec::new();
    let mut q = VecDeque::new();
    let mut visited: HashSet<usize> = HashSet::new();

    let root_id = Arc::as_ptr(&root) as usize;
    visited.insert(root_id);
    q.push_back(root);

    while let Some(item) = q.pop_front() {
        out.push(item.clone());

        for child in item.children() {
            match child {
                RuleChild::Rule(r) => {
                    let id = Arc::as_ptr(&r) as usize;
                    if visited.insert(id) {
                        q.push_back(r);
                    }
                }
                RuleChild::Production(p) => {
                    for term in p.children() {
                        if let Term::Rule(r) = term {
                            let id = Arc::as_ptr(r) as usize;
                            if visited.insert(id) {
                                q.push_back(r.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicates::constructors::eq;

    fn pred_rule<'a>(value: &'a str) -> Arc<Rule<'a>> {
        Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(eq(value))],
            None,
        )]))
    }

    #[test]
    fn production_detects_main_marker_and_formats_yargy_string() {
        let p = Production::new(
            vec![
                TermOrMain::from(eq("a")),
                TermOrMain::from(Main::new(eq("b"))),
                TermOrMain::from(eq("c")),
            ],
            None,
        );

        assert_eq!(p.main, 1);
        assert_eq!(p.to_string_yargy(), "eq(a) ^eq(b) eq(c)");
    }

    #[test]
    fn production_main_override_out_of_bounds_falls_back_to_zero() {
        let p = Production::new(
            vec![TermOrMain::from(eq("a")), TermOrMain::from(eq("b"))],
            Some(10),
        );
        assert_eq!(p.main, 0);
    }

    #[test]
    #[should_panic(expected = ">1 main")]
    fn production_panics_when_multiple_main_terms_provided() {
        let _ = Production::new(
            vec![
                TermOrMain::from(Main::new(eq("a"))),
                TermOrMain::from(Main::new(eq("b"))),
            ],
            None,
        );
    }

    #[test]
    fn empty_production_has_empty_terms_and_zero_main() {
        let p = EmptyProduction::as_production();
        assert!(p.terms.is_empty());
        assert_eq!(p.main, 0);
    }

    #[test]
    fn define_forward_sets_target_rule() {
        let fwd = Rule::forward();
        let target = pred_rule("x");
        Rule::define_forward(&fwd, target.clone());

        let children = fwd.children();
        assert_eq!(children.len(), 1);
        match &children[0] {
            RuleChild::Rule(rule) => assert!(Arc::ptr_eq(rule, &target)),
            RuleChild::Production(_) => panic!("expected forward child to be a rule"),
        }
    }

    #[test]
    #[should_panic(expected = "define_forward called on non-forward rule")]
    fn define_forward_panics_on_non_forward_rule() {
        let non_forward = pred_rule("x");
        let target = pred_rule("y");
        Rule::define_forward(&non_forward, target);
    }

    #[test]
    #[should_panic(expected = "forward(forward(...)) not allowed")]
    fn define_forward_panics_when_target_is_forward() {
        let fwd = Rule::forward();
        let fwd_target = Rule::forward();
        Rule::define_forward(&fwd, fwd_target);
    }

    #[test]
    fn repeatable_builds_expected_variants() {
        let base = pred_rule("x");

        let unbounded = base.repeatable(None, None, false);
        assert!(matches!(
            unbounded.kind,
            RuleKind::Repeatable { reverse: false, .. }
        ));

        let min_bounded = base.repeatable(Some(2), None, true);
        assert!(matches!(
            min_bounded.kind,
            RuleKind::MinBounded {
                min: 2,
                reverse: true,
                ..
            }
        ));

        let max_bounded = base.repeatable(None, Some(3), false);
        assert!(matches!(
            max_bounded.kind,
            RuleKind::MaxBounded {
                max: 3,
                reverse: false,
                ..
            }
        ));

        let min_max_bounded = base.repeatable(Some(2), Some(4), true);
        assert!(matches!(
            min_max_bounded.kind,
            RuleKind::MinMaxBounded {
                min: 2,
                max: 4,
                reverse: true,
                ..
            }
        ));
    }

    #[test]
    #[should_panic]
    fn repeatable_panics_when_min_is_zero() {
        let base = pred_rule("x");
        let _ = base.repeatable(Some(0), None, false);
    }

    #[test]
    #[should_panic]
    fn repeatable_panics_when_max_is_zero() {
        let base = pred_rule("x");
        let _ = base.repeatable(None, Some(0), false);
    }

    #[test]
    #[should_panic]
    fn repeatable_panics_when_max_less_than_min() {
        let base = pred_rule("x");
        let _ = base.repeatable(Some(3), Some(2), false);
    }

    #[test]
    fn label_reflects_rule_kind() {
        let base = pred_rule("x");
        assert_eq!(base.label(), "rule");
        assert_eq!(base.named("X").label(), "X");
        assert_eq!(base.optional(false).label(), "optional");
        assert_eq!(base.repeatable(None, None, false).label(), "repeatable");
        assert_eq!(
            base.repeatable(Some(2), None, false).label(),
            "repeat(min=2)"
        );
        assert_eq!(
            base.repeatable(None, Some(3), false).label(),
            "repeat(max=3)"
        );
        assert_eq!(
            base.repeatable(Some(2), Some(3), false).label(),
            "repeat(min=2,max=3)"
        );
        assert_eq!(Rule::empty().label(), "e");
        assert_eq!(Rule::forward().label(), "forward(...)");
        assert_eq!(
            pred_rule("x")
                .pipeline("demo", vec!["KEY".to_string()])
                .label(),
            "pipeline(demo)"
        );
    }

    #[test]
    fn bfs_rule_returns_unique_nodes_for_shared_children() {
        let child = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![
                TermOrMain::from(child.clone()),
                TermOrMain::from(child.clone()),
            ],
            None,
        )]));

        let walked = bfs_rule(root);
        assert_eq!(walked.len(), 2);
    }

    #[test]
    fn as_bnf_returns_multiline_string() {
        let child = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(child)],
            None,
        )]));

        assert_eq!(root.as_bnf(), "R0 -> R1\nR1 -> 'x'");
    }
}
