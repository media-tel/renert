//! Пайплайны ключевых фраз для быстрого построения правил.
//!
//! Модуль предоставляет три варианта пайплайна:
//! - [`Pipeline`] — точное пословное совпадение;
//! - [`CaselessPipeline`] — совпадение без учета регистра;
//! - [`MorphPipeline`] — совпадение по нормальным формам слов.
//!
//! Каждый пайплайн можно:
//! - преобразовать в набор продукций (`productions`);
//! - представить как BNF-правило (`as_bnf`);
//! - конвертировать в [`Rule`] для парсера (`into_rule`).
//!
//! Для удобства есть схемы (`*Scheme`) и функции верхнего уровня
//! [`pipeline`], [`caseless_pipeline`], [`morph_pipeline`].
//!
//! Примеры и обзор газеттиров — в документации модуля [`pipeline`](crate::pipeline).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use crate::predicates::bank::Dictionary;
use crate::predicates::constructors::{caseless_owned, eq_owned, PredicateKind, TokenView};
use crate::rule::constructors::{Production, Rule, Term, TermOrMain};
use crate::token::{global_morph_tokenizer, MorphTokenizer, Tokenizer};

/// Обходит все строковые значения, упоминаемые в предикате, и применяет к ним переданную функцию.
fn visit_predicate_values(predicate: &PredicateKind<'_>, f: &mut impl FnMut(&str)) {
    match predicate {
        PredicateKind::Eq(p) => f(p.value.as_ref()),
        PredicateKind::Caseless(p) => f(p.value.as_ref()),
        PredicateKind::Normalized(p) => f(p.value),
        PredicateKind::Gram(p) => f(p.value),

        PredicateKind::InCaseless(p) => {
            for value in &p.values {
                f(value);
            }
        }

        PredicateKind::Dictionary(p) => {
            for value in &p.values {
                f(value);
            }
        }

        PredicateKind::Or(items) => {
            for item in items {
                visit_predicate_values(item, f);
            }
        }

        _ => {}
    }
}

/// Извлекает строковые значения только из первого терма продукции и применяет к ним функцию.
fn visit_first_term_values(term: &Term<'_>, f: &mut impl FnMut(&str)) {
    match term {
        Term::Pred(predicate) => visit_predicate_values(predicate, f),
        Term::Rule(_) => {}
    }
}

/// Строит индекс продукций по строковым значениям первого терма с заданной нормализацией.
fn build_index_from_first_term_by<'a>(
    productions: &[Production<'a>],
    mut normalize: impl FnMut(&str) -> String,
) -> HashMap<String, Vec<usize>> {
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();

    for (prod_idx, production) in productions.iter().enumerate() {
        let Some(first_term) = production.terms.first() else {
            continue;
        };

        visit_first_term_values(first_term, &mut |value| {
            index.entry(normalize(value)).or_default().push(prod_idx);
        });
    }

    index
}

/// Строит индекс по первому терму без изменения регистра.
fn build_index_from_first_term(productions: &[Production<'_>]) -> HashMap<String, Vec<usize>> {
    build_index_from_first_term_by(productions, |value| value.to_owned())
}

/// Строит индекс по первому терму, нормализуя значения к lowercase.
fn build_index_from_first_term_lowercase(
    productions: &[Production<'_>],
) -> HashMap<String, Vec<usize>> {
    build_index_from_first_term_by(productions, |value| value.to_lowercase())
}

/// Возвращает итератор по продукциям, индексированным по заданному строковому ключу.
fn indexed_productions<'s, 'a>(
    productions: &'s [Production<'a>],
    index: &'s HashMap<String, Vec<usize>>,
    key: &str,
) -> impl Iterator<Item = &'s Production<'a>> + 's {
    index
        .get(key)
        .into_iter()
        .flat_map(|indices| indices.iter().copied())
        .filter_map(|idx| productions.get(idx))
}

/// Создаёт словарный предикат `Dictionary` из набора строк, владение которыми остаётся в предикате.
fn dictionary_owned<'a>(values: &[String]) -> PredicateKind<'a> {
    let set: HashSet<String> = values.iter().cloned().collect();
    PredicateKind::Dictionary(Dictionary { values: set })
}

/// Проверяет, состоит ли строка только из алфавитных символов.
fn is_alpha_token(value: &str) -> bool {
    value.chars().all(|ch| ch.is_alphabetic())
}

