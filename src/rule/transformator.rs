//! Набор трансформаторов для нормализации графа правил.
//!
//! Трансформаторы применяются последовательно в `Rule::normalized` и
//! приводят расширенные конструкции (`optional`, `repeatable`, `or`, `empty`)
//! к базовому представлению, удобному для дальнейших преобразований.

use std::collections::HashMap;
use std::sync::Arc;

use crate::rule::constructors::{EmptyProduction, Production, Rule, RuleKind, Term, TermOrMain};

/// Общий интерфейс трансформатора графа правил.
pub trait Transformator<'a> {
    /// Применяет преобразование к корневому правилу и возвращает новый корень.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>>;
}

/// Мемоизация для трансформаторов, чтобы не обрабатывать один и тот же Rule несколько раз.
struct Memo<'a> {
    visited: HashMap<usize, Arc<Rule<'a>>>,
}

impl<'a> Memo<'a> {
    /// Создает пустой кеш посещенных узлов.
    fn new() -> Self {
        Self {
            visited: HashMap::new(),
        }
    }

    #[inline]
    /// Вычисляет ключ по адресу `Arc`.
    fn key(rule: &Arc<Rule<'a>>) -> usize {
        Arc::as_ptr(rule) as usize
    }

    /// Возвращает ранее вычисленное отображение для правила.
    fn get(&self, rule: &Arc<Rule<'a>>) -> Option<Arc<Rule<'a>>> {
        self.visited.get(&Self::key(rule)).cloned()
    }

    /// Сохраняет соответствие оригинального правила и результата трансформации.
    fn put(&mut self, original: &Arc<Rule<'a>>, mapped: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        self.visited.insert(Self::key(original), mapped.clone());
        mapped
    }
}

#[inline]
/// Вспомогательный конструктор правила из одной продукции.
fn rule1<'a>(items: Vec<TermOrMain<'a>>) -> Arc<Rule<'a>> {
    Arc::new(Rule::new(vec![Production::new(items, None)]))
}

#[inline]
/// Вспомогательный конструктор `or`-правила.
fn or_rule<'a>(rules: Vec<Arc<Rule<'a>>>) -> Arc<Rule<'a>> {
    Arc::new(Rule::or(rules))
}

#[inline]
/// Вспомогательный конструктор пустого правила.
fn empty_rule<'a>() -> Arc<Rule<'a>> {
    Arc::new(Rule::empty())
}

/// Строит верхне-ограниченное повторение.
///
/// # Panics
/// Паникует, если `count < 1`.
fn max_bound<'a>(item: Arc<Rule<'a>>, count: usize, reverse: bool) -> Arc<Rule<'a>> {
    assert!(count >= 1);
    if count == 1 {
        item
    } else {
        let a = rule1(vec![
            TermOrMain::from(item.clone()),
            TermOrMain::from(max_bound(item.clone(), count - 1, reverse)),
        ]);
        let b = item;

        let (a, b) = if reverse { (b, a) } else { (a, b) };
        or_rule(vec![a, b])
    }
}

/// Строит рекурсивное repeatable-правило через `Forward`.
fn repeatable<'a>(item: Arc<Rule<'a>>, reverse: bool) -> Arc<Rule<'a>> {
    let temp = Rule::forward();
    let a = rule1(vec![
        TermOrMain::from(item.clone()),
        TermOrMain::from(temp.clone()),
    ]);
    let b = item;

    let (a, b) = if reverse { (b, a) } else { (a, b) };
    Rule::define_forward(&temp, or_rule(vec![a, b]));
    temp
}

/// Строит optional-правило как `or(empty, item)` с учетом порядка.
fn optional<'a>(item: Arc<Rule<'a>>, reverse: bool) -> Arc<Rule<'a>> {
    let a = empty_rule();
    let b = item;

    let (a, b) = if reverse { (b, a) } else { (a, b) };
    or_rule(vec![a, b])
}

