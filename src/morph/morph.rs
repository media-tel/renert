//! Морфологический анализ: адаптер над `morph-rs`.
//!
//! Этот модуль реализует тонкий слой-адаптер над [`morph_rs::MorphAnalyzer`]:
//!
//! - `parse(word) -> Vec<Form>` — все возможные разборы слова
//! - `normalized_set(word) -> Set<lemma>` — все нормальные формы
//! - `check_gram(gram)` — валидация граммем
//! - [`CachedMorphAnalyzer`] — LRU-кеш поверх анализа
//!
//! ## Связь с другими модулями
//!
//! - [`Form`] и [`Grams`] определены в `morph::models`
//! - путь к словарю определяется в `morph::dict_loader`
//! - [`MorphTokenizer`](crate::token::MorphTokenizer) использует этот модуль для обогащения токенов
//!
//! ```text
//! text
//!   ↓
//! Tokenizer
//!   ↓
//! MorphTokenizer
//!   ↓
//! MorphAnalyzer / CachedMorphAnalyzer
//!   ↓
//! Vec<Form>  ← используется предикатами грамматики
//! ```

use once_cell::sync::Lazy;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use parking_lot::Mutex;

use morph_rs;
use morph_rs::morph::grammemes::Grammem;

use std::path::Path;

use crate::error::MorphError;
use crate::morph::dict_loader::dict_dir_default;
use crate::morph::models::{Form, Grams};

/// Преобразует граммему morph-rs в канонический строковый код OpenCorpora.
///
/// Например:
/// - `Grammem::Case(Nominativus)` → `"nomn"`
/// - `Grammem::POS(Noun)` → `"NOUN"`
///
/// Используется для унификации представления граммем в [`Grams`].
fn grammem_to_opencorpora(g: &Grammem) -> &'static str {
    use morph_rs::morph::grammemes::*;
    match g {
        Grammem::ParteSpeech(p) => match p {
            ParteSpeech::Noun => "NOUN",
            ParteSpeech::AdjectiveFull => "ADJF",
            ParteSpeech::AdjectiveShort => "ADJS",
            ParteSpeech::Comparative => "COMP",
            ParteSpeech::Verb => "VERB",
            ParteSpeech::Infinitive => "INFN",
            ParteSpeech::ParticipleFull => "PRTF",
            ParteSpeech::ParticipleShort => "PRTS",
            ParteSpeech::Gerundive => "GRND",
            ParteSpeech::Number => "NUMR",
            ParteSpeech::Adverb => "ADVB",
            ParteSpeech::NounPronoun => "NPRO",
            ParteSpeech::Predicative => "PRED",
            ParteSpeech::Preposition => "PREP",
            ParteSpeech::Conjunction => "CONJ",
            ParteSpeech::Particle => "PRCL",
            ParteSpeech::Interjection => "INTJ",
        },
        Grammem::Animacy(a) => match a {
            Animacy::Animate => "anim",
            Animacy::Inanimate => "inan",
            Animacy::Both => "Inmx",
        },
        Grammem::Aspect(a) => match a {
            Aspect::Perfetto => "perf",
            Aspect::Imperfetto => "impf",
        },
        Grammem::Case(c) => match c {
            Case::Fixed => "Fixd",
            Case::Nominativus => "nomn",
            Case::Genetivus => "gent",
            Case::Dativus => "datv",
            Case::Accusativus => "accs",
            Case::Ablativus => "ablt",
            Case::Locativus => "loct",
            Case::Vocativus => "voct",
            Case::Gen2 => "gen2",
            Case::Acc2 => "acc2",
            Case::Loc2 => "loc2",
        },
        Grammem::Gender(g) => match g {
            Gender::Masculine => "masc",
            Gender::Feminine => "femn",
            Gender::Neutral => "neut",
            Gender::Common => "ms-f",
            Gender::CommonWavering => "Ms-f",
            Gender::GenderNeutral => "GNdr",
        },
        Grammem::Involvement(i) => match i {
            Involvement::Incluso => "incl",
            Involvement::Excluso => "excl",
        },
        Grammem::Mood(m) => match m {
            Mood::Indicativo => "indc",
            Mood::Imperativo => "impr",
        },
        Grammem::Number(n) => match n {
            Number::Singular => "sing",
            Number::Plural => "plur",
            Number::SingulariaTantum => "Sgtm",
            Number::PluraliaTantum => "Pltm",
        },
        Grammem::Trans(t) => match t {
            Transitivity::Transitive => "tran",
            Transitivity::Intransitive => "intr",
        },
        Grammem::Tense(t) => match t {
            Tense::Past => "past",
            Tense::Present => "pres",
            Tense::Future => "futr",
        },
        Grammem::Voice(v) => match v {
            Voice::Active => "actv",
            Voice::Passive => "pssv",
        },
        Grammem::Person(p) => match p {
            Person::First => "1per",
            Person::Second => "2per",
            Person::Third => "3per",
            Person::Impersonal => "Impe",
            Person::PossibleImpersonal => "Impx",
        },
        Grammem::Other(o) => match o {
            Other::Abbreviation => "Abbr",
            Other::Name => "Name",
            Other::Surname => "Surn",
            Other::Patronymic => "Patr",
            Other::Geography => "Geox",
            Other::Organization => "Orgn",
            Other::Trademark => "Trad",
            Other::PossibleSubstantive => "Subx",
            Other::Superior => "Supr",
            Other::Quality => "Qual",
            Other::Pronominal => "Apro",
            Other::Ordinal => "Anum",
            Other::Possessive => "Poss",
            Other::Questionable => "Ques",
            Other::Demonstrative => "Dmns",
            Other::Anaphoric => "Anph",
            Other::Comparative => "Cmp2",
            Other::FormEY => "V-ey",
            Other::FormOY => "V-oy",
            Other::FormEJ => "V-ej",
            Other::FormBE => "V-be",
            Other::FormENEN => "V-en",
            Other::FormIE => "V-ie",
            Other::FormBI => "V-bi",
            Other::ParticipleSH => "V-sh",
            Other::Multiple => "Mult",
            Other::Reflessivo => "Refl",
            Other::Spoken => "Infr",
            Other::Slang => "Slng",
            Other::Archaic => "Arch",
            Other::Literary => "Litr",
            Other::Error => "Erro",
            Other::Distortion => "Dist",
            Other::Parenthesis => "Prnt",
            Other::ImperfectiveParticiple => "Fimp",
            Other::PossiblePredicative => "Prdx",
            Other::Countable => "Coun",
            Other::Collection => "Coll",
            Other::AfterPreposition => "Af-p",
            Other::PrepositionVariant => "Vpre",
            Other::Initial => "Init",
            Other::PossibleAdjective => "Adjx",
            Other::Hypothetical => "Hypo",
            Other::Other => "Other",
        },
    }
}