/// Строит морфологический предикат для набора вариантов терма.
fn morph_predicate_for_values<'a>(values: &[String]) -> PredicateKind<'a> {
    if values.iter().all(|value| is_alpha_token(value)) {
        return dictionary_owned(values);
    }

    if values.len() == 1 {
        return eq_owned(values[0].clone());
    }

    let variants: Vec<PredicateKind<'a>> =
        values.iter().map(|value| eq_owned(value.clone())).collect();

    PredicateKind::Or(variants)
}

/// Универсальный конструктор BNF-правил: собирает метку, продукции и индекс по ним.
fn make_rule<'a>(
    label: impl Into<String>,
    productions: impl Into<Vec<Production<'a>>>,
    index_builder: impl FnOnce(&[Production<'a>]) -> HashMap<String, Vec<usize>>,
) -> (String, Vec<Production<'a>>, HashMap<String, Vec<usize>>) {
    let productions = productions.into();
    let index = index_builder(&productions);
    let label = label.into();
    (label, productions, index)
}

/// BNF-слой точного пайплайна: продукции и индекс по первому терму для [`predict`](PipelineBNFRule::predict).
#[derive(Debug, Clone)]
pub struct PipelineBNFRule<'a> {
    /// Имя BNF-правила.
    pub label: String,
    /// Продукции, соответствующие ключам пайплайна.
    pub productions: Vec<Production<'a>>,
    index: HashMap<String, Vec<usize>>,
}

impl<'a> PipelineBNFRule<'a> {
    /// Служебная аббревиатура для текстового BNF-представления.
    pub const ABBR: &'static str = "pipeline";

    /// Создает BNF-правило пайплайна и строит индекс по первому терму продукции.
    pub fn new(label: impl Into<String>, productions: impl Into<Vec<Production<'a>>>) -> Self {
        let (label, productions, index) =
            make_rule(label, productions, build_index_from_first_term);

        Self {
            label,
            productions,
            index,
        }
    }

    /// Возвращает продукции-кандидаты по текущему токену lookahead.
    pub fn predict<'s>(
        &'s self,
        token: &dyn TokenView,
    ) -> impl Iterator<Item = &'s Production<'a>> + 's {
        indexed_productions(&self.productions, &self.index, token.token().value.as_ref())
    }
}

impl fmt::Display for PipelineBNFRule<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.label, Self::ABBR)
    }
}

/// BNF-слой caseless-пайплайна: индекс по первому терму в lowercase, [`predict`](CaselessPipelineBNFRule::predict) без учёта регистра.
#[derive(Debug, Clone)]
pub struct CaselessPipelineBNFRule<'a> {
    /// Имя BNF-правила.
    pub label: String,
    /// Продукции, соответствующие ключам пайплайна.
    pub productions: Vec<Production<'a>>,
    index: HashMap<String, Vec<usize>>,
}

impl<'a> CaselessPipelineBNFRule<'a> {
    /// Служебная аббревиатура для текстового BNF-представления.
    pub const ABBR: &'static str = "caseless_pipeline";

    /// Создает BNF-правило и индекс по первому терму в lowercase.
    pub fn new(label: impl Into<String>, productions: impl Into<Vec<Production<'a>>>) -> Self {
        let (label, productions, index) =
            make_rule(label, productions, build_index_from_first_term_lowercase);

        Self {
            label,
            productions,
            index,
        }
    }

    /// Возвращает продукции-кандидаты по токену без учета регистра.
    pub fn predict<'s>(
        &'s self,
        token: &dyn TokenView,
    ) -> impl Iterator<Item = &'s Production<'a>> + 's {
        let value = token.token().value.to_lowercase();
        indexed_productions(&self.productions, &self.index, &value)
    }
}

impl fmt::Display for CaselessPipelineBNFRule<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.label, Self::ABBR)
    }
}

/// BNF-слой морфологического пайплайна: [`predict`](MorphPipelineBNFRule::predict) сопоставляет токен с индексом по нормальным формам.
#[derive(Debug, Clone)]
pub struct MorphPipelineBNFRule<'a> {
    /// Имя BNF-правила.
    pub label: String,
    /// Продукции, соответствующие ключам пайплайна.
    pub productions: Vec<Production<'a>>,
    index: HashMap<String, Vec<usize>>,
}

impl<'a> MorphPipelineBNFRule<'a> {
    /// Служебная аббревиатура для текстового BNF-представления.
    pub const ABBR: &'static str = "morph_pipeline";

