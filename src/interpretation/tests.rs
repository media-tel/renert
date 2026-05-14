use std::collections::{BTreeMap, HashMap};

use super::*;
use crate::fact;
use crate::interpretation::normalizer::InterpretationValue;
use crate::interpretation::normalizer::{InflectedNormalizer, NormalizedItem};
use crate::interpretation::FactAttributeInput;
use crate::interpretation::FactValue;
use crate::parser::Parser;
use crate::predicates::constructors::{and, dictionary, eq, gte, is_digit, lte};
use crate::rule::builder::{or_, pred, term};
use crate::rule::registry::RuleRegistry;
use crate::token::Tokenizer;

// =================
// Yargy API analogs
// =================

#[test]
fn fact_record_to_json_preserves_order() {
    let scheme = crate::interpretation::fact(
        "OrderTest",
        vec![
            FactAttributeInput::from("z_field"),
            FactAttributeInput::from("a_field"),
        ],
    );
    let mut kwargs = BTreeMap::new();
    kwargs.insert("z_field".into(), FactValue::Int(1));
    kwargs.insert("a_field".into(), FactValue::Int(2));
    let record = FactRecord::try_new(scheme, kwargs).expect("record");
    let json_str = serde_json::to_string(&record.to_json_value()).expect("json");
    let z_pos = json_str.find("\"z_field\"").expect("z_field key");
    let a_pos = json_str.find("\"a_field\"").expect("a_field key");
    assert!(
        z_pos < a_pos,
        "JSON key order should match as_json / schema order (z before a), got {json_str}"
    );
}

#[test]
fn test_date_interpretation_like_yargy() {
    fact!(Date => [year, month, day]);

    static MONTHS: &[&str] = &[
        "январь",
        "февраль",
        "март",
        "апрель",
        "мая",
        "июнь",
        "июль",
        "август",
        "сентябрь",
        "октябрь",
        "ноябрь",
        "декабрь",
    ];

    let month_name = pred(dictionary(MONTHS));
    let month = pred(and(vec![gte(1), lte(12)]));
    let day = pred(and(vec![gte(1), lte(31)]));
    let year = pred(and(vec![gte(1900), lte(2100)]));

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();

    let rule = (day.clone().interpretation(Date::day)
        + month_name.clone().interpretation(Date::month)
        + year.clone().interpretation(Date::year))
        | (year.clone().interpretation(Date::year)
            + term("-")
            + month.clone().interpretation(Date::month)
            + term("-")
            + day.clone().interpretation(Date::day))
        | (year.clone().interpretation(Date::year) + term("г") + term("."));

    let rule = rule.interpretation_fact::<Date>().build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);

    let lines = ["2015г.", "18 июля 2016", "2016-01-02"];
    let facts: Vec<_> = lines
        .iter()
        .map(|line| {
            parser
                .r#match(line)
                .and_then(|m| m.fact(&registry))
                .expect("expected date fact")
        })
        .collect();

    // Date(year='2015', month=None, day=None)
    assert_eq!(
        facts[0].get("year"),
        Some(&FactValue::Str("2015".to_string()))
    );
    assert_eq!(facts[0].get("month"), None);
    assert_eq!(facts[0].get("day"), None);

    // Date(year='2016', month='июля', day='18')
    assert_eq!(
        facts[1].get("year"),
        Some(&FactValue::Str("2016".to_string()))
    );
    assert_eq!(
        facts[1].get("month"),
        Some(&FactValue::Str("июля".to_string()))
    );
    assert_eq!(facts[1].get("day"), Some(&FactValue::Str("18".to_string())));

    // Date(year='2016', month='01', day='02')
    assert_eq!(
        facts[2].get("year"),
        Some(&FactValue::Str("2016".to_string()))
    );
    assert_eq!(
        facts[2].get("month"),
        Some(&FactValue::Str("01".to_string()))
    );
    assert_eq!(facts[2].get("day"), Some(&FactValue::Str("02".to_string())));

    assert_eq!(
        facts[0].to_string(),
        "Date(\n    year='2015',\n    month=None,\n    day=None\n)"
    );
    assert_eq!(
        facts[1].to_string(),
        "Date(\n    year='2016',\n    month='июля',\n    day='18'\n)"
    );
    assert_eq!(
        facts[2].to_string(),
        "Date(\n    year='2016',\n    month='01',\n    day='02'\n)"
    );
}

