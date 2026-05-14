//! Ядро парсера на алгоритме Earley.
//!
//! Модуль отвечает за построение чарта и выполнение шагов Earley
//! (`predict` / `scan` / `complete`), а также за высокоуровневый API
//! поиска совпадений.
//!
//! Токенизация и тэггинг вынесены в [`super::tokenizer`].
//!
//! Поддерживаются два режима входа:
//! - текст (`find`/`findall`/`match`/`extract`);
//! - готовые токены (`parse`, `find`/`findall` для `&[Token]`).

use std::collections::HashSet;
use std::sync::Arc;

use super::chart::Chart;
use super::find_input::FindInput;
use super::item::State;
use super::match_result::{
    prepare_matches, prepare_resolved_matches, prepare_trees, Match, MatchBorrowed,
};
use super::tokenizer::{anytoken_to_token, ParserTokenizer, PassTagger, Tagger};
use crate::tree::{Leaf, Node, ParseChild, Tree};

use crate::predicates::constructors::{PredicateKind, TokenView};
use crate::rule::builder::RuleId;
use crate::rule::constructors::Term;
use crate::rule::registry::RuleRegistry;
use crate::token::{global_morph_tokenizer, AnyToken, AnyTokenOwnedLite, Token};

struct PredictMemo {
    /// `predicted[col] = {rule_id, ...}` уже предсказанные в колонке правила.
    predicted: Vec<HashSet<RuleId>>,
}

impl PredictMemo {
    /// Создает memo-структуру для указанного количества колонок.
    fn new(cols: usize) -> Self {
        Self {
            predicted: (0..cols).map(|_| HashSet::new()).collect(),
        }
    }

    /// Возвращает `true`, если правило еще не предсказывали в этой колонке.
    ///
    /// Используется как защита от повторного `predict` одного и того же `rule_id`
    /// в одной колонке.
    #[inline]
    fn should_predict(&mut self, col_idx: usize, rule_id: RuleId) -> bool {
        self.predicted[col_idx].insert(rule_id)
    }
}

/// Основной объект парсера.
///
/// Содержит:
/// - реестр правил;
/// - корневое правило;
/// - токенизатор;
/// - тэггер.
pub struct Parser<'r> {
    registry: &'r RuleRegistry<'r>,
    root_id: RuleId,
    tokenizer: Arc<dyn ParserTokenizer>,
    tagger: Arc<dyn Tagger>,
}

impl<'r> Parser<'r> {
    /// Создает парсер с глобальным morph-токенизатором и `PassTagger`.
    pub fn new(registry: &'r RuleRegistry<'r>, root_id: RuleId) -> Self {
        Self {
            registry,
            root_id,
            tokenizer: global_morph_tokenizer(),
            tagger: Arc::new(PassTagger),
        }
    }

    /// Создает парсер с пользовательским токенизатором и `PassTagger`.
    pub fn with_tokenizer(
        registry: &'r RuleRegistry<'r>,
        root_id: RuleId,
        tokenizer: Arc<dyn ParserTokenizer>,
    ) -> Self {
        Self {
            registry,
            root_id,
            tokenizer,
            tagger: Arc::new(PassTagger),
        }
    }

    /// Создает парсер с пользовательскими токенизатором и тэггером.
    pub fn with_tokenizer_and_tagger(
        registry: &'r RuleRegistry<'r>,
        root_id: RuleId,
        tokenizer: Arc<dyn ParserTokenizer>,
        tagger: Arc<dyn Tagger>,
    ) -> Self {
        Self {
            registry,
            root_id,
            tokenizer,
            tagger,
        }
    }