/// Строит combined-вариант repeatable+optional через `Forward`.
fn repeatable_optional<'a>(
    item: Arc<Rule<'a>>,
    reverse_repeatable: bool,
    reverse_optional: bool,
) -> Arc<Rule<'a>> {
    let temp = Rule::forward();
    let mut a = empty_rule();
    let mut b = rule1(vec![
        TermOrMain::from(item.clone()),
        TermOrMain::from(temp.clone()),
    ]);
    let mut c = item;

    if reverse_repeatable {
        std::mem::swap(&mut b, &mut c);
    }
    if reverse_optional {
        // a,b,c = b,c,a
        let old_a = a;
        a = b;
        b = c;
        c = old_a;
    }

    Rule::define_forward(&temp, or_rule(vec![a, b, c]));
    temp
}

#[inline]
/// Возвращает `count` клонов правила.
fn repeat<'a>(item: Arc<Rule<'a>>, count: usize) -> Vec<Arc<Rule<'a>>> {
    (0..count).map(|_| item.clone()).collect()
}

/// Обновляет внутренние ссылки `Forward`, применяя функцию `visit`.
fn update_forwards<'a, F>(root: &Arc<Rule<'a>>, mut visit: F)
where
    F: FnMut(Arc<Rule<'a>>) -> Arc<Rule<'a>>,
{
    // Сначала обходим весь граф, затем перепривязываем inner у Forward к уже
    // трансформированным узлам, чтобы не оставить "смешанные" ссылки old/new.
    for node in root.walk_bfs() {
        if let RuleKind::Forward { inner } = &node.kind {
            let current = inner.read().clone();
            if let Some(old_inner) = current {
                let new_inner = visit(old_inner);
                Rule::define_forward(&node, new_inner);
            }
        }
    }
}

/// Стандартная пересборка правила с рекурсивным обходом дочерних узлов.
fn rebuild_default<'a, F>(item: Arc<Rule<'a>>, mut visit_rule: F) -> Arc<Rule<'a>>
where
    F: FnMut(Arc<Rule<'a>>) -> Arc<Rule<'a>>,
{
    match &item.kind {
        RuleKind::Base { productions } => {
            // Глубокая перестройка Base: каждый вложенный Rule-терм рекурсивно
            // прогоняется через текущий transformator, Pred-термы копируются как есть.
            let new_prods = productions
                .iter()
                .map(|p| {
                    let new_terms = p
                        .terms
                        .iter()
                        .cloned()
                        .map(|t| match t {
                            Term::Rule(r) => Term::Rule(visit_rule(r)),
                            Term::Pred(p) => Term::Pred(p),
                        })
                        .collect();
                    Production {
                        terms: new_terms,
                        main: p.main,
                    }
                })
                .collect();

            Arc::new(Rule {
                kind: RuleKind::Base {
                    productions: new_prods,
                },
            })
        }

        RuleKind::Or { rules } => Arc::new(Rule {
            kind: RuleKind::Or {
                rules: rules.iter().cloned().map(&mut visit_rule).collect(),
            },
        }),

        RuleKind::Optional { rule, reverse } => Arc::new(Rule {
            kind: RuleKind::Optional {
                rule: visit_rule(rule.clone()),
                reverse: *reverse,
            },
        }),

        RuleKind::Repeatable { rule, reverse } => Arc::new(Rule {
            kind: RuleKind::Repeatable {
                rule: visit_rule(rule.clone()),
                reverse: *reverse,
            },
        }),

        RuleKind::RepeatableOptional {
            rule,
            reverse_repeatable,
            reverse_optional,
        } => Arc::new(Rule {
            kind: RuleKind::RepeatableOptional {
                rule: visit_rule(rule.clone()),
                reverse_repeatable: *reverse_repeatable,
                reverse_optional: *reverse_optional,
            },
        }),

        RuleKind::MinBounded { rule, min, reverse } => Arc::new(Rule {
            kind: RuleKind::MinBounded {
                rule: visit_rule(rule.clone()),
                min: *min,
                reverse: *reverse,
            },
        }),

        RuleKind::MaxBounded { rule, max, reverse } => Arc::new(Rule {
            kind: RuleKind::MaxBounded {
                rule: visit_rule(rule.clone()),
                max: *max,
                reverse: *reverse,
            },
        }),

        RuleKind::MinMaxBounded {
            rule,
            min,
            max,
            reverse,
        } => Arc::new(Rule {
            kind: RuleKind::MinMaxBounded {
                rule: visit_rule(rule.clone()),
                min: *min,
                max: *max,
                reverse: *reverse,
            },
        }),

        RuleKind::Named {
            rule,
            name,
            fact_scheme,
        } => Arc::new(Rule {
            kind: RuleKind::Named {
                rule: visit_rule(rule.clone()),
                name: name.clone(),
                fact_scheme: fact_scheme.clone(),
            },
        }),

        RuleKind::Interpretation {
            rule,
            interpretation,
        } => Arc::new(Rule {
            kind: RuleKind::Interpretation {
                rule: visit_rule(rule.clone()),
                interpretation: interpretation.clone(),
            },
        }),

        RuleKind::Relation { rule, relation } => Arc::new(Rule {
            kind: RuleKind::Relation {
                rule: visit_rule(rule.clone()),
                relation: relation.clone(),
            },
        }),

        RuleKind::Pipeline {
            rule,
            pipeline,
            values,
        } => Arc::new(Rule {
            kind: RuleKind::Pipeline {
                // Сохраняем wrapper, но рекурсивно пересобираем внутреннее правило.
                rule: visit_rule(rule.clone()),
                pipeline: pipeline.clone(),
                values: values.clone(),
            },
        }),

        RuleKind::Forward { .. } | RuleKind::Empty => item,
    }
}

/// Схлопывает расширенные композиции (`optional/repeatable`) в минимальные формы.
pub struct SquashExtendedTransformator<'a> {
    memo: Memo<'a>,
}