#[test]
fn date_interpretation_pipeline() {
    fact!(Date => [year, month, day]);

    let text = "\
        01 января 2020
        2021-02-03
        1999 г.
    ";

    let mut registry = RuleRegistry::new();

    // Упрощённые DAY / MONTH_NAME / MONTH / YEAR
    let day = pred(is_digit()); // DAY
    let month_name = pred(dictionary(&["января", "февраля", "марта"])); // MONTH_NAME
    let month = pred(is_digit()); // MONTH
    let year = pred(is_digit()); // YEAR

    let date_id = registry.next_id();

    // Аналог yargy-примера:
    // or_(
    //   rule(DAY.interpretation(Date.day), MONTH_NAME.interpretation(Date.month), YEAR.interpretation(Date.year)),
    //   rule(YEAR.interpretation(Date.year), '-', MONTH.interpretation(Date.month), '-', DAY.interpretation(Date.day)),
    //   rule(YEAR.interpretation(Date.year), 'г', '.')
    // ).interpretation(Date)

    let date_rule = (
        // DAY MONTH_NAME YEAR
        day.clone().interpretation(Date::day)
            + month_name.interpretation(Date::month)
            + year.clone().interpretation(Date::year)
    ) | (
        // YEAR - MONTH - DAY
        year.clone().interpretation(Date::year)
            + term("-")
            + month.clone().interpretation(Date::month)
            + term("-")
            + day.clone().interpretation(Date::day)
    ) | (
        // YEAR "г."
        year.interpretation(Date::year) + term("г") + term(".")
    );
    let date_rule = date_rule.interpretation_fact::<Date>().build(date_id);

    registry.add(date_rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, date_id);

    let matches: Vec<_> = parser.findall(text).into_iter().collect();
    assert_eq!(matches.len(), 3, "expected three date matches");

    let facts: Vec<_> = matches.iter().filter_map(|m| m.fact(&registry)).collect();

    assert_eq!(facts.len(), 3, "expected three extracted facts");

    // 01 января 2020
    assert_eq!(facts[0].name(), "Date");
    assert_eq!(facts[0].get("day"), Some(&FactValue::Str("01".to_string())));
    assert_eq!(
        facts[0].get("month"),
        Some(&FactValue::Str("января".to_string()))
    );
    assert_eq!(
        facts[0].get("year"),
        Some(&FactValue::Str("2020".to_string()))
    );

    // 2021-02-03
    assert_eq!(facts[1].name(), "Date");
    assert_eq!(
        facts[1].get("year"),
        Some(&FactValue::Str("2021".to_string()))
    );
    assert_eq!(
        facts[1].get("month"),
        Some(&FactValue::Str("02".to_string()))
    );
    assert_eq!(facts[1].get("day"), Some(&FactValue::Str("03".to_string())));

    // 1999 г.
    assert_eq!(facts[2].name(), "Date");
    assert_eq!(
        facts[2].get("year"),
        Some(&FactValue::Str("1999".to_string()))
    );
    assert_eq!(facts[2].get("month"), None);
    assert_eq!(facts[2].get("day"), None);
}

#[test]
fn test_normalizer_api() {
    fact!(Date => [year, month, day]);

    static MONTHS: &[&str] = &[
        "январь",
        "февраль",
        "март",
        "апрель",
        "мая",
        "июнь",
        "июль",
        "август",
        "сентябрь",
        "октябрь",
        "ноябрь",
        "декабрь",
    ];

    fn parse_int(value: FactValue) -> i64 {
        value.as_str().unwrap().parse::<i64>().unwrap()
    }

    let months: HashMap<&'static str, i64> = HashMap::from([
        ("январь", 1),
        ("февраль", 2),
        ("март", 3),
        ("апрель", 4),
        ("мая", 5),
        ("июнь", 6),
        ("июль", 7),
        ("август", 8),
        ("сентябрь", 9),
        ("октябрь", 10),
        ("ноябрь", 11),
        ("декабрь", 12),
    ]);

    let month_name = pred(dictionary(MONTHS));
    let month = pred(and(vec![gte(1), lte(12)]));
    let day = pred(and(vec![gte(1), lte(31)]));
    let year = pred(and(vec![gte(1900), lte(2100)]));

    let mut registry = RuleRegistry::new();
    let date_id = registry.next_id();

    let date_rule = (day.clone().interpretation(Date::day.custom(parse_int))
        + month_name.interpretation(
            Date::month
                .normalized()
                .custom(move |value: String| months.get(value.as_str()).copied().unwrap()),
        )
        + year.clone().interpretation(Date::year.custom(parse_int)))
        | (year.clone().interpretation(Date::year.custom(parse_int))
            + term("-")
            + month.interpretation(Date::month.custom(parse_int))
            + term("-")
            + day.interpretation(Date::day.custom(parse_int)))
        | (year.interpretation(Date::year.custom(parse_int)) + term("г") + term("."));

    let date_rule = date_rule.interpretation_fact::<Date>().build(date_id);

    registry.add(date_rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, date_id);
    let m = parser.r#match("18 июня 2016").expect("expected match");
    let fact = m.fact(&registry).expect("expected date fact");

    assert_eq!(fact.get("day"), Some(&FactValue::Int(18)));
    assert_eq!(fact.get("month"), Some(&FactValue::Int(6)));
    assert_eq!(fact.get("year"), Some(&FactValue::Int(2016)));
}

