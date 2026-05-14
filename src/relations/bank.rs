//! Конкретные морфологические отношения.
//!
//! Реализует согласование по роду, числу, падежу и их комбинацию «род + число + падеж» (ГЧП)
//! через типы отношений и фабрики [`gender_relation`], [`number_relation`], [`case_relation`],
//! [`gnc_relation`].

use std::sync::Arc;

use crate::morph::models::Form;

use super::graph::Relation;

/// Согласование по роду.
///
/// Возвращает `true`, если формы совместимы по роду.
/// Особый случай: оба слова во множественном числе — согласование всегда выполнено.
#[derive(Debug, Clone, Copy)]
pub struct GenderRelation;

impl Relation for GenderRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        if first.grams.number().plural && second.grams.number().plural {
            return true;
        }
        let f = first.grams.gender();
        let o = second.grams.gender();
        (f.male && o.male)
            || (f.female && o.female)
            || (f.neutral && o.neutral)
            || (f.bi && (o.male || o.female))
            || (o.bi && (f.male || f.female))
            || f.general
            || o.general
    }

    fn label(&self) -> String {
        "gender".into()
    }
}

/// Согласование по числу.
///
/// Учитывает как обычные формы (`sing` / `plur`),
/// так и лексемы с ограниченным числом (`Sgtm`, `Pltm`).
#[derive(Debug, Clone, Copy)]
pub struct NumberRelation;

impl Relation for NumberRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        let f = first.grams.number();
        let o = second.grams.number();
        (f.single && o.single)
            || (f.plural && o.plural)
            || (f.only_single && o.single)
            || (f.only_plural && o.plural)
            || (o.only_single && f.single)
            || (o.only_plural && f.plural)
    }

    fn label(&self) -> String {
        "number".into()
    }
}

/// Согласование по падежу.
///
/// Формы совместимы, если у них совпадает маска падежей,
/// либо у одной из форм установлен признак `Fixd` (фиксированный падеж).
#[derive(Debug, Clone, Copy)]
pub struct CaseRelation;

impl Relation for CaseRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        let f = first.grams.case();
        let o = second.grams.case();
        f.mask == o.mask || f.fixed || o.fixed
    }

    fn label(&self) -> String {
        "case".into()
    }
}

/// Согласование по роду, числу и падежу одновременно (ГЧП).
///
/// Аналог `gnc_relation` из Python: конъюнкция [`GenderRelation`],
/// [`NumberRelation`] и [`CaseRelation`].
#[derive(Debug, Clone, Copy)]
pub struct GncRelation;

impl Relation for GncRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        GenderRelation.check(first, second)
            && NumberRelation.check(first, second)
            && CaseRelation.check(first, second)
    }

    fn label(&self) -> String {
        "gnc".into()
    }
}

/// Создаёт отношение согласования по роду.
pub fn gender_relation() -> Arc<dyn Relation> {
    Arc::new(GenderRelation)
}

/// Создаёт отношение согласования по числу.
pub fn number_relation() -> Arc<dyn Relation> {
    Arc::new(NumberRelation)
}

/// Создаёт отношение согласования по падежу.
pub fn case_relation() -> Arc<dyn Relation> {
    Arc::new(CaseRelation)
}