/// Обратное преобразование: код OpenCorpora → [`Grammem`].
///
/// Возвращает `None` для неизвестных строк.
pub fn opencorpora_to_grammem(s: &str) -> Option<Grammem> {
    use morph_rs::morph::grammemes::*;
    let g = match s {
        "NOUN" => Grammem::ParteSpeech(ParteSpeech::Noun),
        "ADJF" => Grammem::ParteSpeech(ParteSpeech::AdjectiveFull),
        "ADJS" => Grammem::ParteSpeech(ParteSpeech::AdjectiveShort),
        "COMP" => Grammem::ParteSpeech(ParteSpeech::Comparative),
        "VERB" => Grammem::ParteSpeech(ParteSpeech::Verb),
        "INFN" => Grammem::ParteSpeech(ParteSpeech::Infinitive),
        "PRTF" => Grammem::ParteSpeech(ParteSpeech::ParticipleFull),
        "PRTS" => Grammem::ParteSpeech(ParteSpeech::ParticipleShort),
        "GRND" => Grammem::ParteSpeech(ParteSpeech::Gerundive),
        "NUMR" => Grammem::ParteSpeech(ParteSpeech::Number),
        "ADVB" => Grammem::ParteSpeech(ParteSpeech::Adverb),
        "NPRO" => Grammem::ParteSpeech(ParteSpeech::NounPronoun),
        "PRED" => Grammem::ParteSpeech(ParteSpeech::Predicative),
        "PREP" => Grammem::ParteSpeech(ParteSpeech::Preposition),
        "CONJ" => Grammem::ParteSpeech(ParteSpeech::Conjunction),
        "PRCL" => Grammem::ParteSpeech(ParteSpeech::Particle),
        "INTJ" => Grammem::ParteSpeech(ParteSpeech::Interjection),
        "anim" => Grammem::Animacy(Animacy::Animate),
        "inan" => Grammem::Animacy(Animacy::Inanimate),
        "Inmx" => Grammem::Animacy(Animacy::Both),
        "perf" => Grammem::Aspect(Aspect::Perfetto),
        "impf" => Grammem::Aspect(Aspect::Imperfetto),
        "Fixd" => Grammem::Case(Case::Fixed),
        "nomn" => Grammem::Case(Case::Nominativus),
        "gent" => Grammem::Case(Case::Genetivus),
        "datv" => Grammem::Case(Case::Dativus),
        "accs" => Grammem::Case(Case::Accusativus),
        "ablt" => Grammem::Case(Case::Ablativus),
        "loct" => Grammem::Case(Case::Locativus),
        "voct" => Grammem::Case(Case::Vocativus),
        "gen2" => Grammem::Case(Case::Gen2),
        "acc2" => Grammem::Case(Case::Acc2),
        "loc2" => Grammem::Case(Case::Loc2),
        "masc" => Grammem::Gender(Gender::Masculine),
        "femn" => Grammem::Gender(Gender::Feminine),
        "neut" => Grammem::Gender(Gender::Neutral),
        "ms-f" => Grammem::Gender(Gender::Common),
        "Ms-f" => Grammem::Gender(Gender::CommonWavering),
        "GNdr" => Grammem::Gender(Gender::GenderNeutral),
        "incl" => Grammem::Involvement(Involvement::Incluso),
        "excl" => Grammem::Involvement(Involvement::Excluso),
        "indc" => Grammem::Mood(Mood::Indicativo),
        "impr" => Grammem::Mood(Mood::Imperativo),
        "sing" => Grammem::Number(Number::Singular),
        "plur" => Grammem::Number(Number::Plural),
        "Sgtm" => Grammem::Number(Number::SingulariaTantum),
        "Pltm" => Grammem::Number(Number::PluraliaTantum),
        "tran" => Grammem::Trans(Transitivity::Transitive),
        "intr" => Grammem::Trans(Transitivity::Intransitive),
        "past" => Grammem::Tense(Tense::Past),
        "pres" => Grammem::Tense(Tense::Present),
        "futr" => Grammem::Tense(Tense::Future),
        "actv" => Grammem::Voice(Voice::Active),
        "pssv" => Grammem::Voice(Voice::Passive),
        "1per" => Grammem::Person(Person::First),
        "2per" => Grammem::Person(Person::Second),
        "3per" => Grammem::Person(Person::Third),
        "Impe" => Grammem::Person(Person::Impersonal),
        "Impx" => Grammem::Person(Person::PossibleImpersonal),
        "Abbr" => Grammem::Other(Other::Abbreviation),
        "Name" => Grammem::Other(Other::Name),
        "Surn" => Grammem::Other(Other::Surname),
        "Patr" => Grammem::Other(Other::Patronymic),
        "Geox" => Grammem::Other(Other::Geography),
        "Orgn" => Grammem::Other(Other::Organization),
        "Trad" => Grammem::Other(Other::Trademark),
        "Subx" => Grammem::Other(Other::PossibleSubstantive),
        "Supr" => Grammem::Other(Other::Superior),
        "Qual" => Grammem::Other(Other::Quality),
        "Apro" => Grammem::Other(Other::Pronominal),
        "Anum" => Grammem::Other(Other::Ordinal),
        "Poss" => Grammem::Other(Other::Possessive),
        "Ques" => Grammem::Other(Other::Questionable),
        "Dmns" => Grammem::Other(Other::Demonstrative),
        "Anph" => Grammem::Other(Other::Anaphoric),
        "Cmp2" => Grammem::Other(Other::Comparative),
        "V-ey" => Grammem::Other(Other::FormEY),
        "V-oy" => Grammem::Other(Other::FormOY),
        "V-ej" => Grammem::Other(Other::FormEJ),
        "V-be" => Grammem::Other(Other::FormBE),
        "V-en" => Grammem::Other(Other::FormENEN),
        "V-ie" => Grammem::Other(Other::FormIE),
        "V-bi" => Grammem::Other(Other::FormBI),
        "V-sh" => Grammem::Other(Other::ParticipleSH),
        "Mult" => Grammem::Other(Other::Multiple),
        "Refl" => Grammem::Other(Other::Reflessivo),
        "Infr" => Grammem::Other(Other::Spoken),
        "Slng" => Grammem::Other(Other::Slang),
        "Arch" => Grammem::Other(Other::Archaic),
        "Litr" => Grammem::Other(Other::Literary),
        "Erro" => Grammem::Other(Other::Error),
        "Dist" => Grammem::Other(Other::Distortion),
        "Prnt" => Grammem::Other(Other::Parenthesis),
        "Fimp" => Grammem::Other(Other::ImperfectiveParticiple),
        "Prdx" => Grammem::Other(Other::PossiblePredicative),
        "Coun" => Grammem::Other(Other::Countable),
        "Coll" => Grammem::Other(Other::Collection),
        "Af-p" => Grammem::Other(Other::AfterPreposition),
        "Vpre" => Grammem::Other(Other::PrepositionVariant),
        "Init" => Grammem::Other(Other::Initial),
        "Adjx" => Grammem::Other(Other::PossibleAdjective),
        "Hypo" => Grammem::Other(Other::Hypothetical),
        _ => return None,
    };
    Some(g)
}