#[test]
fn test_date_fact_display_example_api() {
    fact!(Date => [year, month, day]);

    static MONTHS: &[&str] = &[
        "январь",
        "февраль",
        "март",
        "апрель",
        "мая",
        "июнь",
        "июль",
        "август",
        "сентябрь",
        "октябрь",
        "ноябрь",
        "декабрь",
    ];

    let month_name = pred(dictionary(MONTHS));
    let month = pred(and(vec![gte(1), lte(12)]));
    let day = pred(and(vec![gte(1), lte(31)]));
    let year = pred(and(vec![gte(1900), lte(2100)]));

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();

    let date_rule = (day.clone().interpretation(Date::day)
        + month_name.clone().interpretation(Date::month)
        + year.clone().interpretation(Date::year))
        | (year.clone().interpretation(Date::year)
            + term("-")
            + month.clone().interpretation(Date::month)
            + term("-")
            + day.clone().interpretation(Date::day))
        | (year.interpretation(Date::year) + term("г") + term("."));

    let date_rule = date_rule.interpretation_fact::<Date>().build(rid);

    registry.add(date_rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let text = "2015г.\n18 июля 2016\n2016-01-02\n";

    let mut rendered = Vec::new();
    let mut years = Vec::new();
    let mut json_projection = Vec::new();

    for line in text.lines() {
        if let Some(m) = parser.r#match(line) {
            if let Some(fact) = m.fact(&registry) {
                rendered.push(fact.to_string());
                years.push(fact.get("year").cloned());
                json_projection.push(fact.as_json());
            }
        }
    }

    assert_eq!(
        rendered,
        vec![
            "Date(\n    year='2015',\n    month=None,\n    day=None\n)".to_string(),
            "Date(\n    year='2016',\n    month='июля',\n    day='18'\n)".to_string(),
            "Date(\n    year='2016',\n    month='01',\n    day='02'\n)".to_string(),
        ]
    );
    assert_eq!(
        years,
        vec![
            Some(FactValue::Str("2015".to_string())),
            Some(FactValue::Str("2016".to_string())),
            Some(FactValue::Str("2016".to_string())),
        ]
    );
    assert_eq!(
        json_projection,
        vec![
            vec![("year".to_string(), FactValue::Str("2015".to_string()))],
            vec![
                ("year".to_string(), FactValue::Str("2016".to_string())),
                ("month".to_string(), FactValue::Str("июля".to_string())),
                ("day".to_string(), FactValue::Str("18".to_string())),
            ],
            vec![
                ("year".to_string(), FactValue::Str("2016".to_string())),
                ("month".to_string(), FactValue::Str("01".to_string())),
                ("day".to_string(), FactValue::Str("02".to_string())),
            ],
        ]
    );
}

#[test]
fn test_predicate_attribute() {
    fact!(F => [a]);

    let mut registry = RuleRegistry::new();
    let rule_id = registry.next_id();
    let rule = pred(eq("a"))
        .interpretation(F::a)
        .interpretation_fact::<F>()
        .build(rule_id);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rule_id);
    let m = parser.r#match("a").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "F");
    assert_eq!(m.span(), crate::span::Span::new(0, 1));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 1)]);
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("a".to_string()))]
    );
    assert_eq!(record.get("a"), Some(&FactValue::Str("a".to_string())));
}