    /// Создает BNF-правило и индекс по первому терму продукции.
    pub fn new(label: impl Into<String>, productions: impl Into<Vec<Production<'a>>>) -> Self {
        let (label, productions, index) =
            make_rule(label, productions, build_index_from_first_term);

        Self {
            label,
            productions,
            index,
        }
    }

    /// Возвращает продукции-кандидаты по нормальным формам токена.
    ///
    /// Если у токена есть морфологические формы, сравнение идет по множеству `normalized`.
    /// Иначе используется `token.normalized_value()`.
    pub fn predict<'s>(
        &'s self,
        token: &dyn TokenView,
    ) -> impl Iterator<Item = &'s Production<'a>> + 's {
        let mut matched: Vec<usize> = Vec::new();

        if let Some(forms) = token.forms() {
            // Удаляем дубли нормальных форм до lookup, чтобы не раздувать matched лишними индексами.
            let mut seen_forms: HashSet<&str> = HashSet::with_capacity(forms.len());
            for form in forms {
                let value = form.normalized.as_str();
                if !seen_forms.insert(value) {
                    continue;
                }

                if let Some(indices) = self.index.get(value) {
                    matched.extend(indices.iter().copied());
                }
            }
        } else {
            let value = token.token().normalized_value();
            if let Some(indices) = self.index.get(value.as_ref()) {
                matched.extend(indices.iter().copied());
            }
        }

        if matched.len() > 1 {
            // Дедуп за O(n log n), но только когда это действительно нужно.
            matched.sort_unstable();
            matched.dedup();
        }

        matched.into_iter().map(move |idx| &self.productions[idx])
    }
}

impl fmt::Display for MorphPipelineBNFRule<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.label, Self::ABBR)
    }
}

/// Один словарный ключ пайплайна: искомая фраза (`value`) и термы с вариантами на каждую позицию.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// Исходная строка ключа (значение, которое пайплайн должен находить).
    pub value: String,
    /// Разбитые термы ключа; каждый терм может иметь несколько вариантов.
    pub terms: Vec<Vec<String>>,
}

impl Key {
    /// Создает ключ с явным набором термов и вариантов.
    pub fn new(value: impl Into<String>, terms: Vec<Vec<String>>) -> Self {
        Self {
            value: value.into(),
            terms,
        }
    }

    /// Создает ключ, где каждый терм имеет ровно один вариант.
    pub fn simple(value: impl Into<String>, terms: Vec<String>) -> Self {
        Self {
            value: value.into(),
            terms: terms.into_iter().map(|term| vec![term]).collect(),
        }
    }
}

/// Продукция [`Rule`], собранная из ключа пайплайна; хранит исходный `value` ключа поверх базовой продукции.
#[derive(Debug, Clone)]
pub struct PipelineProduction<'a> {
    /// Значение ключа, связанное с продукцией.
    pub value: String,
    base: Production<'a>,
}

impl<'a> PipelineProduction<'a> {
    /// Создает пайплайн-продукцию из значения ключа и списка термов.
    pub fn new(value: impl Into<String>, terms: Vec<TermOrMain<'a>>) -> Self {
        Self {
            value: value.into(),
            base: Production::new(terms, None),
        }
    }

    /// Возвращает ссылку на базовую продукцию.
    pub fn as_production(&self) -> &Production<'a> {
        &self.base
    }
}

impl<'a> Deref for PipelineProduction<'a> {
    type Target = Production<'a>;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<'a> From<PipelineProduction<'a>> for Production<'a> {
    fn from(value: PipelineProduction<'a>) -> Self {
        value.base
    }
}

/// Токенизирует строку и возвращает ключ, где каждый терм — одиночный вариант.
fn key_from_tokenizer_line(line: &str, tokenizer: &Tokenizer) -> Key {
    let parts: Vec<String> = tokenizer
        .split(line)
        .into_iter()
        .map(|part| part.into_owned())
        .collect();
    Key::simple(line.to_string(), parts)
}

#[inline]
fn collect_scheme_lines(lines: impl IntoIterator<Item = String>) -> Vec<String> {
    lines.into_iter().collect()
}

#[inline]
fn tokenizer_scheme_keys<'a>(
    lines: &'a [String],
    tokenizer: &'a Tokenizer,
) -> impl Iterator<Item = Key> + 'a {
    lines
        .iter()
        .map(move |line| key_from_tokenizer_line(line, tokenizer))
}