/// Парсит список OpenCorpora-кодов граммем в `Vec<Grammem>`.
///
/// Возвращает `None`, если `grams` отсутствует или все значения невалидны.
pub(crate) fn parse_opencorpora_grammemes(grams: Option<&[String]>) -> Option<Vec<Grammem>> {
    grams.and_then(|values| {
        let parsed: Vec<Grammem> = values
            .iter()
            .filter_map(|gram| opencorpora_to_grammem(gram))
            .collect();

        if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        }
    })
}

/// Преобразует `ParsedWord` из morph-rs в [`Form`].
///
/// - `normalized` ← `raw.normal_form()`
/// - `grams` ← все граммемы в формате OpenCorpora
/// - `raw` ← сохраняется для последующей инфлекции
fn prepare_form(raw: morph_rs::ParsedWord) -> Form {
    let normalized = raw.normal_form().to_lowercase();

    let grams = raw
        .tag()
        .into_iter()
        .map(|g| grammem_to_opencorpora(&g).to_owned())
        .collect::<BTreeSet<_>>();

    Form::new(normalized, Grams { values: grams }, Some(raw))
}

/// Размер LRU-кеша для [`CachedMorphAnalyzer`].
pub const CACHE_SIZE: usize = 10_000;

static CACHE_CAP: Lazy<NonZeroUsize> =
    Lazy::new(|| NonZeroUsize::new(CACHE_SIZE).expect("CACHE_SIZE must be > 0"));