#[test]
fn test_fact_through_or_root() {
    fact!(A => [x]);
    fact!(B => [y]);

    let a = pred(eq("a"))
        .interpretation(A::x)
        .interpretation_fact::<A>();
    let b = pred(eq("b"))
        .interpretation(B::y)
        .interpretation_fact::<B>();

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = or_([a, b]).build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);

    let ma = parser.r#match("a").expect("expected match for 'a'");
    let fa = ma.fact(&registry).expect("expected A fact from or-root");
    assert_eq!(fa.name(), "A");
    assert_eq!(fa.get("x"), Some(&FactValue::Str("a".into())));

    let mb = parser.r#match("b").expect("expected match for 'b'");
    let fb = mb.fact(&registry).expect("expected B fact from or-root");
    assert_eq!(fb.name(), "B");
    assert_eq!(fb.get("y"), Some(&FactValue::Str("b".into())));
}

#[test]
fn test_merge_facts() {
    fact!(F => [a, b]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = (pred(eq("a")).interpretation(F::a) + pred(eq("b")).interpretation(F::b))
        .interpretation_fact::<F>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("a b").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");
    assert_eq!(record.get("a"), Some(&FactValue::Str("a".to_string())));
    assert_eq!(record.get("b"), Some(&FactValue::Str("b".to_string())));
    assert_eq!(
        record.spans,
        vec![crate::span::Span::new(0, 1), crate::span::Span::new(2, 3),]
    );
    assert_eq!(
        record.as_json(),
        vec![
            ("a".to_string(), FactValue::Str("a".to_string())),
            ("b".to_string(), FactValue::Str("b".to_string())),
        ]
    );
    assert_eq!(m.span(), crate::span::Span::new(0, 3));
}

#[test]
fn test_rule_attribute() {
    fact!(FApi => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = (term("a") + term("A"))
        .interpretation(FApi::a)
        .interpretation_fact::<FApi>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("a   A").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FApi");
    assert_eq!(record.get("a"), Some(&FactValue::Str("a A".to_string())));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 5)]);
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("a A".to_string()))]
    );
    assert_eq!(m.span(), crate::span::Span::new(0, 5));
}

#[test]
fn test_insted_attributes() {
    fact!(F => [a, b]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = pred(eq("a"))
        .interpretation(F::a)
        .interpretation(F::b)
        .interpretation_fact::<F>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("a").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");
    assert_eq!(record.get("a"), None);
    assert_eq!(record.get("b"), Some(&FactValue::Str("a".to_string())));
}

#[test]
fn test_nested_facts() {
    fact!(F => [a]);
    fact!(G => [b]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();

    let rule = pred(eq("a"))
        .interpretation(F::a)
        .interpretation_fact::<F>()
        .interpretation(G::b)
        .interpretation_fact::<G>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("a").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    let nested = FactValue::Object(vec![("a".to_string(), FactValue::Str("a".to_string()))]);
    assert_eq!(record.name(), "G");
    assert_eq!(record.get("b"), Some(&nested));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 1)]);
    assert_eq!(record.as_json(), vec![("b".to_string(), nested)]);
}

#[test]
fn nested_facts_via_runtime_interpretators_analog() {
    let f_scheme = crate::interpretation::fact("F", vec![FactAttributeInput::from("a")]);
    let g_scheme = crate::interpretation::fact("G", vec![FactAttributeInput::from("b")]);

    let f_attr = AttributeInterpretator::new(Attribute::new("F", "a", None));
    let f_fact = FactInterpretator::new(f_scheme);
    let g_attr = AttributeInterpretator::new(Attribute::new("G", "b", None));
    let g_fact = FactInterpretator::new(g_scheme);

    let f_a_value = f_attr
        .call(InterpretatorInput::from_tokens(
            Tokenizer::new().tokenize("a"),
            None,
        ))
        .unwrap();
    let f_value = f_fact
        .call(InterpretatorInput::new(
            vec![InterpretatorInputItem::Result(f_a_value)],
            None,
        ))
        .unwrap();
    let g_b_value = g_attr
        .call(InterpretatorInput::new(
            vec![InterpretatorInputItem::Result(f_value)],
            None,
        ))
        .unwrap();
    let g_value = g_fact
        .call(InterpretatorInput::new(
            vec![InterpretatorInputItem::Result(g_b_value)],
            None,
        ))
        .unwrap();

    let InterpretationValue::FactResult(result) = g_value else {
        panic!("expected fact result");
    };
    let fact = result.into_runtime_fact().expect("runtime fact expected");
    let normalized = fact.normalized();
    assert_eq!(
        normalized.get("b"),
        Some(&FactValue::Object(vec![(
            "a".to_string(),
            FactValue::Str("a".to_string())
        )]))
    );
}

