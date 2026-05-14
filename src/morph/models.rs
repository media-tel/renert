//! Морфологические модели: формы, граммемы и удобные представления.
//!
//! Этот модуль содержит **чистые модели данных**, которые описывают результат
//! морфологического анализа слова:
//!
//! - [`Form`] — один морфологический разбор слова
//! - [`Grams`] — набор граммем (POS, падеж, род, число и т.д.)
//! - [`Gender`], [`Number`], [`Case`] — удобные record-подобные структуры
//!
//! Архитектурно это аналогично `pymorphy2.Parse` и связанных свойств в yargy
//!
//! - `Form.normalized` ≈ `parse.normal_form`
//! - `Form.grams` ≈ `parse.tag`
//! - `Form.raw` ≈ внутренний ParsedWord (нужен для инфлекции)

use morph_rs::morph::grammemes::Grammem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Род (gender) слова.
///
/// Это удобная проекция набора граммем [`Grams`] в булевы флаги.
///
/// Аналог:
/// `gram('masc')`, `gram('femn')`, `gram('GNdr')` и т.п.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gender {
    /// Мужской род (`masc`)
    pub male: bool,
    /// Женский род (`femn`)
    pub female: bool,
    /// Средний род (`neut`)
    pub neutral: bool,
    /// Общий род (`Ms-f` / `ms-f`)
    pub bi: bool,
    /// Обобщённый род (`GNdr`)
    pub general: bool,
}

impl Gender {
    /// Создаёт [`Gender`] из набора граммем.
    fn from_grams(g: &Grams) -> Self {
        Self {
            male: g.contains("masc"),
            female: g.contains("femn"),
            neutral: g.contains("neut"),
            bi: g.contains("Ms-f") || g.contains("ms-f"),
            general: g.contains("GNdr"),
        }
    }
}

/// Число (number) слова.
///
/// Учитывает как обычные формы (`sing` / `plur`),
/// так и специальные категории (`Sgtm`, `Pltm`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Number {
    /// Единственное число (`sing`)
    pub single: bool,
    /// Множественное число (`plur`)
    pub plural: bool,
    /// Только единственное (`Sgtm`)
    pub only_single: bool,
    /// Только множественное (`Pltm`)
    pub only_plural: bool,
}

impl Number {
    /// Создаёт [`Number`] из набора граммем.
    pub fn new(grams: &Grams) -> Self {
        Self {
            single: grams.contains("sing"),
            plural: grams.contains("plur"),
            only_single: grams.contains("Sgtm"),
            only_plural: grams.contains("Pltm"),
        }
    }

    /// Создаёт [`Number`] из набора граммем.
    ///
    /// Это алиас к [`Number::new`], добавлен для читаемости API.
    pub fn from_grams(grams: &Grams) -> Self {
        Self::new(grams)
    }
}

/// Падеж (case) слова.
///
/// Представлен в виде маски из 7 возможных падежей:
/// `nomn, gent, datv, accs, ablt, loct, voct`.
///
/// Также учитывается признак фиксированного падежа (`Fixd`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Маска падежей (в порядке: nomn, gent, datv, accs, ablt, loct, voct)
    pub mask: [bool; 7],
    /// Фиксированный падеж (`Fixd`)
    pub fixed: bool,
}

impl Case {
    /// Создаёт [`Case`] из набора граммем.
    pub fn new(grams: &Grams) -> Self {
        let list = ["nomn", "gent", "datv", "accs", "ablt", "loct", "voct"];
        let mut mask = [false; 7];
        for (i, g) in list.iter().enumerate() {
            mask[i] = grams.contains(g);
        }
        Self {
            mask,
            fixed: grams.contains("Fixd"),
        }
    }

    /// Создаёт [`Case`] из набора граммем.
    ///
    /// Это алиас к [`Case::new`], добавлен для единообразия API.
    pub fn from_grams(grams: &Grams) -> Self {
        Self::new(grams)
    }
}

/// Набор граммем (POS, род, число, падеж и т.д.).
///
/// Это тонкая обёртка над `BTreeSet<String>`, где строки соответствуют
/// кодам OpenCorpora (`"NOUN"`, `"nomn"`, `"masc"`, …).
///
/// Используется внутри [`Form`] и в предикатах грамматики.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Grams {
    /// Множество граммем в кодировке OpenCorpora (`"NOUN"`, `"nomn"`, ...).
    pub values: BTreeSet<String>,
}