impl<'a> SquashExtendedTransformator<'a> {
    /// Создает трансформатор.
    pub fn new() -> Self {
        Self { memo: Memo::new() }
    }

    /// Рекурсивно обходит граф и схлопывает эквивалентные extended-конструкции.
    fn visit(&mut self, item: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        // Мемоизация по identity Arc защищает от повторной обработки общих подграфов
        // и разрывает потенциальные циклы рекурсии.
        if let Some(cached) = self.memo.get(&item) {
            return cached;
        }

        // Локальные правила схлопывания + рекурсивный fallback через visit/rebuild_default.
        let out = match &item.kind {
            RuleKind::Repeatable {
                rule: child,
                reverse,
            } => match &child.kind {
                RuleKind::Optional {
                    rule: inner,
                    reverse: opt_rev,
                } => {
                    // repeatable(optional(x)) => repeatable_optional(x)
                    self.visit(Arc::new(Rule {
                        kind: RuleKind::RepeatableOptional {
                            rule: inner.clone(),
                            reverse_repeatable: *reverse,
                            reverse_optional: *opt_rev,
                        },
                    }))
                }
                RuleKind::RepeatableOptional {
                    rule: inner,
                    reverse_optional: ro,
                    ..
                } => self.visit(Arc::new(Rule {
                    kind: RuleKind::RepeatableOptional {
                        rule: inner.clone(),
                        reverse_repeatable: *reverse,
                        reverse_optional: *ro,
                    },
                })),
                RuleKind::Repeatable { rule: inner, .. }
                | RuleKind::MinBounded { rule: inner, .. }
                | RuleKind::MaxBounded { rule: inner, .. }
                | RuleKind::MinMaxBounded { rule: inner, .. } => Arc::new(Rule {
                    kind: RuleKind::Repeatable {
                        rule: inner.clone(),
                        reverse: *reverse,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::Repeatable {
                        rule: self.visit(child.clone()),
                        reverse: *reverse,
                    },
                }),
            },

            RuleKind::Optional {
                rule: child,
                reverse,
            } => match &child.kind {
                RuleKind::Repeatable {
                    rule: inner,
                    reverse: rep_rev,
                } => self.visit(Arc::new(Rule {
                    kind: RuleKind::RepeatableOptional {
                        rule: inner.clone(),
                        reverse_repeatable: *rep_rev,
                        reverse_optional: *reverse,
                    },
                })),
                RuleKind::RepeatableOptional {
                    rule: inner,
                    reverse_repeatable: rr,
                    ..
                } => self.visit(Arc::new(Rule {
                    kind: RuleKind::RepeatableOptional {
                        rule: inner.clone(),
                        reverse_repeatable: *rr,
                        reverse_optional: *reverse,
                    },
                })),
                RuleKind::Optional { rule: inner, .. } => Arc::new(Rule {
                    kind: RuleKind::Optional {
                        rule: inner.clone(),
                        reverse: *reverse,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::Optional {
                        rule: self.visit(child.clone()),
                        reverse: *reverse,
                    },
                }),
            },

            RuleKind::RepeatableOptional {
                rule: child,
                reverse_repeatable,
                reverse_optional,
            } => match &child.kind {
                RuleKind::Repeatable { rule: inner, .. }
                | RuleKind::MinBounded { rule: inner, .. }
                | RuleKind::MaxBounded { rule: inner, .. }
                | RuleKind::MinMaxBounded { rule: inner, .. }
                | RuleKind::Optional { rule: inner, .. }
                | RuleKind::RepeatableOptional { rule: inner, .. } => Arc::new(Rule {
                    kind: RuleKind::RepeatableOptional {
                        rule: inner.clone(),
                        reverse_repeatable: *reverse_repeatable,
                        reverse_optional: *reverse_optional,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::RepeatableOptional {
                        rule: self.visit(child.clone()),
                        reverse_repeatable: *reverse_repeatable,
                        reverse_optional: *reverse_optional,
                    },
                }),
            },

            RuleKind::MinBounded {
                rule: child,
                min,
                reverse,
            } => match &child.kind {
                RuleKind::Repeatable { .. } | RuleKind::RepeatableOptional { .. } => {
                    self.visit(child.clone())
                }
                RuleKind::Optional {
                    rule: inner,
                    reverse: opt_rev,
                } => Arc::new(Rule {
                    kind: RuleKind::Optional {
                        rule: Arc::new(Rule {
                            kind: RuleKind::MinBounded {
                                rule: inner.clone(),
                                min: *min,
                                reverse: *reverse,
                            },
                        }),
                        reverse: *opt_rev,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::MinBounded {
                        rule: self.visit(child.clone()),
                        min: *min,
                        reverse: *reverse,
                    },
                }),
            },

            RuleKind::MaxBounded {
                rule: child,
                max,
                reverse,
            } => match &child.kind {
                RuleKind::Repeatable { .. } | RuleKind::RepeatableOptional { .. } => {
                    self.visit(child.clone())
                }
                RuleKind::Optional {
                    rule: inner,
                    reverse: opt_rev,
                } => Arc::new(Rule {
                    kind: RuleKind::Optional {
                        rule: Arc::new(Rule {
                            kind: RuleKind::MaxBounded {
                                rule: inner.clone(),
                                max: *max,
                                reverse: *reverse,
                            },
                        }),
                        reverse: *opt_rev,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::MaxBounded {
                        rule: self.visit(child.clone()),
                        max: *max,
                        reverse: *reverse,
                    },
                }),
            },

            RuleKind::MinMaxBounded {
                rule: child,
                min,
                max,
                reverse,
            } => match &child.kind {
                RuleKind::Repeatable { .. } | RuleKind::RepeatableOptional { .. } => {
                    self.visit(child.clone())
                }
                RuleKind::Optional {
                    rule: inner,
                    reverse: opt_rev,
                } => Arc::new(Rule {
                    kind: RuleKind::Optional {
                        rule: Arc::new(Rule {
                            kind: RuleKind::MinMaxBounded {
                                rule: inner.clone(),
                                min: *min,
                                max: *max,
                                reverse: *reverse,
                            },
                        }),
                        reverse: *opt_rev,
                    },
                }),
                _ => Arc::new(Rule {
                    kind: RuleKind::MinMaxBounded {
                        rule: self.visit(child.clone()),
                        min: *min,
                        max: *max,
                        reverse: *reverse,
                    },
                }),
            },

            _ => rebuild_default(item.clone(), |r| self.visit(r)),
        };

        self.memo.put(&item, out)
    }
}

impl<'a> Default for SquashExtendedTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Transformator<'a> for SquashExtendedTransformator<'a> {
    /// Применяет схлопывание и синхронизирует `Forward`-внутренности.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        update_forwards(&root, |r| self.visit(r));
        self.visit(root)
    }
}

/// Заменяет расширенные конструкции на композицию базовых правил.
pub struct ReplaceExtendedTransformator<'a> {
    memo: Memo<'a>,
}

impl<'a> ReplaceExtendedTransformator<'a> {
    /// Создает трансформатор.
    pub fn new() -> Self {
        Self { memo: Memo::new() }
    }

    /// Рекурсивно заменяет extended-узлы на развёрнутую форму.
    ///
    /// # Panics
    /// Паникует при некорректных границах повторений:
    /// - `min == 0`
    /// - `max < min`
    fn visit(&mut self, item: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        // У shared-узлов важно переиспользовать уже построенный результат:
        // это сохраняет форму графа и ускоряет трансформацию.
        if let Some(cached) = self.memo.get(&item) {
            return cached;
        }

        // Здесь extended-узлы разворачиваются в композицию базовых правил;
        // остальные узлы проходят стандартную рекурсивную пересборку.
        let out = match &item.kind {
            RuleKind::Repeatable {
                rule: child,
                reverse,
            } => {
                let c = self.visit(child.clone());
                repeatable(c, *reverse)
            }
            RuleKind::Optional {
                rule: child,
                reverse,
            } => {
                let c = self.visit(child.clone());
                optional(c, *reverse)
            }
            RuleKind::RepeatableOptional {
                rule: child,
                reverse_repeatable,
                reverse_optional,
            } => {
                let c = self.visit(child.clone());
                repeatable_optional(c, *reverse_repeatable, *reverse_optional)
            }
            RuleKind::MinBounded {
                rule: child,
                min,
                reverse,
            } => {
                assert!(*min >= 1);
                let c = self.visit(child.clone());
                let mut items = repeat(c.clone(), min - 1);
                items.push(repeatable(c, *reverse));
                rule1(items.into_iter().map(TermOrMain::from).collect())
            }
            RuleKind::MaxBounded {
                rule: child,
                max,
                reverse,
            } => {
                let c = self.visit(child.clone());
                max_bound(c, *max, *reverse)
            }
            RuleKind::MinMaxBounded {
                rule: child,
                min,
                max,
                reverse,
            } => {
                assert!(*min >= 1);
                assert!(*max >= *min);
                let c = self.visit(child.clone());
                let mut items = repeat(c.clone(), min - 1);
                items.push(max_bound(c, max - min + 1, *reverse));
                rule1(items.into_iter().map(TermOrMain::from).collect())
            }
            _ => rebuild_default(item.clone(), |r| self.visit(r)),
        };

        self.memo.put(&item, out)
    }
}

impl<'a> Default for ReplaceExtendedTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Transformator<'a> for ReplaceExtendedTransformator<'a> {
    /// Применяет замену extended-узлов и синхронизирует `Forward`-внутренности.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        update_forwards(&root, |r| self.visit(r));
        self.visit(root)
    }
}