/// Адаптер над [`morph_rs::MorphAnalyzer`], который:
/// - открывает словарь по стандартному пути проекта
/// - возвращает результаты в виде [`Form`]
/// - предоставляет yargy-like API
pub struct MorphAnalyzer {
    /// Внутренний анализатор `morph-rs`.
    pub analyzer: morph_rs::MorphAnalyzer,
}

impl MorphAnalyzer {
    /// Открывает словарь из указанного каталога (должен содержать `dict.json` и `dict.fst`).
    pub fn open_at(dir: impl AsRef<Path>) -> Result<Self, MorphError> {
        let dir = dir.as_ref();
        morph_rs::MorphAnalyzer::open(dir)
            .map(|analyzer| Self { analyzer })
            .map_err(|e| MorphError::OpenDictFailed {
                path: dir.to_path_buf(),
                source: Box::new(e),
            })
    }

    /// Открывает морфологический словарь проекта.
    ///
    /// Путь определяется через [`dict_dir_default`].
    ///
    /// ## Ошибки
    /// Возвращает `Err`, если словарь не найден или не может быть открыт.
    pub fn open() -> Result<Self, MorphError> {
        Self::open_at(dict_dir_default()?)
    }

    /// Аналог `morph(word)` / `__call__(word)` в yargy.
    ///
    /// Возвращает **все возможные разборы** слова.
    /// Если `morph-rs` вернул ошибку парсинга, возвращается пустой вектор.
    ///
    /// ```rust,no_run
    /// use renert::morph::morph::MorphAnalyzer;
    ///
    /// # use renert::error;
    /// # fn main() -> error::Result<()> {
    /// let m = MorphAnalyzer::open()?;
    /// let forms = m.parse("Ленина");
    ///
    /// assert!(!forms.is_empty());
    /// for f in &forms {
    ///     println!("lemma={}, grams={:?}", f.normalized, f.grams.values);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse(&self, word: &str) -> Vec<Form> {
        let parsed = match self.analyzer.parse(word) {
            Ok(pw) => pw,
            Err(_) => return vec![],
        };

