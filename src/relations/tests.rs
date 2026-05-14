use super::*;
use crate::fact;
use crate::interpretation::{FactValue, RuleInterpretation};
use crate::parser::Parser;
use crate::predicates::constructors::gram;
use crate::rule::builder::{main_term, pred, rule};
use crate::rule::registry::RuleRegistry;

#[test]
fn gnc_relation_filters_incompatible_forms() {
    fact!(Name => [first, last]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let gnc = gnc_relation();

    let first_rule = pred(gram("Name"))
        .interpretation(Name::first.inflected(FORMS))
        .match_relation(gnc.clone());

    let last_rule = pred(gram("Surn"))
        .interpretation(Name::last.inflected(FORMS))
        .match_relation(gnc);

    let name_rule = rule([first_rule, last_rule]).interpretation_fact::<Name>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(name_rule.build(()));
    registry.validate().expect("registry must be valid");

    let parser = Parser::new(&registry, rid);

    // "саше иванову" — дательный, оба согласованы → match
    let m = parser.find("саше иванову");
    assert!(m.is_some(), "expected match for 'саше иванову'");

    let record = m.unwrap().fact(&registry).expect("expected Name fact");
    assert_eq!(record.name(), "Name");

    let first_val = match record.get("first") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for first, got {:?}", other),
    };

    let last_val = match record.get("last") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for last, got {:?}", other),
    };

    assert_eq!(first_val, "саша");
    assert_eq!(last_val, "иванов");
}

#[test]
fn gnc_relation_rejects_inconsistent_case() {
    let gnc = gnc_relation();

    let first_rule = pred(gram("Name")).match_relation(gnc.clone());
    let last_rule = pred(gram("Surn")).match_relation(gnc);

    let name_rule = rule([first_rule, last_rule]);

    let mut registry = RuleRegistry::new();
    let rid = registry.add(name_rule.build(()));

    let parser = Parser::new(&registry, rid);

    // "сашу ивановой" — accusative + genitive/dative → no GNC agreement
    let m = parser.r#match("сашу ивановой");
    assert!(m.is_none(), "expected no match for 'сашу ивановой'");
}

#[test]
fn main_marker_sets_relation_head() {
    let relation = and_relation(vec![number_relation(), gender_relation()]);

    let a =
        rule([pred(gram("Surn")), main_term(pred(gram("Name")))]).match_relation(relation.clone());
    let b = pred(gram("VERB")).match_relation(relation);

    let ab = rule([a, b]);

    let mut registry = RuleRegistry::new();
    let rid = registry.add(ab.build(()));

    let parser = Parser::new(&registry, rid);

    // "иванов иван стал" — sing masc, verb sing → match
    let m = parser.r#match("иванов иван стал");
    assert!(m.is_some(), "expected match for 'иванов иван стал'");

    // "иванов иван стали" — sing masc name, but verb is plur → no match
    let m = parser.r#match("иванов иван стали");
    assert!(
        m.is_none(),
        "expected no match for 'иванов иван стали' (number disagreement)"
    );
}