/// Преобразует `RuleKind::Or` в `RuleKind::Base` с отдельной продукцией на альтернативу.
pub struct ReplaceOrTransformator<'a> {
    memo: Memo<'a>,
}

impl<'a> ReplaceOrTransformator<'a> {
    /// Создает трансформатор.
    pub fn new() -> Self {
        Self { memo: Memo::new() }
    }

    /// Рекурсивно заменяет `Or`-узлы базовыми продукциями.
    fn visit(&mut self, item: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        if let Some(cached) = self.memo.get(&item) {
            return cached;
        }

        let out = match &item.kind {
            RuleKind::Or { rules } => {
                let prods = rules
                    .iter()
                    .cloned()
                    .map(|r| Production {
                        terms: vec![Term::Rule(self.visit(r))],
                        main: 0,
                    })
                    .collect();
                Arc::new(Rule {
                    kind: RuleKind::Base { productions: prods },
                })
            }
            _ => rebuild_default(item.clone(), |r| self.visit(r)),
        };

        self.memo.put(&item, out)
    }
}

impl<'a> Default for ReplaceOrTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Transformator<'a> for ReplaceOrTransformator<'a> {
    /// Применяет замену `Or` и синхронизирует `Forward`-внутренности.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        update_forwards(&root, |r| self.visit(r));
        self.visit(root)
    }
}