/// Строит список продукций пайплайна для ключей, используя переданный конструктор предикатов.
fn simple_pipeline_productions<F>(
    keys: &[Key],
    mut predicate: F,
) -> Vec<PipelineProduction<'static>>
where
    F: FnMut(String) -> PredicateKind<'static>,
{
    let mut out = Vec::with_capacity(keys.len());

    for key in keys {
        let mut terms: Vec<TermOrMain<'static>> = Vec::with_capacity(key.terms.len());
        for values in &key.terms {
            if let Some(value) = values.first() {
                terms.push(TermOrMain::from(predicate(value.clone())));
            }
        }

        out.push(PipelineProduction::new(key.value.clone(), terms));
    }

    out
}

/// Строит список морфологических продукций для ключей, учитывая все нормальные формы термов.
fn morph_pipeline_productions(keys: &[Key]) -> Vec<PipelineProduction<'static>> {
    let mut out = Vec::with_capacity(keys.len());

    for key in keys {
        let mut terms: Vec<TermOrMain<'static>> = Vec::with_capacity(key.terms.len());
        for values in &key.terms {
            terms.push(TermOrMain::from(morph_predicate_for_values(values)));
        }

        out.push(PipelineProduction::new(key.value.clone(), terms));
    }

    out
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct CompiledPipeline<R> {
    pipeline_productions: Vec<PipelineProduction<'static>>,
    rule: Arc<Rule<'static>>,
    bnf_rule: R,
}

fn rebind_pipeline_productions<'a>(
    productions: &[PipelineProduction<'static>],
) -> Vec<PipelineProduction<'a>> {
    productions
        .iter()
        .cloned()
        .map(rebind_pipeline_production)
        .collect()
}

fn rebind_pipeline_production<'a>(
    production: PipelineProduction<'static>,
) -> PipelineProduction<'a> {
    // Safety: compiled pipeline productions are built only from owned `String`/`Cow::Owned`
    // data and do not borrow from the source pipeline after compilation.
    unsafe {
        std::mem::transmute::<PipelineProduction<'static>, PipelineProduction<'a>>(production)
    }
}

fn rebind_rule<'a>(rule: Arc<Rule<'static>>) -> Arc<Rule<'a>> {
    // Safety: compiled rules used here own all string data inside predicates.
    unsafe { std::mem::transmute::<Arc<Rule<'static>>, Arc<Rule<'a>>>(rule) }
}

/// Стратегия, описывающая конкретный вид пайплайна.
#[doc(hidden)]
pub trait PipelineStrategy {
    /// Тип BNF-правила для этого вида пайплайна.
    type BnfRule<'a>: Clone;

    /// Статическая текстовая метка пайплайна.
    const LABEL: &'static str;

    /// Скомпилировать пайплайн в переиспользуемые структуры.
    fn compile(keys: &[Key]) -> CompiledPipeline<Self::BnfRule<'static>>;

    /// Понизить lifetime owning-BNF-структуры до вызываемого контекста.
    fn rebind_bnf_rule<'a>(rule: Self::BnfRule<'static>) -> Self::BnfRule<'a>;
}

#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct ExactStrategy;

impl PipelineStrategy for ExactStrategy {
    type BnfRule<'a> = PipelineBNFRule<'a>;

    const LABEL: &'static str = "Pipeline";

    fn compile(keys: &[Key]) -> CompiledPipeline<Self::BnfRule<'static>> {
        let pipeline_productions = simple_pipeline_productions(keys, eq_owned);
        let base_productions: Vec<Production<'static>> = pipeline_productions
            .iter()
            .cloned()
            .map(Production::from)
            .collect();

        let values: Vec<String> = pipeline_productions
            .iter()
            .map(|production| production.value.clone())
            .collect();

        CompiledPipeline {
            // Сохраняем канонические значения pipeline, чтобы normalized() возвращал key, а не поверхностный текст.
            rule: Arc::new(Rule::new(base_productions.clone())).pipeline(Self::LABEL, values),
            bnf_rule: PipelineBNFRule::new(Self::LABEL, base_productions),
            pipeline_productions,
        }
    }

    fn rebind_bnf_rule<'a>(rule: Self::BnfRule<'static>) -> Self::BnfRule<'a> {
        // Safety: compiled BNF rules here contain only owning productions/index keys.
        unsafe { std::mem::transmute::<Self::BnfRule<'static>, Self::BnfRule<'a>>(rule) }
    }
}