        parsed.0.into_iter().map(prepare_form).collect()
    }

    /// Аналог `morph.normalized(word)` в yargy.
    ///
    /// Возвращает множество всех возможных лемм слова.
    ///
    /// ```rust,no_run
    /// use renert::morph::morph::MorphAnalyzer;
    ///
    /// # use renert::error;
    /// # fn main() -> error::Result<()> {
    /// let m = MorphAnalyzer::open()?;
    /// let norms = m.normalized_set("Ленина");
    ///
    /// assert!(!norms.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn normalized_set(&self, word: &str) -> BTreeSet<String> {
        match self.analyzer.normalize(word) {
            Ok(nw) => nw.0.into_iter().map(|w| w.word()).collect(),
            Err(_) => BTreeSet::new(),
        }
    }

    /// Проверяет корректность граммемы.
    ///
    /// Используется при инициализации правил и предикатов,
    /// чтобы ловить ошибки заранее.
    ///
    /// ```rust,no_run
    /// use renert::morph::morph::MorphAnalyzer;
    /// use renert::error;
    ///
    /// fn main() -> error::Result<()> {
    /// let m = MorphAnalyzer::open()?;
    /// assert!(m.check_gram("nomn").is_ok());
    /// assert!(m.check_gram("bad-gram").is_err());
    /// Ok(())
    /// }
    /// ```
    pub fn check_gram(&self, gram: &str) -> Result<(), MorphError> {
        let input_json = format!("\"{}\"", gram);
        let parsed: Grammem =
            serde_json::from_str(&input_json).map_err(|_| MorphError::InvalidGrammeme {
                grammem: gram.to_string(),
            })?;

        // канонический код (например "NOUN" или "nomn")
        let canon_json =
            serde_json::to_string(&parsed).map_err(|_| MorphError::InvalidGrammeme {
                grammem: gram.to_string(),
            })?;

        if canon_json == input_json {
            Ok(())
        } else {
            Err(MorphError::InvalidGrammeme {
                grammem: gram.to_string(),
            })
        }
    }
}

/// Добавляет LRU-кеш поверх [`MorphAnalyzer::parse`].
/// Значительно ускоряет работу на больших текстах,
/// где одни и те же слова встречаются много раз.
pub struct CachedMorphAnalyzer {
    inner: MorphAnalyzer,
    cache: Mutex<LruCache<String, Arc<[Form]>>>,
}

impl CachedMorphAnalyzer {
    /// Создаёт кеширующий анализатор поверх [`MorphAnalyzer`].
    pub fn new(inner: MorphAnalyzer) -> Self {
        Self {
            inner,
            cache: Mutex::new(LruCache::new(*CACHE_CAP)),
        }
    }

    /// Открывает кеширующий анализатор из указанного каталога.
    pub fn open_at(dir: impl AsRef<Path>) -> Result<Self, MorphError> {
        Ok(Self::new(MorphAnalyzer::open_at(dir)?))
    }

    /// Открывает кеширующий анализатор со стандартным словарём.
    pub fn open() -> Result<Self, MorphError> {
        Ok(Self::new(MorphAnalyzer::open()?))
    }

    /// yargy-like: `morph(word)`
    pub fn call(&self, word: &str) -> Vec<Form> {
        self.parse(word)
    }

    /// yargy-like: `morph.normalized(word)`
    pub fn normalized(&self, word: &str) -> BTreeSet<String> {
        self.normalized_set(word)
    }

    /// Доступ к внутреннему анализатору (для `Form::inflect*`).
    pub fn analyzer(&self) -> &morph_rs::MorphAnalyzer {
        &self.inner.analyzer
    }

    /// Возвращает множество всех возможных лемм слова.
    pub fn normalized_set(&self, word: &str) -> BTreeSet<String> {
        self.inner.normalized_set(word)
    }

    /// Проверяет корректность граммемы.
    ///
    /// Делегирует проверку во внутренний [`MorphAnalyzer`].
    pub fn check_gram(&self, gram: &str) -> Result<(), MorphError> {
        self.inner.check_gram(gram)
    }