/// Заменяет `RuleKind::Empty` на базовое правило с epsilon-продукцией.
pub struct ReplaceEmptyTransformator<'a> {
    memo: Memo<'a>,
}

impl<'a> ReplaceEmptyTransformator<'a> {
    /// Создает трансформатор.
    pub fn new() -> Self {
        Self { memo: Memo::new() }
    }

    /// Рекурсивно заменяет пустые узлы на явную пустую продукцию.
    fn visit(&mut self, item: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        if let Some(cached) = self.memo.get(&item) {
            return cached;
        }

        let out = match &item.kind {
            RuleKind::Empty => Arc::new(Rule {
                kind: RuleKind::Base {
                    productions: vec![EmptyProduction::as_production()],
                },
            }),
            _ => rebuild_default(item.clone(), |r| self.visit(r)),
        };

        self.memo.put(&item, out)
    }
}

impl<'a> Default for ReplaceEmptyTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Transformator<'a> for ReplaceEmptyTransformator<'a> {
    /// Применяет замену `Empty` и синхронизирует `Forward`-внутренности.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        update_forwards(&root, |r| self.visit(r));
        self.visit(root)
    }
}

/// Уплощает вложенные одноэлементные базовые правила.
pub struct FlattenTransformator<'a> {
    memo: Memo<'a>,
}

