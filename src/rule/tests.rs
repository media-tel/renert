use super::builder::{or_, pred, rule, term, RuleBuilder};
use crate::predicates::constructors::{eq, gram, in_, not};

macro_rules! assert_bnf {
    ($builder:expr, $($line:expr),+ $(,)?) => {{
        assert_bnf($builder, &[$($line),+]);
    }};
}

fn assert_bnf(builder: RuleBuilder<'_>, bnf: &[&str]) {
    let actual = builder.normalized().as_bnf();
    let actual: Vec<&str> = actual.lines().collect();
    let expected: Vec<&str> = bnf.to_vec();
    assert_eq!(actual, expected);
}

fn rule_a() -> RuleBuilder<'static> {
    term("a")
}

#[test]
fn test_repeatable_optional() {
    let a = rule_a();

    assert_bnf!(
        a.clone().optional().repeatable_with(None, None, false),
        "R0 -> e | 'a' R0 | 'a'",
    );
    assert_bnf!(a.clone().repeatable().optional(), "R0 -> e | 'a' R0 | 'a'",);
    assert_bnf!(
        a.clone()
            .repeatable()
            .optional()
            .repeatable_with(None, None, false),
        "R0 -> e | 'a' R0 | 'a'",
    );
    assert_bnf!(
        a.clone().repeatable().repeatable_with(None, None, false),
        "R0 -> 'a' R0 | 'a'",
    );
    assert_bnf!(a.clone().optional().optional(), "R0 -> e | 'a'",);
    assert_bnf!(
        a.clone()
            .repeatable_with(None, Some(2), false)
            .repeatable_with(None, None, false),
        "R0 -> 'a' R0 | 'a'",
    );
    assert_bnf!(
        a.clone()
            .repeatable()
            .repeatable_with(Some(1), Some(2), false),
        "R0 -> 'a' R0 | 'a'",
    );
    assert_bnf!(
        a.clone().optional().repeatable_with(None, Some(2), false),
        "R0 -> e | R1",
        "R1 -> 'a' 'a' | 'a'",
    );
    assert_bnf!(
        a.clone().repeatable_with(None, None, true).optional(),
        "R0 -> e | 'a' | 'a' R0",
    );
    assert_bnf!(
        a.clone().repeatable().repeatable_with(None, None, true),
        "R0 -> 'a' | 'a' R0",
    );
    assert_bnf!(
        a.clone()
            .repeatable_with(None, None, true)
            .repeatable_with(Some(1), Some(2), false),
        "R0 -> 'a' | 'a' R0",
    );
    assert_bnf!(
        a.clone().repeatable().repeatable_with(Some(2), None, true),
        "R0 -> 'a' R0 | 'a'",
    );
    assert_bnf!(
        a.repeatable_with(None, Some(2), true),
        "R0 -> 'a' | 'a' 'a'",
    );
}

#[test]
fn test_or() {
    assert_bnf!(or_([term("a"), term("b")]).named("A"), "A -> 'a' | 'b'",);
}

#[test]
fn test_flatten() {
    assert_bnf!(rule([rule([term("a")])]), "R0 -> 'a'",);
}

#[test]
fn test_loop() {
    let a = super::builder::forward();
    let b = a.clone().named("A");
    a.define(b);

    assert_bnf!(a, "A -> A");
}

#[test]
fn test_bounded() {
    let a = rule_a();

    assert_bnf!(
        a.clone().repeatable_with(None, Some(3), false),
        "R0 -> 'a' R1 | 'a'",
        "R1 -> 'a' 'a' | 'a'",
    );
    assert_bnf!(
        a.clone().repeatable_with(Some(2), None, false),
        "R0 -> 'a' R1",
        "R1 -> 'a' R1 | 'a'",
    );
    assert_bnf!(
        a.repeatable_with(Some(2), Some(3), false),
        "R0 -> 'a' R1",
        "R1 -> 'a' 'a' | 'a'",
    );
}

#[test]
fn test_demo_pred_named_bnf() {
    assert_bnf!(
        pred(gram("NOUN")).named("NOUN_TOKEN"),
        "NOUN_TOKEN -> gram(NOUN)",
    );
}

#[test]
fn test_demo_empty_helpers_return_epsilon() {
    let empty_vec: Vec<RuleBuilder<'static>> = Vec::new();
    assert_bnf!(rule(empty_vec.clone()), "R0 -> e");
    assert_bnf!(or_(empty_vec), "R0 -> e");
}

#[test]
fn test_demo_reverse_optional_order() {
    assert_bnf!(rule_a().optional_with(true), "R0 -> 'a' | e",);
}

#[test]
fn test_grammar_key_value_size() {
    let key = or_([rule([term("р"), term(".")]), rule([term("размер")])]).named("KEY");

    let value = or_([rule([term("S")]), rule([term("M")]), rule([term("L")])]).named("VALUE");

    let size = rule([key, value]).named("SIZE");

    assert_bnf!(
        size,
        "SIZE -> KEY VALUE",
        "KEY -> 'р' '.' | 'размер'",
        "VALUE -> 'S' | 'M' | 'L'",
    );
}

#[test]
fn test_grammar_with_in_predicate() {
    let key = or_([rule([term("р"), term(".")]), rule([term("размер")])]).named("KEY");

    let value = pred(in_(&["S", "M", "L"])).named("VALUE");
    let size = rule([key, value]).named("SIZE");

    assert_bnf!(
        size,
        "SIZE -> KEY VALUE",
        "KEY -> 'р' '.' | 'размер'",
        "VALUE -> in_(...)",
    );
}

#[test]
fn test_recursive_expr_forward() {
    let expr = super::builder::forward();
    expr.define(
        or_([
            rule([term("a")]),
            rule([term("("), expr.clone(), term("+"), expr.clone(), term(")")]),
        ])
        .named("EXPR"),
    );

    assert_bnf!(expr, "EXPR -> 'a' | R0 EXPR ')'", "R0 -> '(' EXPR '+'",);
}

#[test]
fn test_recursive_quoted_text_forward() {
    let word = pred(not(eq("»")));

    let text = super::builder::forward();
    text.define(or_([
        rule([word.clone()]),
        rule([word.clone(), text.clone()]),
    ]));

    let title = rule([term("«"), text, term("»")]).named("TITLE");

    assert_bnf!(
        title.clone(),
        "TITLE -> '«' R0 '»'",
        "R0 -> not('»') | not('»') R0",
    );
}

#[test]
fn test_recursive_quoted_text_forward_users_printing() {
    let word = pred(not(eq("»")));

    let text = super::builder::forward();
    text.define(or_([
        rule([word.clone()]),
        rule([word.clone(), text.clone()]),
    ]));

    let title = rule([term("«"), text, term("»")]).named("TITLE");

    let actual = title.normalized().as_bnf();
    assert_eq!(actual, "TITLE -> '«' R0 '»'\nR0 -> not('»') | not('»') R0");
}

#[test]
fn test_quoted_text_repeatable() {
    let title = rule([term("«"), pred(not(eq("»"))).repeatable(), term("»")]).named("TITLE");

    assert_bnf!(title, "TITLE -> '«' R0 '»'", "R0 -> not('»') R0 | not('»')",);
}