    /// Возвращает морфологические разборы слова с использованием кеша.
    pub fn parse(&self, word: &str) -> Vec<Form> {
        {
            let mut cache = self.cache.lock();
            if let Some(arc) = cache.get(word) {
                return arc.to_vec();
            }
        }

        let parsed = self.inner.parse(word);
        let arc: Arc<[Form]> = Arc::from(parsed.as_slice());

        self.cache.lock().put(word.to_string(), arc);

        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static MORPH: Lazy<CachedMorphAnalyzer> =
        Lazy::new(|| CachedMorphAnalyzer::open().expect("Failed to open morph dictionary"));

    fn morph() -> &'static CachedMorphAnalyzer {
        &MORPH
    }

    #[test]
    fn test_morph() {
        let morph = morph();

        // python: morph('сирота')
        let forms = morph.call("сирота");
        assert!(!forms.is_empty());

        // ищем нужный разбор (порядок не гарантирован)
        let expected = ["ms-f", "NOUN", "anim", "nomn", "sing"];
        let form = forms
            .iter()
            .find(|f| f.normalized == "сирота" && expected.iter().all(|g| f.grams.contains(g)))
            .unwrap_or_else(|| {
                panic!(
                    "Expected Form('сирота', {:?}) among parses: {:?}",
                    expected,
                    forms
                        .iter()
                        .map(|f| (&f.normalized, &f.grams.values))
                        .collect::<Vec<_>>()
                )
            });

        let grams = &form.grams;
        assert!(grams.gender().bi);
        assert!(grams.number().single);
        assert!(!grams.case().fixed);

        let values = morph.normalized("стали");
        assert_eq!(
            values,
            BTreeSet::from(["сталь".to_string(), "стать".to_string()])
        );
    }

    #[test]
    fn test_inflect() {
        let morph = morph();

        let forms = morph.call("стола");
        assert!(!forms.is_empty());

        let form = forms
            .iter()
            .find(|f| f.normalized == "стол" && f.grams.contains("NOUN"))
            .unwrap_or(&forms[0]);

        // form.inflect() -> nomn+sing
        assert_eq!(form.inflect_str(morph.analyzer(), &[]), "стол");

        // form.inflect({'nomn','plur'})
        assert_eq!(
            form.inflect_str(morph.analyzer(), &["nomn", "plur"]),
            "столы"
        );
    }

    #[test]
    fn test_check_gram() {
        let morph = morph();

        // Ошибка значений на неизвестную
        assert!(matches!(
            morph.check_gram("verb"),
            Err(MorphError::InvalidGrammeme { .. })
        ));
        assert!(morph.check_gram("nomn").is_ok());
        assert!(morph.check_gram("NOUN").is_ok());
    }

    #[test]
    fn test_grams_contains_and_repr_like_behavior() {
        let morph = morph();

        let forms = morph.call("сирота");
        assert!(!forms.is_empty());

        let f = &forms[0];
        assert!(f.grams.contains("NOUN") || f.grams.contains("VERB") || f.grams.contains("ADJF"));

        // contains должен работать как `'X' in grams`
        assert_eq!(f.grams.contains("NOUN"), f.grams.values.contains("NOUN"));
    }

    #[test]
    fn test_gender_bi_on_sirota() {
        let morph = morph();

        let forms = morph.call("сирота");
        assert!(!forms.is_empty());

        // ищем разбор с "ms-f" (или хотя бы проверим что граммема реально присутствует)
        let form = forms
            .iter()
            .find(|f| f.grams.contains("ms-f") || f.grams.contains("Ms-f"))
            .unwrap_or(&forms[0]);

        let g = form.grams.gender();
        assert!(
            g.bi,
            "Expected bi gender for 'сирота' parse; grams={:?}",
            form.grams.values
        );
    }

    #[test]
    fn test_number_plural_and_single() {
        let morph = morph();

        // SINGLE: "стола" -> обычно singular
        let forms = morph.call("стола");
        assert!(!forms.is_empty());

        let form = forms
            .iter()
            .find(|f| f.grams.contains("sing"))
            .unwrap_or(&forms[0]);

        let n = form.grams.number();
        assert!(
            n.single,
            "Expected sing=true; grams={:?}",
            form.grams.values
        );
        assert!(
            !n.plural,
            "Expected plur=false; grams={:?}",
            form.grams.values
        );

        // PLURAL: "столы" -> обычно plural
        let forms2 = morph.call("столы");
        assert!(!forms2.is_empty());

        let form2 = forms2
            .iter()
            .find(|f| f.grams.contains("plur"))
            .unwrap_or(&forms2[0]);

        let n2 = form2.grams.number();
        assert!(
            n2.plural,
            "Expected plur=true; grams={:?}",
            form2.grams.values
        );
    }