    /// Строит чарт для текста.
    ///
    /// Если `all = true`, парсер стартует разбор из каждой позиции, а не только из начала.
    pub fn chart<'t>(&self, text: &'t str, all: bool) -> Chart<'t> {
        let toks_any = self.tagger.tag(self.tokenizer.tokenize(text));
        self.build_chart_any(&toks_any, all, true)
    }

    /// Возвращает итоговые состояния (`State`) после разбора текста.
    pub fn matches(&self, text: &str, all: bool) -> Vec<State> {
        let toks_any = self.tagger.tag(self.tokenizer.tokenize(text));
        let chart = self.build_chart_any(&toks_any, all, false);
        self.collect_states(&chart, all)
    }

    /// Выполняет полный пайплайн и возвращает owning-матчи.
    ///
    /// Эквивалентно:
    /// `text -> tokenize/tag -> chart -> states -> trees -> normalize/prepare -> Match`.
    pub fn extract(&self, text: &str, all: bool) -> Vec<Match> {
        // 1) Подготовка входа.
        let toks_any = self.tagger.tag(self.tokenizer.tokenize(text));
        // 2) Earley-чарт и финальные состояния для root-правила.
        let chart = self.build_chart_any(&toks_any, all, false);
        let states = self.collect_states(&chart, all);
        // 3) Состояния -> деревья -> нормализованные деревья.
        let trees = prepare_matches(prepare_trees(states), self.registry, &toks_any);
        // 4) Конвертация в owning-матчи с привязкой текста и токенов.
        self.owned_from_trees(trees, text, &toks_any)
    }

    /// Возвращает лучшее одиночное совпадение (`Option<Match>`).
    ///
    /// Внутри сортирует деревья и берет первое (наиболее приоритетное по `Ord`).
    pub fn r#match(&self, text: &str) -> Option<Match> {
        let toks_any = self.tagger.tag(self.tokenizer.tokenize(text));
        let chart = self.build_chart_any(&toks_any, false, false);
        let states = self.collect_states(&chart, false);
        let mut trees = prepare_matches(prepare_trees(states), self.registry, &toks_any);
        // Детерминированный выбор "лучшего" дерева по Ord.
        trees.sort();
        let tree = trees.into_iter().next()?;
        self.owned_from_trees(vec![tree], text, &toks_any)
            .into_iter()
            .next()
    }

    /// Полиморфный поиск одного совпадения по входу `I`.
    pub fn find<'a, I>(&self, input: I) -> I::One
    where
        I: FindInput<'a>,
    {
        input.parser_find(self)
    }

    /// Полиморфный поиск всех совпадений по входу `I`.
    pub fn findall<'a, I>(&self, input: I) -> I::Many
    where
        I: FindInput<'a>,
    {
        input.parser_findall(self)
    }

    /// Первое непересекающееся совпадение в тексте: поверхностные значения листовых токенов.
    ///
    /// Внутри вызывает [`Self::find`] и преобразует результат в вектор строк значений токенов.
    pub fn find_text(&self, text: &str) -> Option<Vec<String>> {
        self.find(text).map(Match::into_token_strings)
    }

    /// Все непересекающиеся совпадения в тексте: для каждого — вектор строк значений токенов.
    ///
    /// Внутри вызывает [`Self::findall`].
    pub fn findall_text(&self, text: &str) -> Vec<Vec<String>> {
        self.findall(text)
            .into_iter()
            .map(Match::into_token_strings)
            .collect()
    }

    /// Парсит готовый срез токенов и возвращает первое совпадение.
    pub fn parse<'t>(&self, tokens: &'t [Token<'t>]) -> Option<MatchBorrowed<'t>> {
        self.find_tokens(tokens)
    }

    /// Внутренний поиск одного совпадения в тексте (возвращает [`Match`]).
    pub(super) fn find_match(&self, text: &str) -> Option<Match> {
        self.findall_match(text).into_iter().next()
    }

    /// Внутренний поиск всех совпадений в тексте с разрешением пересечений (возвращает [`Match`]).
    pub(super) fn findall_match(&self, text: &str) -> Vec<Match> {
        let toks_any = self.tagger.tag(self.tokenizer.tokenize(text));
        // all=true: поиск совпадений с любого старта, а не только с колонки 0.
        let chart = self.build_chart_any(&toks_any, true, false);
        let states = self.collect_states(&chart, true);
        // Устраняем конкурирующие по span деревья, затем стабилизируем порядок.
        let mut trees = prepare_resolved_matches(prepare_trees(states), self.registry, &toks_any);
        trees.sort();
        self.owned_from_trees(trees, text, &toks_any)
    }

    /// Внутренний поиск первого совпадения в готовом срезе токенов.
    pub(super) fn find_tokens<'t>(&self, tokens: &'t [Token<'t>]) -> Option<MatchBorrowed<'t>> {
        // Для token-входа всегда работаем в режиме all=true.
        let states = self.collect_states(&self.build_chart_plain(tokens, true), true);
        let empty_any: &[AnyToken<'_>] = &[];
        let mut trees = prepare_matches(prepare_trees(states), self.registry, empty_any);
        // Выбираем приоритетное дерево.
        trees.sort();
        let tree = trees.into_iter().next()?;
        let range = tree.range;
        Some(MatchBorrowed::new(tree, range, tokens))
    }

    /// Внутренний поиск всех совпадений в готовом срезе токенов.
    pub(super) fn findall_tokens<'t>(&self, tokens: &'t [Token<'t>]) -> Vec<MatchBorrowed<'t>> {
        let states = self.collect_states(&self.build_chart_plain(tokens, true), true);
        let empty_any: &[AnyToken<'_>] = &[];
        let mut trees = prepare_resolved_matches(prepare_trees(states), self.registry, empty_any);
        trees.sort();
        let mut out = Vec::with_capacity(trees.len());
        for tree in trees {
            let range = tree.range;
            out.push(MatchBorrowed::new(tree, range, tokens));
        }
        out
    }

    /// Конвертирует деревья в owning-матчи, прикрепляя `text` и `tokens`.
    fn owned_from_trees<'t>(
        &self,
        trees: Vec<Tree>,
        text: &str,
        tokens: &[AnyToken<'t>],
    ) -> Vec<Match> {
        // Arc на текст позволяет дешево шарить одну строку между всеми Match.
        let text_arc: Arc<str> = Arc::from(text);

        // Конвертируем входные токены в компактный owning-формат ровно один раз.
        let mut v = Vec::with_capacity(tokens.len());
        for t in tokens {
            v.push(AnyTokenOwnedLite::from(t));
        }
        let tokens_arc: Arc<[AnyTokenOwnedLite]> = v.into();

        // Каждый tree получает общий text/tokens (clone Arc = O(1)).
        let mut out = Vec::with_capacity(trees.len());
        for tree in trees {
            let range = tree.range;
            out.push(Match::new(
                tree,
                range,
                text_arc.clone(),
                tokens_arc.clone(),
            ));
        }
        out
    }

    /// Собирает финальные состояния корневого правила из чарта.
    fn collect_states<'t>(&self, chart: &Chart<'t>, all: bool) -> Vec<State> {
        if all {
            // Берем финальные состояния root-правила из всех колонок.
            chart
                .matches(self.root_id, self.registry)
                .cloned()
                .collect()
        } else {
            // Классический parse: только из последней колонки.
            chart
                .last_column()
                .matches(self.root_id, self.registry)
                .cloned()
                .collect()
        }
    }

    fn build_chart_any<'s, 't>(
        &self,
        tokens: &'s [AnyToken<'t>],
        all: bool,
        store_tokens: bool,
    ) -> Chart<'t> {
        // Earley по колонкам: колонка `i` соответствует позиции перед токеном `tokens[i]`
        // (колонка 0 — до первого токена). В каждой колонке обрабатываем состояния по мере
        // роста списка: predict (новые правила/продукции), scan (сдвиг dot по предикату),
        // complete (сдвиг dot у родителей после завершённого вложенного правила).
        //
        // `all`: стартовать корень не только из колонки 0, а из любой позиции (findall).
        // `store_tokens`: положить в чарт plain-токены для API; иначе только скелет колонок.
        let mut chart = if store_tokens {
            // Нужен "полный" chart с токенами (например для API chart()).
            let mut toks_plain = Vec::with_capacity(tokens.len());
            for t in tokens {
                toks_plain.push(anytoken_to_token(t));
            }
            Chart::new(toks_plain)
        } else {
            // Быстрый режим: только количество колонок, без копирования токенов.
            Chart::from_token_count(tokens.len())
        };
        let mut memo = PredictMemo::new(chart.len());

        for col_idx in 0..chart.len() {
            if chart.columns[col_idx].len() == 0 && !(col_idx == 0 || all) {
                continue;
            }

            // Токен «на входе» этой колонки: для predict/scan используется как lookahead.
            let next_any: Option<&AnyToken<'t>> = tokens.get(col_idx);
            let lookahead_any: Option<&dyn TokenView> = next_any.map(|t| t as &dyn TokenView);

            if (col_idx == 0 || all) && memo.should_predict(col_idx, self.root_id) {
                // В каждую стартовую колонку добавляем начальные состояния root.
                self.predict(&mut chart, col_idx, self.root_id, lookahead_any);
            }

            // Длина колонки может расти во время итерации (predict/complete добавляют сюда же).
            let mut state_idx = 0usize;
            while state_idx < chart.columns[col_idx].len() {
                let (is_completed, next_term, snap) = {
                    let s = chart.columns[col_idx]
                        .get(state_idx)
                        .expect("state index must be valid");

                    let is_completed = s.completed(self.registry);
                    let next_term = if is_completed {
                        None
                    } else {
                        s.next_term(self.registry)
                    };

                    let snap = (
                        s.rule_id,
                        s.production_index,
                        s.dot,
                        s.start_col,
                        Arc::clone(&s.node),
                    );
                    (is_completed, next_term, snap)
                };

                let (rule_id, prod_idx, dot, start_col, node) = snap;

                if is_completed {
                    // Complete: продукция дочитана — поднимаемся к родителям в колонке start_col.
                    self.complete(&mut chart, col_idx, start_col, rule_id, &node);
                } else if let Some(next_term) = next_term {
                    match next_term {
                        Term::Rule(rule2) => {
                            // Predict: следующий символ — нетерминал; добавляем состояния для этого правила.
                            if let Some(rule_id2) = self.registry.rule_id_for_arc(rule2) {
                                if memo.should_predict(col_idx, rule_id2) {
                                    self.predict(&mut chart, col_idx, rule_id2, lookahead_any);
                                }
                            }
                        }
                        Term::Pred(predicate) => {
                            // Scan: следующий символ — предикат; при успехе состояние уходит в следующую колонку.
                            if let Some(tok_any) = next_any {
                                if col_idx + 1 < chart.len() {
                                    self.scan_any_fast(
                                        &mut chart,
                                        col_idx + 1,
                                        col_idx,
                                        tok_any,
                                        predicate,
                                        rule_id,
                                        prod_idx,
                                        dot,
                                        start_col,
                                        &node,
                                    );
                                }
                            }
                        }
                    }
                }

                state_idx += 1;
            }
        }

        chart
    }

    fn build_chart_plain<'t>(&self, tokens: &'t [Token<'t>], all: bool) -> Chart<'t> {
        // Тот же цикл Earley, что в `build_chart_any`, но вход уже `Token` и чарт хранит этот срез;
        // scan идёт через `scan_plain_fast` (без обёртки `AnyToken`).
        let mut chart = Chart::from_slice(tokens);
        let mut memo = PredictMemo::new(chart.len());

        for col_idx in 0..chart.len() {
            // Колонка без состояний и не стартовая (и без режима `all`) — дальше по тексту разбор не «течёт».
            if chart.columns[col_idx].len() == 0 && !(col_idx == 0 || all) {
                continue;
            }

            // Токен, который будет сопоставлен при scan при выходе из этой колонки; для predict — lookahead.
            let next_tok: Option<&Token<'t>> = tokens.get(col_idx);
            let lookahead_tok: Option<&dyn TokenView> = next_tok.map(|t| t as &dyn TokenView);

            if col_idx == 0 || all {
                // Стартовые items корневого правила в колонках 0 или при «скользящем» старте (`all`).
                if memo.should_predict(col_idx, self.root_id) {
                    self.predict(&mut chart, col_idx, self.root_id, lookahead_tok);
                }
            }

            // Обходим все состояния колонки; predict/complete могут добавлять новые в конец — обходим по индексу.
            let mut state_idx = 0usize;
            while state_idx < chart.columns[col_idx].len() {
                // Снимаем снимок полей состояния без удержания borrow колонки на время вызовов chart.
                let (is_completed, next_term, snap) = {
                    let s = chart.columns[col_idx]
                        .get(state_idx)
                        .expect("state index must be valid");
                    let is_completed = s.completed(self.registry);
                    let next_term = if is_completed {
                        None
                    } else {
                        s.next_term(self.registry)
                    };
                    let snap = (
                        s.rule_id,
                        s.production_index,
                        s.dot,
                        s.start_col,
                        Arc::clone(&s.node),
                    );
                    (is_completed, next_term, snap)
                };

                let (rule_id, prod_idx, dot, start_col, node) = snap;

                if is_completed {
                    // Продукция закрыта — complete поднимает dot у родителей в колонке `start_col`.
                    self.complete(&mut chart, col_idx, start_col, rule_id, &node);
                } else if let Some(next_term) = next_term {
                    match next_term {
                        Term::Rule(rule2) => {
                            // За точкой нетерминал — predict разворачивает вложенное правило в той же колонке.
                            if let Some(rule_id2) = self.registry.rule_id_for_arc(rule2) {
                                if memo.should_predict(col_idx, rule_id2) {
                                    self.predict(&mut chart, col_idx, rule_id2, lookahead_tok);
                                }
                            }
                        }
                        Term::Pred(predicate) => {
                            // Scan по `Token`: предикат проверяет `next_tok`, новое состояние — в `col_idx + 1`.
                            if let Some(tok) = next_tok {
                                if col_idx + 1 < chart.len() {
                                    self.scan_plain_fast(
                                        &mut chart,
                                        col_idx + 1,
                                        col_idx,
                                        tok,
                                        predicate,
                                        rule_id,
                                        prod_idx,
                                        dot,
                                        start_col,
                                        &node,
                                    );
                                }
                            }
                        }
                    }
                }

                state_idx += 1;
            }
        }

        chart
    }

    fn predict<'t>(
        &self,
        chart: &mut Chart<'t>,
        col_idx: usize,
        rule_id: RuleId,
        lookahead: Option<&dyn TokenView>,
    ) {
        // Predict: для каждой подходящей продукции `rule_id` добавляем Earley-item с dot=0
        // в текущую колонку (узел — пустой нетерминальный корень этой продукции).
        for prod_idx in self
            .registry
            .predicted_production_indices(rule_id, lookahead)
        {
            let node = Arc::new(Node::new(rule_id, prod_idx, prod_idx));
            let st = State::new(rule_id, prod_idx, 0, col_idx, col_idx, node);
            chart.columns[col_idx].append(st, self.registry);
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn scan_any_fast<'t>(
        &self,
        chart: &mut Chart<'t>,
        next_col_idx: usize,
        token_index: usize,
        token: &AnyToken<'t>,
        predicate: &PredicateKind<'r>,
        rule_id: RuleId,
        production_index: usize,
        dot: usize,
        start_col: usize,
        node: &Arc<Node>,
    ) {
        // Scan: предикат — терминал на позиции dot; при совпадении с `token` сдвигаем dot,
        // цепляем лист с индексом токена и span, пишем в колонку `next_col_idx`.
        if predicate.check(token) {
            let matched_forms = predicate.matched_forms(token);
            let leaf = Arc::new(Leaf::with_forms(
                0,
                token_index,
                token.span(),
                matched_forms,
            ));
            let next_state = State::new(
                rule_id,
                production_index,
                dot + 1,
                start_col,
                next_col_idx,
                node.attached(ParseChild::Leaf(leaf)),
            );
            chart.columns[next_col_idx].append(next_state, self.registry);
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn scan_plain_fast<'t>(
        &self,
        chart: &mut Chart<'t>,
        next_col_idx: usize,
        token_index: usize,
        token: &Token<'t>,
        predicate: &PredicateKind<'r>,
        rule_id: RuleId,
        production_index: usize,
        dot: usize,
        start_col: usize,
        node: &Arc<Node>,
    ) {
        if predicate.check(token) {
            let matched_forms = predicate.matched_forms(token);
            let leaf = Arc::new(Leaf::with_forms(0, token_index, token.span, matched_forms));
            let next_state = State::new(
                rule_id,
                production_index,
                dot + 1,
                start_col,
                next_col_idx,
                node.attached(ParseChild::Leaf(leaf)),
            );
            chart.columns[next_col_idx].append(next_state, self.registry);
        }
    }

    fn complete<'t>(
        &self,
        chart: &mut Chart<'t>,
        col_idx: usize,
        origin: usize,          // completed.start_col
        completed_rule: RuleId, // completed.rule_id
        completed_node: &Arc<Node>,
    ) {
        // Complete: в колонке `origin` ищем состояния, у которых следующий символ —
        // нетерминал `completed_rule`; он только что «закрылся» в `completed_node`.
        // Для каждого такого родителя добавляем в колонку `col_idx` состояние с dot+1
        // и дочерним узлом, указывающим на завершённый подразбор.
        if origin < col_idx {
            // `origin` и `col_idx` — разные колонки: читаем родителей и пишем successors без копии.
            let (left, right) = chart.columns.split_at_mut(col_idx);
            let origin_col = &left[origin];
            let cur_col = &mut right[0];

            // origin_col: где "начался" completed; cur_col: куда пишем продвинутые parents.
            for parent in origin_col.parents(completed_rule) {
                let next_state = State::new(
                    parent.rule_id,
                    parent.production_index,
                    parent.dot + 1,
                    parent.start_col,
                    col_idx,
                    parent
                        .node
                        .attached(ParseChild::Node(Arc::clone(completed_node))),
                );
                cur_col.append(next_state, self.registry);
            }
        } else {
            // origin == col_idx: split_at_mut нельзя, поэтому временно материализуем parents.
            let parents: Vec<State> = chart.columns[origin]
                .parents(completed_rule)
                .cloned()
                .collect();

            for parent in parents {
                let next_state = State::new(
                    parent.rule_id,
                    parent.production_index,
                    parent.dot + 1,
                    parent.start_col,
                    col_idx,
                    parent
                        .node
                        .attached(ParseChild::Node(Arc::clone(completed_node))),
                );
                chart.columns[col_idx].append(next_state, self.registry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::predicates::constructors::eq;
    use crate::rule::builder::{pred, term};
    use crate::token::{TokenType, Tokenizer};

    struct DropPunctTagger;

    impl Tagger for DropPunctTagger {
        fn tag<'t>(&self, tokens: Vec<AnyToken<'t>>) -> Vec<AnyToken<'t>> {
            tokens
                .into_iter()
                .filter(|t| t.token_type() != TokenType::Punct)
                .collect()
        }
    }

    #[test]
    fn api_example_style_geo_with_republic_variant() {
        let text = "Russian republic";
        let mut registry = RuleRegistry::new();
        let geo_id = registry.add((term("Russian") + term("republic")).build(()));
        let parser = Parser::new(&registry, geo_id);
        let values = parser.find_text(text).expect("expected geo match");
        assert_eq!(values, vec!["Russian".to_string(), "republic".to_string()]);
    }

    #[test]
    fn find_text_matches_find_into_token_strings() {
        let text = "hello world";
        let mut registry = RuleRegistry::new();
        let root_id = registry.next_id();
        registry.add((term("hello") + term("world")).build(root_id));
        let parser = Parser::with_tokenizer(&registry, root_id, Arc::new(Tokenizer::new()));

        let a = parser
            .find(text)
            .map(|m| m.into_token_strings())
            .expect("find");
        let b = parser.find_text(text).expect("find_text");
        assert_eq!(a, b);
    }

    #[test]
    fn parse_find_and_findall_for_token_inputs_work() {
        let text = "hello world";
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize(text);

        let mut registry = RuleRegistry::new();
        let root_id = registry.next_id();
        registry.add((term("hello") + term("world")).build(root_id));

        let parser = Parser::with_tokenizer(&registry, root_id, Arc::new(Tokenizer::new()));

        let by_parse = parser.parse(&tokens).expect("parse should match");
        let by_slice = parser
            .find(tokens.as_slice())
            .expect("find(&[Token]) should match");
        let by_vec = parser
            .find(&tokens)
            .expect("find(&Vec<Token>) should match");
        let all = parser.findall(tokens.as_slice());

        assert_eq!(by_parse.token_range(), (0, 2));
        assert_eq!(by_slice.token_range(), (0, 2));
        assert_eq!(by_vec.token_range(), (0, 2));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].tokens()[0].value.as_ref(), "hello");
        assert_eq!(all[0].tokens()[1].value.as_ref(), "world");
    }

    #[test]
    fn chart_matches_extract_and_match_methods_work_together() {
        let mut registry = RuleRegistry::new();
        let root_id = registry.next_id();
        registry.add((term("hello") + term("world")).build(root_id));

        let parser = Parser::with_tokenizer(&registry, root_id, Arc::new(Tokenizer::new()));
        let chart = parser.chart("hello world", false);
        let states = parser.matches("hello world", false);
        let extracted = parser.extract("hello world", false);
        let best = parser.r#match("hello world").expect("match should exist");

        assert_eq!(chart.tokens.len(), 2);
        assert_eq!(chart.len(), 3);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].range(), (0, 2));
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].matched_text(), "hello world");
        assert_eq!(best.matched_text(), "hello world");
    }

    #[test]
    fn findall_resolves_overlapping_matches_for_text_input() {
        let mut registry = RuleRegistry::new();
        let root_id = registry.next_id();
        registry.add(((term("hello") + term("world")) | term("hello")).build(root_id));

        let parser = Parser::with_tokenizer(&registry, root_id, Arc::new(Tokenizer::new()));
        let matches = parser.findall("hello world");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text(), "hello world");
        assert_eq!(matches[0].token_range(), (0, 2));
    }

    #[test]
    fn custom_tagger_is_used_in_text_pipeline() {
        let mut registry = RuleRegistry::new();
        let root_id = registry.next_id();
        registry.add((term("hello") + term("world")).build(root_id));

        let parser_without_tagger =
            Parser::with_tokenizer(&registry, root_id, Arc::new(Tokenizer::new()));
        assert!(parser_without_tagger.find("hello, world").is_none());

        let parser_with_tagger = Parser::with_tokenizer_and_tagger(
            &registry,
            root_id,
            Arc::new(Tokenizer::new()),
            Arc::new(DropPunctTagger),
        );
        let m = parser_with_tagger
            .find("hello, world")
            .expect("tagger should allow match");
        let values: Vec<String> = m
            .tokens()
            .into_iter()
            .map(|t| t.value.into_owned())
            .collect();
        assert_eq!(values, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn early_skip_does_not_create_false_match_when_parse_dies_early() {
        let mut registry = RuleRegistry::new();

        // RULE = eq("a") + eq("b")
        let rule_id = registry.next_id();
        let rule = (pred(eq("a")) + pred(eq("b"))).build(rule_id);
        registry.add(rule);

        let parser = Parser::new(&registry, rule_id);

        let text = "a x x x x x x x x x";

        let m = parser.r#match(text);
        assert!(m.is_none(), "parser must not produce a match");
    }

    #[test]
    fn early_skip_does_not_break_findall_all_true() {
        let mut registry = RuleRegistry::new();

        // RULE = eq("a") + eq("b")
        let rule_id = registry.next_id();
        let rule = (pred(eq("a")) + pred(eq("b"))).build(rule_id);
        registry.add(rule);

        let parser = Parser::new(&registry, rule_id);

        let text = "x a b y a b z";
        let got = parser.findall_text(text);

        assert_eq!(got.len(), 2);

        assert_eq!(
            got,
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["a".to_string(), "b".to_string()]
            ]
        );
    }

    #[test]
    fn early_skip_handles_empty_input_correctly() {
        let mut registry = RuleRegistry::new();

        // RULE = eq("a")
        let rule_id = registry.next_id();
        let rule = pred(eq("a")).build(rule_id);
        registry.add(rule);

        let parser = Parser::new(&registry, rule_id);

        let text = "";
        assert!(parser.r#match(text).is_none());
        assert!(parser.findall(text).is_empty());
    }
}
