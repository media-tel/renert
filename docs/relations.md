# Согласование

Модуль `relations` нужен для морфологического согласования токенов внутри одного матча:
род, число, падеж или их комбинации. Это не отдельный фильтр поверх текста, а часть
подготовки дерева разбора: несовместимые морфологические формы отбрасываются, и если
форм не остаётся — матч не проходит.

Ключевые элементы API:

- базовые отношения: `gender_relation`, `number_relation`, `case_relation`, `gnc_relation`;
- логические композиции: `and_relation`, `or_relation`, `not_relation`;
- привязка отношения к правилу: `RuleBuilderRelationExt::match_relation`.

## Быстрый старт

```rust
use renert::fact;
use renert::interpretation::{FactValue, RuleInterpretation};
use renert::predicates::gram;
use renert::relations::{gnc_relation, RuleBuilderRelationExt};
use renert::{pred, rule, Parser, RuleRegistry};

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
let m = parser.find("саше иванову").expect("expected match");
let record = m.fact(&registry).expect("expected Name fact");

assert_eq!(record.name(), "Name");
assert_eq!(record.get("first"), Some(&FactValue::Str("саша".to_string())));
assert_eq!(record.get("last"), Some(&FactValue::Str("иванов".to_string())));
```

## Какие отношения есть

### `gender_relation()`

Согласование по роду.

### `number_relation()`

Согласование по числу (`sing/plur`, плюс учёт `Sgtm/Pltm`).

### `case_relation()`

Согласование по падежу.

### `gnc_relation()`

Комбинированное согласование по роду + числу + падежу
(по сути `gender && number && case`).

## Как правильно привязывать relation к правилам

`match_relation` навешивается на `RuleBuilder`:

```rust
let relation = gnc_relation();
let a = pred(gram("Name")).match_relation(relation.clone());
let b = pred(gram("Surn")).match_relation(relation);
```

Важный нюанс: для одной группы согласования нужен **один и тот же** экземпляр `Arc<dyn Relation>`
(обычно через `clone()`).  
Если создать два независимых `gnc_relation()`, это будут разные группы и между ними не
появится связь.

## Когда матч отбрасывается

Если формы двух (или более) связанных токенов несовместимы, relation-граф сужает набор
форм. Если у какого-то токена набор форм становится пустым, матч отбрасывается.

Пример несовместимого набора:

```rust
use renert::predicates::gram;
use renert::relations::{gnc_relation, RuleBuilderRelationExt};
use renert::{pred, rule, Parser, RuleRegistry};

let gnc = gnc_relation();
let first_rule = pred(gram("Name")).match_relation(gnc.clone());
let last_rule = pred(gram("Surn")).match_relation(gnc);

let name_rule = rule([first_rule, last_rule]);

let mut registry = RuleRegistry::new();
let rid = registry.add(name_rule.build(()));
let parser = Parser::new(&registry, rid);

assert!(parser.r#match("сашу ивановой").is_none());
```

## Логические композиции отношений

Можно комбинировать отношения:

```rust
use renert::relations::{and_relation, gender_relation, number_relation, RuleBuilderRelationExt};

let rel = and_relation(vec![number_relation(), gender_relation()]);
let a = some_rule.match_relation(rel.clone());
let b = another_rule.match_relation(rel);
```

Также доступны:

- `or_relation(vec![...])` — хотя бы одно отношение;
- `not_relation(relation)` — отрицание.

## `main_term` и relation на группе

Если relation навешан на составное правило (группа из нескольких термов), для выбора
главного токена внутри группы используется `Production::main` (задаётся через `main_term`).
Это важно для сценариев, где группа должна согласовываться с внешним токеном.

Пример:

```rust
use renert::predicates::gram;
use renert::relations::{
    and_relation, gender_relation, number_relation, RuleBuilderRelationExt,
};
use renert::{main_term, pred, rule, Parser, RuleRegistry};

let relation = and_relation(vec![number_relation(), gender_relation()]);

let a = rule([pred(gram("Surn")), main_term(pred(gram("Name")))])
    .match_relation(relation.clone());
let b = pred(gram("VERB")).match_relation(relation);
let ab = rule([a, b]);

let mut registry = RuleRegistry::new();
let rid = registry.add(ab.build(()));
let parser = Parser::new(&registry, rid);

assert!(parser.r#match("иванов иван стал").is_some());
assert!(parser.r#match("иванов иван стали").is_none());
```

## Как это работает внутри

1. Во время разбора relation-обёртки попадают в дерево.
2. На этапе подготовки матча собирается relation-граф по токенам.
3. `validate()` попарно сужает формы совместимых токенов.
4. Если у токена форм не осталось, матч отбрасывается.
5. Иначе суженные формы применяются к листьям дерева.

## Типичные ошибки

- Навесили `match_relation` только на один терм — согласовываться не с кем.
- Использовали разные экземпляры relation вместо `clone()` одного `Arc`.
- Ожидаете эффект на plain-токенах без морфологических форм.
- Не поставили `main_term` в сложной группе, где важен главный токен.

## Шпаргалка

| Задача | Что использовать |
| --- | --- |
| Согласовать имя и фамилию по ГЧП | `gnc_relation()` + `.match_relation(...)` на оба терма |
| Согласовать только по числу | `number_relation()` |
| Согласовать по числу и роду | `and_relation(vec![number_relation(), gender_relation()])` |
| Разрешить один из вариантов | `or_relation(vec![...])` |
| Инвертировать условие | `not_relation(...)` |

## Пример 

Ниже представлен пример использования согласования по падежу, роду и числу на примере фамилий.
```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::gram;
use renert::relations::{gnc_relation, RuleBuilderRelationExt};
use renert::{pred, rule, Parser, RuleRegistry};

fn main() {
    fact!(Name => [first, last]);
    static FORMS: &[&str] = &["nomn", "sing"];
    
    let gnc = gnc_relation();

    let name_rule = rule([
        pred(gram("Name"))
            .interpretation(Name::first.inflected(FORMS))
            .match_relation(gnc.clone()),
        pred(gram("Surn"))
            .interpretation(Name::last.inflected(FORMS))
            .match_relation(gnc),
    ])
    .interpretation_fact::<Name>();

    let mut registry = RuleRegistry::new();
    let rid = registry.add(name_rule.build(()));
    registry.validate().expect("registry must be valid");
    let parser = Parser::new(&registry, rid);
    for m in parser.findall("Сашу Иванову, Саше Иванову") {
        if let Some(fact) = m.fact(&registry) {
            println!("{fact}");
        }
    }
}
```
Output
```
Name(
    first='саша',
    last='иванова'
)
Name(
    first='саша',
    last='иванов'
)
```