/// Создаёт отношение согласования по роду, числу и падежу.
pub fn gnc_relation() -> Arc<dyn Relation> {
    Arc::new(GncRelation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Grams;

    fn form(grams: &[&str]) -> Form {
        Form::new("_".into(), Grams::new(grams.iter().copied()), None)
    }

    // ── GenderRelation ──────────────────────────────────────────────────────

    #[test]
    fn gender_same_male() {
        let f1 = form(&["masc", "sing"]);
        let f2 = form(&["masc", "sing"]);
        assert!(GenderRelation.check(&f1, &f2));
    }

    #[test]
    fn gender_male_vs_female() {
        let f1 = form(&["masc", "sing"]);
        let f2 = form(&["femn", "sing"]);
        assert!(!GenderRelation.check(&f1, &f2));
    }

    #[test]
    fn gender_both_plural_skips_gender_check() {
        // Оба во множественном числе — согласование по роду всегда выполнено
        let f1 = form(&["masc", "plur"]);
        let f2 = form(&["femn", "plur"]);
        assert!(GenderRelation.check(&f1, &f2));
    }

    #[test]
    fn gender_bi_matches_male() {
        let f1 = form(&["Ms-f", "sing"]);
        let f2 = form(&["masc", "sing"]);
        assert!(GenderRelation.check(&f1, &f2));
    }

    #[test]
    fn gender_general_always_matches() {
        let f1 = form(&["GNdr", "sing"]);
        let f2 = form(&["masc", "sing"]);
        assert!(GenderRelation.check(&f1, &f2));
    }

    // ── NumberRelation ──────────────────────────────────────────────────────

    #[test]
    fn number_both_single() {
        let f1 = form(&["sing"]);
        let f2 = form(&["sing"]);
        assert!(NumberRelation.check(&f1, &f2));
    }

    #[test]
    fn number_single_vs_plural() {
        let f1 = form(&["sing"]);
        let f2 = form(&["plur"]);
        assert!(!NumberRelation.check(&f1, &f2));
    }

    #[test]
    fn number_only_single_matches_single() {
        // Sgtm + sing — совместимы
        let f1 = form(&["Sgtm"]);
        let f2 = form(&["sing"]);
        assert!(NumberRelation.check(&f1, &f2));
    }

    #[test]
    fn number_only_plural_matches_plural() {
        // Pltm + plur — совместимы
        let f1 = form(&["Pltm"]);
        let f2 = form(&["plur"]);
        assert!(NumberRelation.check(&f1, &f2));
    }

    #[test]
    fn number_only_plural_vs_single() {
        let f1 = form(&["Pltm"]);
        let f2 = form(&["sing"]);
        assert!(!NumberRelation.check(&f1, &f2));
    }

    // ── CaseRelation ────────────────────────────────────────────────────────

    #[test]
    fn case_same_nomn() {
        let f1 = form(&["nomn"]);
        let f2 = form(&["nomn"]);
        assert!(CaseRelation.check(&f1, &f2));
    }

    #[test]
    fn case_nomn_vs_gent() {
        let f1 = form(&["nomn"]);
        let f2 = form(&["gent"]);
        assert!(!CaseRelation.check(&f1, &f2));
    }

    #[test]
    fn case_fixed_always_matches() {
        let f1 = form(&["nomn", "Fixd"]);
        let f2 = form(&["gent"]);
        assert!(CaseRelation.check(&f1, &f2));
    }

    #[test]
    fn case_other_fixed_always_matches() {
        let f1 = form(&["nomn"]);
        let f2 = form(&["gent", "Fixd"]);
        assert!(CaseRelation.check(&f1, &f2));
    }

    // ── GncRelation ─────────────────────────────────────────────────────────

    #[test]
    fn gnc_full_agreement() {
        let f1 = form(&["masc", "sing", "nomn"]);
        let f2 = form(&["masc", "sing", "nomn"]);
        assert!(GncRelation.check(&f1, &f2));
    }

    #[test]
    fn gnc_fails_on_case_mismatch() {
        let f1 = form(&["masc", "sing", "nomn"]);
        let f2 = form(&["masc", "sing", "gent"]);
        assert!(!GncRelation.check(&f1, &f2));
    }

    #[test]
    fn gnc_fails_on_gender_mismatch() {
        let f1 = form(&["masc", "sing", "nomn"]);
        let f2 = form(&["femn", "sing", "nomn"]);
        assert!(!GncRelation.check(&f1, &f2));
    }

    #[test]
    fn gnc_fails_on_number_mismatch() {
        let f1 = form(&["masc", "sing", "nomn"]);
        let f2 = form(&["masc", "plur", "nomn"]);
        assert!(!GncRelation.check(&f1, &f2));
    }

    // ── Фабричные функции ───────────────────────────────────────────────────

    #[test]
    fn factory_functions_return_arc() {
        let f1 = form(&["masc", "sing", "nomn"]);
        let f2 = form(&["masc", "sing", "nomn"]);
        assert!(gender_relation().check(&f1, &f2));
        assert!(number_relation().check(&f1, &f2));
        assert!(case_relation().check(&f1, &f2));
        assert!(gnc_relation().check(&f1, &f2));
    }

    #[test]
    fn labels() {
        assert_eq!(GenderRelation.label(), "gender");
        assert_eq!(NumberRelation.label(), "number");
        assert_eq!(CaseRelation.label(), "case");
        assert_eq!(GncRelation.label(), "gnc");
    }
}