#[test]
fn test_rule_custom_attribute() {
    fact!(F => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("1")
        .interpretation(F::a.custom(|s: FactValue| s.as_str().unwrap().parse::<i64>().unwrap()))
        .interpretation_fact::<F>()
        .build(rid);
    registry.add(rule);

    registry.validate().unwrap();
    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("1").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "F");
    assert_eq!(record.get("a"), Some(&FactValue::Int(1)));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 1)]);
    assert_eq!(record.as_json(), vec![("a".to_string(), FactValue::Int(1))]);
}

#[test]
fn test_rule_custom() {
    fact!(F => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();

    let rule = (term("3") + term(".") + term("14"))
        .interpretation(F::a.custom(|s: FactValue| FactValue::Str(s.as_str().unwrap().to_string())))
        .interpretation_fact::<F>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("3.14").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "F");
    assert_eq!(record.get("a"), Some(&FactValue::Str("3.14".to_string())));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 4)]);
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("3.14".to_string()))]
    );
}

#[test]
fn test_inflected() {
    fact!(Adj => [v]);
    static FORMS: &[&str] = &["nomn", "femn"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("московским")
        .interpretation(Adj::v.inflected(FORMS))
        .interpretation_fact::<Adj>()
        .build(rid);

    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("московским").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(
        record.get("v"),
        Some(&FactValue::Str("московская".to_string()))
    );
}

#[test]
fn test_inflected_with_nomn_masc_for_adjective() {
    fact!(AdjMasc => [v]);
    static FORMS: &[&str] = &["nomn", "masc"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("московским")
        .interpretation(AdjMasc::v.inflected(FORMS))
        .interpretation_fact::<AdjMasc>()
        .build(rid);

    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("московским").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(
        record.get("v"),
        Some(&FactValue::Str("московский".to_string()))
    );
}

#[test]
fn test_inflected_with_nomn_sing_for_month() {
    fact!(MonthNomn => [v]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("июня")
        .interpretation(MonthNomn::v.inflected(FORMS))
        .interpretation_fact::<MonthNomn>()
        .build(rid);

    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("июня").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.get("v"), Some(&FactValue::Str("июнь".to_string())));
}

#[test]
fn test_inflected_with_datv_plur_for_noun() {
    fact!(NounDatvPlur => [v]);
    static FORMS: &[&str] = &["datv", "plur"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("абажура")
        .interpretation(NounDatvPlur::v.inflected(FORMS))
        .interpretation_fact::<NounDatvPlur>()
        .build(rid);

    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("абажура").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(
        record.get("v"),
        Some(&FactValue::Str("абажурам".to_string()))
    );
}

#[test]
fn test_attribute_normalized() {
    fact!(FNorm => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("LoNDon")
        .interpretation(FNorm::a.normalized())
        .interpretation_fact::<FNorm>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("LoNDon").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FNorm");
    assert_eq!(record.get("a"), Some(&FactValue::Str("london".to_string())));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 6)]);
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("london".to_string()))]
    );
}

#[test]
fn test_attribute_normalized_lemmatizes_russian_word() {
    fact!(FNormRu => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("июня")
        .interpretation(FNormRu::a.normalized())
        .interpretation_fact::<FNormRu>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("июня").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FNormRu");
    assert_eq!(record.get("a"), Some(&FactValue::Str("июнь".to_string())));
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("июнь".to_string()))]
    );
}