#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct CaselessStrategy;

impl PipelineStrategy for CaselessStrategy {
    type BnfRule<'a> = CaselessPipelineBNFRule<'a>;

    const LABEL: &'static str = "CaselessPipeline";

    fn compile(keys: &[Key]) -> CompiledPipeline<Self::BnfRule<'static>> {
        let pipeline_productions = simple_pipeline_productions(keys, caseless_owned);
        let base_productions: Vec<Production<'static>> = pipeline_productions
            .iter()
            .cloned()
            .map(Production::from)
            .collect();

        let values: Vec<String> = pipeline_productions
            .iter()
            .map(|production| production.value.clone())
            .collect();

        CompiledPipeline {
            // Сохраняем канонические значения pipeline, чтобы normalized() возвращал key, а не поверхностный текст.
            rule: Arc::new(Rule::new(base_productions.clone())).pipeline(Self::LABEL, values),
            bnf_rule: CaselessPipelineBNFRule::new(Self::LABEL, base_productions),
            pipeline_productions,
        }
    }

    fn rebind_bnf_rule<'a>(rule: Self::BnfRule<'static>) -> Self::BnfRule<'a> {
        // Safety: compiled BNF rules here contain only owning productions/index keys.
        unsafe { std::mem::transmute::<Self::BnfRule<'static>, Self::BnfRule<'a>>(rule) }
    }
}

/// Обобщённый пайплайн, параметризованный стратегией.
#[derive(Debug)]
pub struct GenericPipeline<S: PipelineStrategy> {
    /// Набор ключей пайплайна.
    pub keys: Vec<Key>,
    compiled: OnceLock<CompiledPipeline<S::BnfRule<'static>>>,
    _marker: PhantomData<S>,
}

impl<S: PipelineStrategy> GenericPipeline<S> {
    /// Создает пайплайн.
    pub fn new(keys: impl IntoIterator<Item = Key>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            compiled: OnceLock::new(),
            _marker: PhantomData,
        }
    }

    fn compiled(&self) -> &CompiledPipeline<S::BnfRule<'static>> {
        self.compiled.get_or_init(|| S::compile(&self.keys))
    }

    /// Совместимый no-op активатор.
    pub fn activate(self) -> Self {
        self
    }

    /// Возвращает продукции пайплайна.
    pub fn productions(&self) -> Vec<PipelineProduction<'_>> {
        rebind_pipeline_productions(&self.compiled().pipeline_productions)
    }

    /// Преобразует пайплайн в BNF-правило.
    pub fn as_bnf(&self) -> S::BnfRule<'_> {
        S::rebind_bnf_rule(self.compiled().bnf_rule.clone())
    }

    /// Человекочитаемая метка пайплайна.
    pub fn label(&self) -> &str {
        S::LABEL
    }

    /// Преобразует пайплайн в [`Rule`].
    #[allow(clippy::wrong_self_convention)]
    pub fn into_rule<'a>(&self) -> Arc<Rule<'a>> {
        rebind_rule(self.compiled().rule.clone())
    }
}

impl<S: PipelineStrategy> Clone for GenericPipeline<S> {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            compiled: OnceLock::new(),
            _marker: PhantomData,
        }
    }
}

/// Точный (case-sensitive) пайплайн.
pub type Pipeline = GenericPipeline<ExactStrategy>;

/// Пайплайн без учета регистра.
pub type CaselessPipeline = GenericPipeline<CaselessStrategy>;

#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct MorphStrategy;

impl PipelineStrategy for MorphStrategy {
    type BnfRule<'a> = MorphPipelineBNFRule<'a>;

    const LABEL: &'static str = "MorphPipeline";

    fn compile(keys: &[Key]) -> CompiledPipeline<Self::BnfRule<'static>> {
        let pipeline_productions = morph_pipeline_productions(keys);
        let base_productions: Vec<Production<'static>> = pipeline_productions
            .iter()
            .cloned()
            .map(Production::from)
            .collect();

        let values: Vec<String> = pipeline_productions
            .iter()
            .map(|production| production.value.clone())
            .collect();