impl Grams {
    /// Создаёт набор граммем из итератора строк.
    pub fn new<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            values: iter.into_iter().map(Into::into).collect(),
        }
    }

    /// Пустой набор граммем.
    pub fn empty() -> Self {
        Self {
            values: BTreeSet::new(),
        }
    }

    /// Проверяет наличие граммемы.
    pub fn contains(&self, gram: &str) -> bool {
        self.values.contains(gram)
    }

    /// Возвращает род слова.
    pub fn gender(&self) -> Gender {
        Gender::from_grams(self)
    }

    /// Возвращает число слова.
    pub fn number(&self) -> Number {
        Number::from_grams(self)
    }

    /// Возвращает падеж слова.
    pub fn case(&self) -> Case {
        Case::from_grams(self)
    }
}

impl Default for Grams {
    fn default() -> Self {
        Self::empty()
    }
}

/// Морфологическая форма слова.
///
/// Это **один** вариант разбора слова.
/// Для одного токена обычно существует несколько `Form`.
///
/// Поля:
/// - `normalized` — лемма (normal form), приведённая к lowercase
/// - `grams` — набор граммем
/// - `raw` — внутренний `ParsedWord` из `morph-rs` (нужен для инфлекции)
///
/// Примечание: `raw` пропускается при сериализации, т.к. используется только во время работы.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Form {
    /// Лемма (normal form).
    pub normalized: String,
    /// Граммемы формы.
    pub grams: Grams,

    /// Внутренний разбор morph-rs (нужен для inflect).
    #[serde(skip)]
    pub raw: Option<morph_rs::ParsedWord>,
}

impl Form {
    /// Создаёт новую морфологическую форму.
    pub fn new(normalized: String, grams: Grams, raw: Option<morph_rs::ParsedWord>) -> Self {
        Self {
            normalized,
            grams,
            raw,
        }
    }

    /// Склоняет форму в указанные граммемы.
    ///
    /// Если `grams == None`, используется поведение по умолчанию:
    /// `nomn + sing` (именительный падеж, единственное число).
    ///
    /// Если `raw == None`, инфлекция невозможна и возвращается `normalized`.
    pub fn inflect(
        &self,
        analyzer: &morph_rs::MorphAnalyzer,
        grams: Option<Vec<Grammem>>,
    ) -> String {
        let Some(parsed) = self.raw.clone() else {
            return self.normalized.clone();
        };

        let target = grams.unwrap_or_else(|| {
            vec![
                Grammem::Case(morph_rs::morph::grammemes::Case::Nominativus),
                Grammem::Number(morph_rs::morph::grammemes::Number::Singular),
            ]
        });

        let mut result = match analyzer.inflect_parsed(parsed, target.clone()) {
            Ok(Some(words)) => words
                .0
                .first()
                .map(|w| w.word().to_lowercase())
                .unwrap_or_else(|| self.normalized.clone()),
            _ => self.normalized.clone(),
        };
        // Parsed inflection can keep source-form constraints for adjectives.
        // If target has explicit gender and parsed path falls back to lemma,
        // use lemma-based inflection as a more specific variant.
        let has_gender = target.iter().any(|g| matches!(g, Grammem::Gender(_)));
        if has_gender && result == self.normalized {
            let alt_from_lemma = analyzer
                .inflect_forms(&self.normalized, target)
                .ok()
                .flatten()
                .and_then(|words| words.0.first().map(|w| w.word().to_lowercase()));
            if let Some(alt) = alt_from_lemma {
                result = alt;
            }
        }
        result
    }

    /// Склоняет форму по строковым граммемам (удобно для DSL).
    ///
    /// Аналог `form.inflect({'nomn', 'plur'})` в Python.
    ///
    /// Если список пуст — используется поведение по умолчанию (`nomn + sing`).
    /// Если граммема некорректна — возвращается `normalized`.
    pub fn inflect_str(&self, analyzer: &morph_rs::MorphAnalyzer, grams: &[&str]) -> String {
        if grams.is_empty() {
            return self.inflect(analyzer, None);
        }

        let mut v: Vec<Grammem> = Vec::with_capacity(grams.len());
        for g in grams {
            let json = format!("\"{}\"", g);
            let parsed = serde_json::from_str::<Grammem>(&json).ok();

            match parsed {
                Some(p) => v.push(p),
                None => return self.normalized.clone(),
            }
        }

        self.inflect(analyzer, Some(v))
    }
}

#[cfg(test)]
mod tests {
    use super::super::dict_loader::dict_dir_default;
    use super::*;
    use once_cell::sync::Lazy;

    static ANALYZER: Lazy<morph_rs::MorphAnalyzer> = Lazy::new(|| {
        let dir = dict_dir_default().expect("Failed to locate morph dictionary for tests");
        morph_rs::MorphAnalyzer::open(dir).expect("Failed to open morph dictionary for tests")
    });

