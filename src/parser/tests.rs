use super::*;
use crate::fact;
use crate::interpretation::{FactValue, RuleInterpretation};
use crate::pipeline::morph_pipeline;
use crate::predicates::constructors::{and, dictionary, eq, gram, is_title, not};
use crate::rule::builder::{or_, pred, rule, term, RuleBuilder};
use crate::rule::registry::RuleRegistry;

fn person_or_title_rule<'a>() -> RuleBuilder<'a> {
    let position = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]));
    let name = (pred(gram("Name")) + pred(gram("Surn"))).named("NAME");
    let person = (position + name).named("PERSON");
    let title = rule([term("«"), pred(not(eq("»"))).repeatable(), term("»")]).named("TITLE");
    or_([person, title])
}

#[test]
fn api_example_style_geo_findall_matches_phrase() {
    let text = "Russian Federation";

    let mut registry = RuleRegistry::new();
    let geo_id = registry.add((term("Russian") + term("Federation")).build(()));

    let parser = Parser::new(&registry, geo_id);
    let matches = parser.findall(text);

    assert!(!matches.is_empty());
    assert!(matches.iter().any(|m| {
        let values: Vec<String> = m
            .tokens()
            .into_iter()
            .map(|t| t.value.into_owned())
            .collect();
        values == vec!["Russian".to_string(), "Federation".to_string()]
    }));
}

#[test]
fn python_geo_example_findall_extracts_two_matches() {
    let text = r#"
        В Чеченской республике на день рождения ...
        Донецкая народная республика провозгласила ...
        Башня Федерация — одна из самых высоких ...
        "#;

    let geo = (pred(and(vec![
        gram("ADJF"),
        is_title(), // аналог is_capitalized()
    ])) + pred(gram("ADJF")).optional().repeatable()
        + pred(dictionary(&["федерация", "республика"])))
    .build(());

    let mut registry = RuleRegistry::new();
    let geo_id = registry.add(geo);

    let parser = Parser::new(&registry, geo_id);
    let result = parser.findall_text(text);
    let expected = vec![
        vec!["Чеченской".to_string(), "республике".to_string()],
        vec![
            "Донецкая".to_string(),
            "народная".to_string(),
            "республика".to_string(),
        ],
    ];
    assert_eq!(result, expected);
}

#[test]
fn python_person_or_title_findall_token_values() {
    let text = "Президент Владимир Путин в фильме «Интервью с Путиным» ..";

    let mut registry = RuleRegistry::new();
    let rid = registry.add(person_or_title_rule().build(()));
    let parser = Parser::new(&registry, rid);

    let result = parser.findall_text(text);
    let expected = vec![
        vec![
            "Президент".to_string(),
            "Владимир".to_string(),
            "Путин".to_string(),
        ],
        vec![
            "«".to_string(),
            "Интервью".to_string(),
            "с".to_string(),
            "Путиным".to_string(),
            "»".to_string(),
        ],
    ];

    assert_eq!(result, expected);
}

#[test]
fn python_person_match_full_input_vs_trailing_tokens() {
    let mut registry = RuleRegistry::new();
    let rid = registry.add(person_or_title_rule().build(()));
    let parser = Parser::new(&registry, rid);

    let good = parser
        .r#match("Президент Владимир Путин")
        .expect("expected full match for person");
    let good_values: Vec<String> = good
        .tokens()
        .into_iter()
        .map(|t| t.value.into_owned())
        .collect();
    assert_eq!(
        good_values,
        vec![
            "Президент".to_string(),
            "Владимир".to_string(),
            "Путин".to_string(),
        ]
    );

    let bad = parser.r#match("Президент Владимир Путин 25 мая");
    assert!(bad.is_none());
}

#[test]
fn python_person_interpretation_fact_position_inflected() {
    fact!(Person => [position]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let root = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]))
        .interpretation(Person::position.inflected(FORMS))
        .interpretation_fact::<Person>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(root.build(()));
    registry.validate().expect("registry must be valid");

    let parser = Parser::new(&registry, rid);
    let m = parser
        .find("президент")
        .expect("expected person match by position pipeline");
    let record = m.fact(&registry).expect("expected extracted person fact");

    assert_eq!(record.name(), "Person");
    assert!(matches!(record.get("position"), Some(FactValue::Str(_))));
}

#[test]
fn extracted_fact_to_json_value_from_match() {
    fact!(TinyFact => [position]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let root = RuleBuilder::from_arc(morph_pipeline(["президент"]))
        .interpretation(TinyFact::position.inflected(FORMS))
        .interpretation_fact::<TinyFact>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(root.build(()));
    registry.validate().expect("registry must be valid");

    let parser = Parser::new(&registry, rid);
    let m = parser.find("президент").expect("match");
    let extracted = m.fact(&registry).expect("extracted fact");

    let v = extracted.to_json_value();
    assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("TinyFact"));
    assert!(v.get("fields").expect("fields").is_object());
    assert!(v.get("spans").expect("spans").is_array());

    let wire = serde_json::to_string(&v).expect("wire json");
    let parsed: serde_json::Value = serde_json::from_str(&wire).expect("parse back");
    assert_eq!(parsed["name"], v["name"]);
    assert_eq!(parsed["fields"], v["fields"]);
}

#[test]
fn inflected_surname_genitive_to_nominative() {
    fact!(Name => [first, last]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let name_rule = rule([
        pred(gram("Name")).interpretation(Name::first.inflected(FORMS)),
        pred(gram("Surn")).interpretation(Name::last.inflected(FORMS)),
    ])
    .interpretation_fact::<Name>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(name_rule.build(()));
    registry.validate().expect("registry must be valid");

    let parser = Parser::new(&registry, rid);

    // "Владимира Путина" — genitive; expect nominative after inflection.
    let m = parser
        .find("Владимира Путина")
        .expect("expected name match in genitive");
    let record = m.fact(&registry).expect("expected Name fact");

    assert_eq!(record.name(), "Name");

    let first = match record.get("first") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for first, got {:?}", other),
    };
    let last = match record.get("last") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for last, got {:?}", other),
    };

    assert_eq!(first, "владимир", "first name should be nominative");
    assert_eq!(last, "путин", "last name should be nominative");
}

#[test]
fn inflected_surname_instrumental_to_nominative() {
    fact!(Name => [first, last]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let name_rule = rule([
        pred(gram("Name")).interpretation(Name::first.inflected(FORMS)),
        pred(gram("Surn")).interpretation(Name::last.inflected(FORMS)),
    ])
    .interpretation_fact::<Name>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(name_rule.build(()));
    registry.validate().expect("registry must be valid");

    let parser = Parser::new(&registry, rid);

    // "Владимиром Путиным" — instrumental case
    let m = parser
        .find("Владимиром Путиным")
        .expect("expected name match in instrumental");
    let record = m.fact(&registry).expect("expected Name fact");

    let first = match record.get("first") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for first, got {:?}", other),
    };
    let last = match record.get("last") {
        Some(FactValue::Str(s)) => s.clone(),
        other => panic!("expected Str for last, got {:?}", other),
    };

    assert_eq!(first, "владимир", "first name should be nominative");
    assert_eq!(last, "путин", "last name should be nominative");
}