        CompiledPipeline {
            rule: Arc::new(Rule::new(base_productions.clone())).pipeline(Self::LABEL, values),
            bnf_rule: MorphPipelineBNFRule::new(Self::LABEL, base_productions),
            pipeline_productions,
        }
    }

    fn rebind_bnf_rule<'a>(rule: Self::BnfRule<'static>) -> Self::BnfRule<'a> {
        // Safety: compiled BNF rules here contain only owning productions/index keys.
        unsafe { std::mem::transmute::<Self::BnfRule<'static>, Self::BnfRule<'a>>(rule) }
    }
}

/// Морфологический пайплайн (совпадение по нормальным формам слов).
pub type MorphPipeline = GenericPipeline<MorphStrategy>;

/// Схема точного пайплайна: строки задаются заранее; активированный [`Pipeline`] создаётся через
/// [`activate`](PipelineScheme::activate) или [`activate_default`](PipelineScheme::activate_default).
#[derive(Debug, Clone)]
pub struct PipelineScheme {
    /// Исходные строки, из которых строится пайплайн.
    pub lines: Vec<String>,
}

impl PipelineScheme {
    /// Текстовая метка схемы.
    pub const LABEL: &'static str = "[pipeline]";

    /// Создает схему точного пайплайна.
    pub fn new(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: collect_scheme_lines(lines),
        }
    }

    /// Преобразует одну строку в [`Key`] через обычный токенизатор.
    pub fn get_key(&self, line: &str, tokenizer: &Tokenizer) -> Key {
        key_from_tokenizer_line(line, tokenizer)
    }

    /// Активирует схему в [`Pipeline`] с заданным токенизатором.
    pub fn activate(&self, tokenizer: &Tokenizer) -> Pipeline {
        Pipeline::new(tokenizer_scheme_keys(&self.lines, tokenizer))
    }

    /// Активирует схему с токенизатором по умолчанию.
    pub fn activate_default(&self) -> Pipeline {
        let tokenizer = Tokenizer::new();
        self.activate(&tokenizer)
    }

    /// Немедленно преобразует схему в [`Rule`].
    pub fn into_rule<'a>(&self) -> Arc<Rule<'a>> {
        self.activate_default().into_rule()
    }
}

#[derive(Debug, Clone)]
pub struct CaselessPipelineScheme {
    /// Исходные строки, из которых строится пайплайн.
    pub lines: Vec<String>,
}

impl CaselessPipelineScheme {
    /// Текстовая метка схемы.
    pub const LABEL: &'static str = "[caseless_pipeline]";

    /// Создает схему caseless-пайплайна.
    pub fn new(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: collect_scheme_lines(lines),
        }
    }

    /// Преобразует одну строку в [`Key`] через обычный токенизатор.
    pub fn get_key(&self, line: &str, tokenizer: &Tokenizer) -> Key {
        key_from_tokenizer_line(line, tokenizer)
    }

    /// Активирует схему в [`CaselessPipeline`] с заданным токенизатором.
    pub fn activate(&self, tokenizer: &Tokenizer) -> CaselessPipeline {
        CaselessPipeline::new(tokenizer_scheme_keys(&self.lines, tokenizer))
    }

    /// Активирует схему с токенизатором по умолчанию.
    pub fn activate_default(&self) -> CaselessPipeline {
        let tokenizer = Tokenizer::new();
        self.activate(&tokenizer)
    }

    /// Немедленно преобразует схему в [`Rule`].
    pub fn into_rule<'a>(&self) -> Arc<Rule<'a>> {
        self.activate_default().into_rule()
    }
}

/// Схема морфологического пайплайна: ключи строятся через [`MorphTokenizer`](crate::token::MorphTokenizer), итог — [`MorphPipeline`].
#[derive(Debug, Clone)]
pub struct MorphPipelineScheme {
    /// Исходные строки, из которых строится пайплайн.
    pub lines: Vec<String>,
}

impl MorphPipelineScheme {
    /// Текстовая метка схемы.
    pub const LABEL: &'static str = "[morph_pipeline]";