    fn grammem_to_opencorpora(g: &Grammem) -> String {
        let s = serde_json::to_string(g).expect("Grammem must be serializable");
        s.trim_matches('"').to_string()
    }

    fn parse_form(word: &str, normalized: &str) -> Form {
        let parsed = ANALYZER.parse(word).expect("word should parse");
        let raw = parsed
            .0
            .into_iter()
            .find(|p| p.normal_form().eq_ignore_ascii_case(normalized))
            .expect("expected parse with requested normalized form");
        let grams = raw
            .tag()
            .into_iter()
            .map(|g| grammem_to_opencorpora(&g))
            .collect::<BTreeSet<_>>();
        Form::new(
            raw.normal_form().to_lowercase(),
            Grams { values: grams },
            Some(raw),
        )
    }

    #[test]
    fn test_grams_new_contains() {
        let grams = Grams::new(vec!["NOUN", "nomn", "sing"]);
        assert!(grams.contains("NOUN"));
        assert!(grams.contains("nomn"));
        assert!(grams.contains("sing"));
        assert!(!grams.contains("plur"));
    }

    #[test]
    fn test_gender_from_grams() {
        let grams = Grams::new(vec!["masc", "femn", "neut", "ms-f", "GNdr"]);
        let g = grams.gender();
        assert!(g.male);
        assert!(g.female);
        assert!(g.neutral);
        assert!(g.bi);
        assert!(g.general);
    }

    #[test]
    fn test_number_from_grams() {
        let grams = Grams::new(vec!["sing", "plur", "Sgtm", "Pltm"]);
        let n = grams.number();
        assert!(n.single);
        assert!(n.plural);
        assert!(n.only_single);
        assert!(n.only_plural);
    }

    #[test]
    fn test_case_from_grams() {
        let grams = Grams::new(vec!["nomn", "gent", "Fixd"]);
        let c = grams.case();
        assert!(c.mask[0]);
        assert!(c.mask[1]);
        assert!(!c.mask[2]);
        assert!(!c.mask[3]);
        assert!(!c.mask[4]);
        assert!(!c.mask[5]);
        assert!(!c.mask[6]);
        assert!(c.fixed);
    }

    #[test]
    fn test_form_inflect_str_invalid_returns_normalized() {
        let form = Form::new("test".to_string(), Grams::new(Vec::<&str>::new()), None);
        let out = form.inflect_str(&ANALYZER, &["bad-gram"]);
        assert_eq!(out, "test");
    }

    #[test]
    fn test_form_inflect_str_empty_returns_normalized_when_no_raw() {
        let form = Form::new("base".to_string(), Grams::new(Vec::<&str>::new()), None);
        let out = form.inflect_str(&ANALYZER, &[]);
        assert_eq!(out, "base");
    }

    #[test]
    fn test_form_inflect_respects_gender_for_ambiguous_adjective() {
        let form = parse_form("московским", "московский");

        let femn_nomn = form.inflect_str(&ANALYZER, &["femn", "nomn"]);
        let masc_nomn = form.inflect_str(&ANALYZER, &["masc", "nomn"]);

        assert_eq!(femn_nomn, "московская");
        assert_eq!(masc_nomn, "московский");
        assert_ne!(femn_nomn, masc_nomn);
    }

    #[test]
    fn test_form_inflect_respects_case_and_number_for_month_name() {
        let form = parse_form("июня", "июнь");

        let nomn_sing = form.inflect_str(&ANALYZER, &["nomn", "sing"]);
        let gent_sing = form.inflect_str(&ANALYZER, &["gent", "sing"]);

        assert_eq!(nomn_sing, "июнь");
        assert_eq!(gent_sing, "июня");
        assert_ne!(nomn_sing, gent_sing);
    }

    #[test]
    fn test_form_inflect_respects_gender_for_participle_like_adjective() {
        let form = parse_form("закрытым", "закрытый");

        let masc_ablt = form.inflect_str(&ANALYZER, &["masc", "ablt"]);
        let femn_ablt = form.inflect_str(&ANALYZER, &["femn", "ablt"]);

        assert_eq!(masc_ablt, "закрытым");
        assert_eq!(femn_ablt, "закрытой");
        assert_ne!(masc_ablt, femn_ablt);
    }

    #[test]
    fn test_form_inflect_respects_number_for_ambiguous_noun() {
        let form = parse_form("заводе", "завод");

        let datv_sing = form.inflect_str(&ANALYZER, &["datv", "sing"]);
        let datv_plur = form.inflect_str(&ANALYZER, &["datv", "plur"]);

        assert_eq!(datv_sing, "заводу");
        assert_eq!(datv_plur, "заводам");
        assert_ne!(datv_sing, datv_plur);
    }
}