impl<'a> FlattenTransformator<'a> {
    /// Создает трансформатор.
    pub fn new() -> Self {
        Self { memo: Memo::new() }
    }

    /// Рекурсивно упрощает вложенность терма, если он сводится к одному элементу.
    fn flatten_term(&mut self, term: Term<'a>) -> Term<'a> {
        match term {
            Term::Rule(r) => {
                // Схлопываем цепочку Base(1 prod, 1 term) -> ... до первого
                // "содержательного" терма, затем уже заходим в visit.
                if let RuleKind::Base { productions } = &r.kind {
                    if productions.len() == 1 {
                        let p0 = &productions[0];
                        if p0.terms.len() == 1 {
                            return self.flatten_term(p0.terms[0].clone());
                        }
                    }
                }
                Term::Rule(self.visit(r))
            }
            Term::Pred(p) => Term::Pred(p),
        }
    }

    /// Рекурсивно перестраивает правило и уплощает избыточные уровни вложенности.
    fn visit(&mut self, item: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        // Для DAG/графа с повторным использованием узлов нельзя повторно "расплющивать"
        // один и тот же Arc: memo держит единый результат.
        if let Some(cached) = self.memo.get(&item) {
            return cached;
        }

        let out = match &item.kind {
            RuleKind::Base { productions } => {
                // Flatten работает только на Base-продукциях; для остальных kind
                // используем общий рекурсивный rebuild_default ниже.
                let new_prods = productions
                    .iter()
                    .map(|p| {
                        //  Production([Rule]) где Rule имеет 1 production => заменить на ту production
                        if p.terms.len() == 1 {
                            if let Term::Rule(r) = &p.terms[0] {
                                if let RuleKind::Base { productions: inner } = &r.kind {
                                    if inner.len() == 1 {
                                        let inner_p = &inner[0];
                                        return Production {
                                            terms: inner_p
                                                .terms
                                                .iter()
                                                .cloned()
                                                .map(|t| {
                                                    let mapped = match t {
                                                        Term::Rule(rr) => {
                                                            let visited = self.visit(rr);
                                                            Term::Rule(visited)
                                                        }
                                                        Term::Pred(pp) => Term::Pred(pp),
                                                    };
                                                    self.flatten_term(mapped)
                                                })
                                                .collect(),
                                            main: inner_p.main,
                                        };
                                    }
                                }
                            }
                        }

                        // default: rebuild production + flatten terms
                        let mut np = Production {
                            terms: p
                                .terms
                                .iter()
                                .cloned()
                                .map(|t| match t {
                                    Term::Rule(r) => Term::Rule(self.visit(r)),
                                    Term::Pred(p) => Term::Pred(p),
                                })
                                .collect(),
                            main: p.main,
                        };

                        np.terms = np.terms.into_iter().map(|t| self.flatten_term(t)).collect();
                        np
                    })
                    .collect();

                Arc::new(Rule {
                    kind: RuleKind::Base {
                        productions: new_prods,
                    },
                })
            }

            _ => rebuild_default(item.clone(), |r| self.visit(r)),
        };

        self.memo.put(&item, out)
    }
}

impl<'a> Default for FlattenTransformator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Transformator<'a> for FlattenTransformator<'a> {
    /// Применяет уплощение и синхронизирует `Forward`-внутренности.
    fn apply(&mut self, root: Arc<Rule<'a>>) -> Arc<Rule<'a>> {
        update_forwards(&root, |r| self.visit(r));
        self.visit(root)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        FlattenTransformator, ReplaceEmptyTransformator, ReplaceExtendedTransformator,
        ReplaceOrTransformator, SquashExtendedTransformator, Transformator,
    };
    use crate::predicates::constructors::eq;
    use crate::rule::constructors::{
        EmptyProduction, Production, Rule, RuleKind, Term, TermOrMain,
    };

    fn pred_rule<'a>(value: &'a str) -> Arc<Rule<'a>> {
        Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(eq(value))],
            None,
        )]))
    }

    #[test]
    fn replace_or_turns_or_into_base_with_rule_terms() {
        let root = Arc::new(Rule::or(vec![pred_rule("a"), pred_rule("b")]));
        let mut t = ReplaceOrTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 2);
                assert!(productions
                    .iter()
                    .all(|p| p.terms.len() == 1 && matches!(p.terms[0], Term::Rule(_))));
            }
            _ => panic!("expected Base after ReplaceOrTransformator"),
        }
    }

    #[test]
    fn replace_empty_turns_empty_into_epsilon_base() {
        let root = Arc::new(Rule::empty());
        let mut t = ReplaceEmptyTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert!(productions[0].terms.is_empty());
                assert_eq!(productions[0].main, EmptyProduction::as_production().main);
            }
            _ => panic!("expected Base after ReplaceEmptyTransformator"),
        }
    }

    #[test]
    fn replace_extended_optional_builds_or_rule() {
        let root = Arc::new(Rule {
            kind: RuleKind::Optional {
                rule: pred_rule("x"),
                reverse: false,
            },
        });
        let mut t = ReplaceExtendedTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Or { rules } => {
                assert_eq!(rules.len(), 2);
                assert!(matches!(rules[0].kind, RuleKind::Empty));
            }
            _ => panic!("expected Or after ReplaceExtendedTransformator optional"),
        }
    }

    #[test]
    fn replace_extended_repeatable_builds_defined_forward() {
        let root = Arc::new(Rule {
            kind: RuleKind::Repeatable {
                rule: pred_rule("x"),
                reverse: false,
            },
        });
        let mut t = ReplaceExtendedTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Forward { inner } => {
                let inner = inner.read().clone().expect("forward must be defined");
                assert!(matches!(inner.kind, RuleKind::Or { .. }));
            }
            _ => panic!("expected Forward after ReplaceExtendedTransformator repeatable"),
        }
    }

    #[test]
    fn replace_extended_updates_forward_inner_during_apply() {
        let fwd = Rule::forward();
        let optional = Arc::new(Rule {
            kind: RuleKind::Optional {
                rule: pred_rule("x"),
                reverse: false,
            },
        });
        Rule::define_forward(&fwd, optional);

        let mut t = ReplaceExtendedTransformator::new();
        let out = t.apply(fwd.clone());
        assert!(matches!(out.kind, RuleKind::Forward { .. }));

        match &fwd.kind {
            RuleKind::Forward { inner } => {
                let mapped_inner = inner
                    .read()
                    .clone()
                    .expect("forward inner should stay defined");
                assert!(matches!(mapped_inner.kind, RuleKind::Or { .. }));
            }
            _ => panic!("expected forward root"),
        }
    }

    #[test]
    fn squash_repeatable_optional_collapse() {
        let inner = pred_rule("x");
        let nested = Arc::new(Rule {
            kind: RuleKind::Optional {
                rule: inner.clone(),
                reverse: true,
            },
        });
        let root = Arc::new(Rule {
            kind: RuleKind::Repeatable {
                rule: nested,
                reverse: false,
            },
        });

        let mut t = SquashExtendedTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::RepeatableOptional {
                reverse_repeatable,
                reverse_optional,
                ..
            } => {
                assert!(!reverse_repeatable);
                assert!(*reverse_optional);
            }
            _ => panic!("expected RepeatableOptional after squash"),
        }
    }

    #[test]
    fn squash_optional_repeatable_collapse() {
        let inner = pred_rule("x");
        let nested = Arc::new(Rule {
            kind: RuleKind::Repeatable {
                rule: inner.clone(),
                reverse: true,
            },
        });
        let root = Arc::new(Rule {
            kind: RuleKind::Optional {
                rule: nested,
                reverse: false,
            },
        });

        let mut t = SquashExtendedTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::RepeatableOptional {
                reverse_repeatable,
                reverse_optional,
                ..
            } => {
                assert!(*reverse_repeatable);
                assert!(!reverse_optional);
            }
            _ => panic!("expected RepeatableOptional after squash"),
        }
    }

    #[test]
    fn flatten_unwraps_single_nested_base_term() {
        let inner = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(inner)],
            None,
        )]));

        let mut t = FlattenTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert_eq!(productions[0].terms.len(), 1);
                assert!(matches!(productions[0].terms[0], Term::Pred(_)));
            }
            _ => panic!("expected Base after flatten"),
        }
    }

    #[test]
    fn flatten_preserves_production_arity() {
        let inner = pred_rule("x");
        let root = Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(inner), TermOrMain::from(eq("y"))],
            None,
        )]));

        let mut t = FlattenTransformator::new();
        let out = t.apply(root);

        match &out.kind {
            RuleKind::Base { productions } => {
                assert_eq!(productions.len(), 1);
                assert_eq!(productions[0].terms.len(), 2);
                let labels: Vec<String> = productions[0].terms.iter().map(|t| t.label()).collect();
                assert_eq!(labels, vec!["eq(x)".to_string(), "eq(y)".to_string()]);
            }
            _ => panic!("expected Base after flatten"),
        }
    }
}