#[test]
fn test_attribute_normalized_custom_maps_month_to_number() {
    fact!(FMonth => [month]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule =
        term("июня")
            .interpretation(FMonth::month.normalized().custom(
                |value: String| match value.as_str() {
                    "июнь" => 6_i64,
                    _ => 0_i64,
                },
            ))
            .interpretation_fact::<FMonth>()
            .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("июня").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FMonth");
    assert_eq!(record.get("month"), Some(&FactValue::Int(6)));
    assert_eq!(
        record.as_json(),
        vec![("month".to_string(), FactValue::Int(6))]
    );
}

#[test]
fn test_inflected_custom_attribute() {
    fact!(F => [a]);
    static FORMS: &[&str] = &["nomn", "sing"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("январе")
        .interpretation(
            F::a.inflected(FORMS)
                .custom(|value: String| match value.as_str() {
                    "январь" => 1_i64,
                    _ => 0_i64,
                }),
        )
        .interpretation_fact::<F>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("январе").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "F");
    assert_eq!(record.get("a"), Some(&FactValue::Int(1)));
    assert_eq!(record.as_json(), vec![("a".to_string(), FactValue::Int(1))]);
}

#[test]
fn test_inflected_custom() {
    let interp = NormalizerInterpretator::new(
        InflectedNormalizer::new(Some(vec!["nomn".to_string(), "sing".to_string()]))
            .custom(|value: String| match value.as_str() {
                "январь" => 1_i64,
                _ => 0_i64,
            })
            .into(),
    );

    let out = interp
        .call(InterpretatorInput::from_tokens(
            Tokenizer::new().tokenize("январе"),
            None,
        ))
        .expect("expected normalizer output");

    assert_eq!(out.normalized_value(), FactValue::Int(1));
}

#[test]
fn test_attribute_const() {
    fact!(FConst => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = term("x")
        .interpretation(FConst::a.r#const(1_i64))
        .interpretation_fact::<FConst>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("x").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FConst");
    assert_eq!(record.get("a"), Some(&FactValue::Int(1)));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, 1)]);
    assert_eq!(record.as_json(), vec![("a".to_string(), FactValue::Int(1))]);
}

#[test]
fn test_attribute_inflected() {
    fact!(FInflect => [a]);
    static FORMS: &[&str] = &["nomn", "plur"];

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let source = "январе";
    let rule = term(source)
        .interpretation(FInflect::a.inflected(FORMS))
        .interpretation_fact::<FInflect>()
        .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match(source).expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FInflect");
    assert_eq!(record.get("a"), Some(&FactValue::Str("январи".to_string())));
    assert_eq!(record.spans, vec![crate::span::Span::new(0, source.len())]);
    assert_eq!(
        record.as_json(),
        vec![("a".to_string(), FactValue::Str("январи".to_string()))]
    );
}

#[test]
fn test_repeatable() {
    fact!(FRepeat => [a]);

    let mut registry = RuleRegistry::new();
    let rid = registry.next_id();
    let rule = (pred(eq("a")).interpretation(FRepeat::a.repeatable())
        + pred(eq("b")).interpretation(FRepeat::a.repeatable()))
    .interpretation_fact::<FRepeat>()
    .build(rid);
    registry.add(rule);
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("a b").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");

    assert_eq!(record.name(), "FRepeat");
    assert_eq!(
        record.get("a"),
        Some(&FactValue::List(vec![
            FactValue::Str("a".to_string()),
            FactValue::Str("b".to_string()),
        ]))
    );
    assert_eq!(
        record.spans,
        vec![crate::span::Span::new(0, 1), crate::span::Span::new(2, 3)]
    );
    assert_eq!(
        record.as_json(),
        vec![(
            "a".to_string(),
            FactValue::List(vec![
                FactValue::Str("a".to_string()),
                FactValue::Str("b".to_string()),
            ]),
        )]
    );
}

#[test]
fn test_pipeline_key() {
    use crate::pipeline::morph_pipeline;

    fact!(F => [a]);

    let pipeline = morph_pipeline(["закрытое общество", "завод"]);

    // Python:
    // RULE = pipeline.interpretation(F.a.normalized()).interpretation(F)
    let mut registry = RuleRegistry::new();
    let rid = registry.add(
        pipeline
            .clone()
            .interpretation(F::a.normalized())
            .interpretation_fact::<F>(),
    );
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("закрытом обществе").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");
    assert_eq!(
        record.get("a"),
        Some(&FactValue::Str("закрытое общество".to_string()))
    );

    // Второй прогон с тем же pipeline
    let mut registry = RuleRegistry::new();
    let rid = registry.add(
        pipeline
            .interpretation(F::a.normalized())
            .interpretation_fact::<F>(),
    );
    registry.validate().unwrap();

    let parser = Parser::new(&registry, rid);
    let m = parser.r#match("заводе").expect("expected match");
    let record = m.fact(&registry).expect("expected fact");
    assert_eq!(record.get("a"), Some(&FactValue::Str("завод".to_string())));
}