    #[test]
    fn test_case_mask_nomn_and_gent() {
        let morph = morph();

        // "стол" => nomn
        let forms = morph.call("стол");
        assert!(!forms.is_empty());
        let f = forms
            .iter()
            .find(|f| f.grams.contains("nomn"))
            .unwrap_or(&forms[0]);

        let c = f.grams.case();
        assert!(
            c.mask[0],
            "Expected nomn mask[0]=true; grams={:?}",
            f.grams.values
        );

        // "стола" => часто gent (род.п.)
        let forms2 = morph.call("стола");
        assert!(!forms2.is_empty());
        let f2 = forms2
            .iter()
            .find(|f| f.grams.contains("gent"))
            .unwrap_or(&forms2[0]);

        let c2 = f2.grams.case();
        assert!(
            c2.mask[1],
            "Expected gent mask[1]=true; grams={:?}",
            f2.grams.values
        );
    }

    #[test]
    fn test_case_fixed_is_false_for_common_words() {
        let morph = morph();

        // "стол" не должен быть Fixd
        let forms = morph.call("стол");
        assert!(!forms.is_empty());
        let f = &forms[0];

        assert!(
            !f.grams.case().fixed,
            "Expected Fixd=false for 'стол'; grams={:?}",
            f.grams.values
        );
    }

    #[test]
    fn test_form_inflect_roundtrip_nomn_sing() {
        let morph = morph();

        // Берём "стола" -> лемма "стол"
        let forms = morph.call("стола");
        assert!(!forms.is_empty());

        let f = forms
            .iter()
            .find(|f| f.normalized == "стол" && f.grams.contains("NOUN"))
            .unwrap_or(&forms[0]);

        // inflect_str([]) должен вернуть nomn+sing => "стол"
        let base = f.inflect_str(morph.analyzer(), &[]);
        assert_eq!(base, "стол");

        // inflect_str(["gent","sing"]) должен вернуть "стола" (часто ровно так)
        let back = f.inflect_str(morph.analyzer(), &["gent", "sing"]);
        assert_eq!(back, "стола");
    }

    #[test]
    fn test_form_inflect_plural_forms() {
        let morph = morph();

        // "стол" -> nomn plur => "столы", gent plur => "столов"
        let forms = morph.call("стол");
        assert!(!forms.is_empty());

        let f = forms
            .iter()
            .find(|f| f.normalized == "стол" && f.grams.contains("NOUN"))
            .unwrap_or(&forms[0]);

        let nomn_plur = f.inflect_str(morph.analyzer(), &["nomn", "plur"]);
        assert_eq!(nomn_plur, "столы");

        let gent_plur = f.inflect_str(morph.analyzer(), &["gent", "plur"]);
        assert_eq!(gent_plur, "столов");
    }

    #[test]
    fn test_check_gram_rejects_non_canonical() {
        let morph = morph();

        // канонические проходят
        assert!(morph.check_gram("NOUN").is_ok());
        assert!(morph.check_gram("nomn").is_ok());

        // некорректные/неканонические — падают
        assert!(matches!(
            morph.check_gram("noun"),
            Err(MorphError::InvalidGrammeme { .. })
        ));
        assert!(matches!(
            morph.check_gram("Nomn"),
            Err(MorphError::InvalidGrammeme { .. })
        ));
        assert!(matches!(
            morph.check_gram("something_random"),
            Err(MorphError::InvalidGrammeme { .. })
        ));
    }

    #[test]
    fn test_parse_opencorpora_grammemes_filters_invalid_values() {
        let grams = Some(
            &[
                "nomn".to_string(),
                "invalid".to_string(),
                "sing".to_string(),
            ][..],
        );
        let parsed = parse_opencorpora_grammemes(grams).expect("must keep valid grammemes");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_parse_opencorpora_grammemes_none_on_empty_or_invalid_input() {
        assert!(parse_opencorpora_grammemes(None).is_none());
        assert!(parse_opencorpora_grammemes(Some(&["bad".to_string()][..])).is_none());
    }
}
