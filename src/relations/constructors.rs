//! Композиционные конструкторы отношений.
//!
//! Модуль реализует логические комбинаторы (`And`, `Or`, `Not`) для
//! [`Relation`], позволяя строить составные отношения из примитивных.

use std::fmt;
use std::sync::Arc;

use crate::morph::models::Form;

use super::graph::Relation;

/// Конъюнкция отношений: все внутренние отношения должны быть выполнены.
pub struct AndRelation {
    /// Внутренние отношения, объединённые логическим «И».
    pub relations: Vec<Arc<dyn Relation>>,
}

impl Relation for AndRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        self.relations.iter().all(|r| r.check(first, second))
    }

    fn label(&self) -> String {
        let inner: Vec<String> = self.relations.iter().map(|r| r.label()).collect();
        format!("and_({})", inner.join(", "))
    }
}

impl fmt::Debug for AndRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AndRelation")
            .field("relations", &format!("[{} items]", self.relations.len()))
            .finish()
    }
}

/// Дизъюнкция отношений: хотя бы одно внутреннее отношение должно быть выполнено.
pub struct OrRelation {
    /// Внутренние отношения, объединённые логическим «ИЛИ».
    pub relations: Vec<Arc<dyn Relation>>,
}

impl Relation for OrRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        self.relations.iter().any(|r| r.check(first, second))
    }

    fn label(&self) -> String {
        let inner: Vec<String> = self.relations.iter().map(|r| r.label()).collect();
        format!("or_({})", inner.join(", "))
    }
}

impl fmt::Debug for OrRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrRelation")
            .field("relations", &format!("[{} items]", self.relations.len()))
            .finish()
    }
}

/// Отрицание отношения.
pub struct NotRelation {
    /// Отношение, результат которого инвертируется.
    pub relation: Arc<dyn Relation>,
}

impl Relation for NotRelation {
    fn check(&self, first: &Form, second: &Form) -> bool {
        !self.relation.check(first, second)
    }

    fn label(&self) -> String {
        format!("not_({})", self.relation.label())
    }
}

impl fmt::Debug for NotRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotRelation")
            .field("relation", &self.relation.label())
            .finish()
    }
}

/// Создает конъюнкцию отношений.
pub fn and_relation(relations: Vec<Arc<dyn Relation>>) -> Arc<dyn Relation> {
    Arc::new(AndRelation { relations })
}

/// Создает дизъюнкцию отношений.
pub fn or_relation(relations: Vec<Arc<dyn Relation>>) -> Arc<dyn Relation> {
    Arc::new(OrRelation { relations })
}

/// Создает отрицание отношения.
pub fn not_relation(relation: Arc<dyn Relation>) -> Arc<dyn Relation> {
    Arc::new(NotRelation { relation })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::models::Grams;

    struct AlwaysTrue;
    impl Relation for AlwaysTrue {
        fn check(&self, _first: &Form, _second: &Form) -> bool {
            true
        }
        fn label(&self) -> String {
            "always_true".into()
        }
    }

    struct AlwaysFalse;
    impl Relation for AlwaysFalse {
        fn check(&self, _first: &Form, _second: &Form) -> bool {
            false
        }
        fn label(&self) -> String {
            "always_false".into()
        }
    }

    struct SameCaseRelation;
    impl Relation for SameCaseRelation {
        fn check(&self, first: &Form, second: &Form) -> bool {
            first.grams.case() == second.grams.case()
        }
        fn label(&self) -> String {
            "same_case".into()
        }
    }

    fn make_form(normalized: &str, grams: &[&str]) -> Form {
        Form::new(
            normalized.to_string(),
            Grams::new(grams.iter().copied()),
            None,
        )
    }

    #[test]
    fn and_relation_all_true() {
        let r = and_relation(vec![Arc::new(AlwaysTrue), Arc::new(AlwaysTrue)]);
        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);
        assert!(r.check(&f1, &f2));
    }

    #[test]
    fn and_relation_one_false() {
        let r = and_relation(vec![Arc::new(AlwaysTrue), Arc::new(AlwaysFalse)]);
        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);
        assert!(!r.check(&f1, &f2));
    }

    #[test]
    fn or_relation_one_true() {
        let r = or_relation(vec![Arc::new(AlwaysFalse), Arc::new(AlwaysTrue)]);
        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);
        assert!(r.check(&f1, &f2));
    }

    #[test]
    fn or_relation_all_false() {
        let r = or_relation(vec![Arc::new(AlwaysFalse), Arc::new(AlwaysFalse)]);
        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);
        assert!(!r.check(&f1, &f2));
    }

    #[test]
    fn not_relation_inverts() {
        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);

        let r_true = not_relation(Arc::new(AlwaysTrue));
        assert!(!r_true.check(&f1, &f2));

        let r_false = not_relation(Arc::new(AlwaysFalse));
        assert!(r_false.check(&f1, &f2));
    }

    #[test]
    fn labels_match_expected_format() {
        let and = and_relation(vec![Arc::new(AlwaysTrue), Arc::new(AlwaysFalse)]);
        assert_eq!(and.label(), "and_(always_true, always_false)");

        let or = or_relation(vec![Arc::new(AlwaysTrue), Arc::new(AlwaysFalse)]);
        assert_eq!(or.label(), "or_(always_true, always_false)");

        let not = not_relation(Arc::new(AlwaysTrue));
        assert_eq!(not.label(), "not_(always_true)");
    }

    #[test]
    fn nested_composition() {
        let inner_or = or_relation(vec![Arc::new(AlwaysFalse), Arc::new(AlwaysTrue)]);
        let inner_not = not_relation(Arc::new(AlwaysFalse));
        let composed = and_relation(vec![inner_or, inner_not]);

        let f1 = make_form("a", &["nomn"]);
        let f2 = make_form("b", &["nomn"]);
        assert!(composed.check(&f1, &f2));
        assert_eq!(
            composed.label(),
            "and_(or_(always_false, always_true), not_(always_false))"
        );
    }

    #[test]
    fn composition_with_morph_relation() {
        let nomn = make_form("дом", &["NOUN", "nomn", "sing"]);
        let gent = make_form("дома", &["NOUN", "gent", "sing"]);

        let same_case: Arc<dyn Relation> = Arc::new(SameCaseRelation);
        assert!(same_case.check(&nomn, &nomn));
        assert!(!same_case.check(&nomn, &gent));

        let negated = not_relation(Arc::new(SameCaseRelation));
        assert!(!negated.check(&nomn, &nomn));
        assert!(negated.check(&nomn, &gent));
    }
}