    /// Создает схему морфологического пайплайна.
    pub fn new(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: collect_scheme_lines(lines),
        }
    }

    fn get_key_with_cache(
        &self,
        line: &str,
        tokenizer: &MorphTokenizer,
        normalized_cache: &mut HashMap<String, Vec<String>>,
    ) -> Key {
        let parts = tokenizer.split(line);
        let terms: Vec<Vec<String>> = parts
            .into_iter()
            .map(|part| {
                let term = part.as_ref();
                if let Some(cached) = normalized_cache.get(term) {
                    return cached.clone();
                }

                let normalized: Vec<String> =
                    tokenizer.morph.normalized_set(term).into_iter().collect();
                let variants = if normalized.is_empty() {
                    vec![term.to_lowercase()]
                } else {
                    normalized
                };
                normalized_cache.insert(term.to_string(), variants.clone());
                variants
            })
            .collect();

        Key::new(line.to_string(), terms)
    }

    /// Преобразует одну строку в [`Key`] через морфологический токенизатор.
    ///
    /// Для каждого терма формируется набор нормальных форм; если они отсутствуют,
    /// используется lowercase-форма исходного терма.
    pub fn get_key(&self, line: &str, tokenizer: &MorphTokenizer) -> Key {
        self.get_key_with_cache(line, tokenizer, &mut HashMap::new())
    }

    /// Активирует схему в [`MorphPipeline`] с заданным морфологическим токенизатором.
    pub fn activate(&self, tokenizer: &MorphTokenizer) -> MorphPipeline {
        let mut normalized_cache: HashMap<String, Vec<String>> = HashMap::new();
        MorphPipeline::new(
            self.lines
                .iter()
                .map(|line| self.get_key_with_cache(line, tokenizer, &mut normalized_cache)),
        )
    }

    /// Активирует схему с глобальным морфологическим токенизатором.
    pub fn activate_default(&self) -> MorphPipeline {
        let tokenizer = global_morph_tokenizer();
        self.activate(&tokenizer)
    }

    /// Преобразует схему в [`Rule`].
    pub fn into_rule<'a>(&self) -> Arc<Rule<'a>> {
        self.activate_default().into_rule()
    }
}

/// Создает схему точного пайплайна из списка строк.
pub fn pipeline_scheme<I, S>(lines: I) -> PipelineScheme
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    PipelineScheme::new(lines.into_iter().map(|line| line.as_ref().to_string()))
}

/// Создает схему пайплайна без учета регистра.
pub fn caseless_pipeline_scheme<I, S>(lines: I) -> CaselessPipelineScheme
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    CaselessPipelineScheme::new(lines.into_iter().map(|line| line.as_ref().to_string()))
}

/// Создает схему морфологического пайплайна.
pub fn morph_pipeline_scheme<I, S>(lines: I) -> MorphPipelineScheme
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    MorphPipelineScheme::new(lines.into_iter().map(|line| line.as_ref().to_string()))
}

/// Быстрый конструктор точного пайплайна, сразу возвращающий [`Rule`].
pub fn pipeline<'a, I, S>(lines: I) -> Arc<Rule<'a>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    pipeline_scheme(lines).into_rule()
}

/// Быстрый конструктор caseless-пайплайна, сразу возвращающий [`Rule`].
pub fn caseless_pipeline<'a, I, S>(lines: I) -> Arc<Rule<'a>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    caseless_pipeline_scheme(lines).into_rule()
}

/// Быстрый конструктор морфологического пайплайна, сразу возвращающий [`Rule`].
pub fn morph_pipeline<'a, I, S>(lines: I) -> Arc<Rule<'a>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    morph_pipeline_scheme(lines).into_rule()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{caseless_pipeline, morph_pipeline, pipeline};
    use crate::parser::Parser;
    use crate::predicates::constructors::eq;
    use crate::rule::constructors::{Production, Rule, TermOrMain};
    use crate::rule::registry::RuleRegistry;

    fn rule_with_suffix<'a>(prefix: Arc<Rule<'a>>, suffix: &'a str) -> Arc<Rule<'a>> {
        Arc::new(Rule::new(vec![Production::new(
            vec![TermOrMain::from(prefix), TermOrMain::from(eq(suffix))],
            None,
        )]))
    }

    #[test]
    fn test_pipeline_basic_and_repeatable() {
        let rule_1 = rule_with_suffix(pipeline(["a b c", "b c"]), "d");

        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule_1);

        let parser = Parser::new(&registry, root_id);
        assert!(parser.r#match("b c d").is_some());
        assert!(parser.r#match("a b c d").is_some());

        let repeated = pipeline(["a b"]).repeatable(None, None, false);
        let rule_2 = rule_with_suffix(repeated, "c");
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule_2);
        let parser = Parser::new(&registry, root_id);
        assert!(parser.r#match("a b a b c").is_some());
    }

    #[test]
    fn test_pipeline_reuses_compiled_rule() {
        let pipeline = super::Pipeline::new([super::Key::simple(
            "a b",
            vec!["a".to_string(), "b".to_string()],
        )]);

        let rule_1 = pipeline.into_rule();
        let rule_2 = pipeline.into_rule();

        assert!(Arc::ptr_eq(&rule_1, &rule_2));
    }

    #[test]
    fn test_caseless_pipeline_matches_case_insensitive() {
        let rule = rule_with_suffix(caseless_pipeline(["A B"]), "c");
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule);

        let parser = Parser::new(&registry, root_id);
        assert!(parser.r#match("A b c").is_some());
    }

    #[test]
    fn test_morph_pipeline_prefers_longest_match() {
        let rule = morph_pipeline([
            "текст",
            "текст песни",
            "материал",
            "информационный материал",
        ]);
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule);

        let parser = Parser::new(&registry, root_id);

        let matches = parser.findall("текстом песни музыкальной группы");
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0]
                .tokens()
                .into_iter()
                .map(|token| token.value.into_owned())
                .collect::<Vec<_>>(),
            vec!["текстом".to_string(), "песни".to_string()]
        );

        let matches = parser.findall("информационного материала под названием");
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0]
                .tokens()
                .into_iter()
                .map(|token| token.value.into_owned())
                .collect::<Vec<_>>(),
            vec!["информационного".to_string(), "материала".to_string()]
        );
    }

    #[test]
    fn test_morph_pipeline_handles_non_alpha_tokens() {
        let rule = morph_pipeline(["1 B."]);
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule);

        let parser = Parser::new(&registry, root_id);
        assert!(parser.r#match("1 b .").is_some());
    }

    #[test]
    fn test_morph_pipeline_electronic_diary_forms() {
        let rule = morph_pipeline(["электронный дневник"]);
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule);

        let parser = Parser::new(&registry, root_id);
        let matches =
            parser.findall("электронным дневником, электронные дневники, электронное дневнику");
        let got: Vec<Vec<String>> = matches
            .into_iter()
            .map(|m| {
                m.tokens()
                    .into_iter()
                    .map(|token| token.value.into_owned())
                    .collect()
            })
            .collect();

        assert_eq!(
            got,
            vec![
                vec!["электронным".to_string(), "дневником".to_string()],
                vec!["электронные".to_string(), "дневники".to_string()],
                vec!["электронное".to_string(), "дневнику".to_string()],
            ]
        );
    }

    #[test]
    fn test_caseless_pipeline_names_with_hyphen() {
        let rule = caseless_pipeline(["Абд Аль-Азиз Бин Мухаммад", "Абд ар-Рахман Наср ас-Са ди"]);
        let mut registry = RuleRegistry::new();
        let root_id = registry.add(rule);

        let parser = Parser::new(&registry, root_id);
        let text = "Абд Аль-Азиз Бин Мухаммад, АБД АР-РАХМАН НАСР АС-СА ДИ";
        let matches = parser.findall(text);
        let got: Vec<Vec<String>> = matches
            .into_iter()
            .map(|m| {
                m.tokens()
                    .into_iter()
                    .map(|token| token.value.into_owned())
                    .collect()
            })
            .collect();

        assert_eq!(
            got,
            vec![
                vec![
                    "Абд".to_string(),
                    "Аль".to_string(),
                    "-".to_string(),
                    "Азиз".to_string(),
                    "Бин".to_string(),
                    "Мухаммад".to_string(),
                ],
                vec![
                    "АБД".to_string(),
                    "АР".to_string(),
                    "-".to_string(),
                    "РАХМАН".to_string(),
                    "НАСР".to_string(),
                    "АС".to_string(),
                    "-".to_string(),
                    "СА".to_string(),
                    "ДИ".to_string(),
                ],
            ]
        );
    }

    #[test]
    fn test_pipeline_scheme_keeps_duplicate_lines_in_keys() {
        let scheme =
            super::PipelineScheme::new(vec!["a b".to_string(), "a b".to_string(), "c".to_string()]);

        let activated = scheme.activate(&crate::token::Tokenizer::new());
        let values: Vec<String> = activated.keys.into_iter().map(|k| k.value).collect();
        assert_eq!(
            values,
            vec!["a b".to_string(), "a b".to_string(), "c".to_string()]
        );
    }
}
