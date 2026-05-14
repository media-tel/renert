# Интерпретация

Интерпретация в `renert` нужна, чтобы превратить совпавшее правило в структурированный
факт: с именем, полями, типизированными значениями и диапазонами в тексте. Чтобы использовать интерпретацию, необходимо подключить `renert::interpretation::RuleInterpretation` для возможности использовать методы модуля `interpretation`.

Коротко:

1. Описываете схему факта (`fact!` или `fact(...)`);
2. На терминалах/подправилах задаёте, в какое поле писать значение
   (`.interpretation(...)`);
3. На корневом правиле ставите `.interpretation_fact::<FactType>()`;
4. После парсинга читаете факт из `Match`: `fact(&registry)` или `facts(&registry)`.

## Быстрый старт

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleRegistry};

fact!(Position => [name]);

static FORMS: &[&str] = &["nomn", "sing"];

let rule = morph_pipeline(["премьер министр", "президент"])
    .interpretation(Position::name.inflected(FORMS))
    .interpretation_fact::<Position>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(rule);
registry.validate().expect("registry must be valid");

let parser = Parser::new(&registry, root_id);
let m = parser.find("президент").expect("match expected");
let fact = m.fact(&registry).expect("fact expected");

assert_eq!(fact.name(), "Position");
println!("{:?}", fact.as_json());
println!("{}", fact.to_json_value());
```

## 1) Как описать факт

### Вариант A: типизированный `fact!` (рекомендуется)

```rust
use renert::fact;

fact!(Person => [first, last, position]);
```

Это создаёт тип с метаданными (`FactMeta`) и дескрипторами полей (`Person::first`,
`Person::last`, ...), которые можно передавать в `.interpretation(...)`.

### Вариант B: динамический `fact(...)`

Полезно, когда схема создаётся программно во время выполнения.

```rust
use renert::interpretation::{fact, FactAttributeInput};

let city_scheme = fact(
    "City",
    vec![
        FactAttributeInput::from("name"),
        FactAttributeInput::from("kind"),
    ],
);
```

## 2) Как связать правило и поля факта

Связка делается методом `RuleInterpretation::interpretation(...)`.

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::gram;
use renert::{pred, rule};

fact!(Name => [first, last]);

let name_rule = rule([
    pred(gram("Name")).interpretation(Name::first),
    pred(gram("Surn")).interpretation(Name::last),
]);
```

После этого правило уже знает, какие куски совпадения идут в какие поля.

## 3) Как обозначить корневой факт

Чтобы из матча можно было получить объект факта, корневое правило помечают:

```rust
let root = name_rule.interpretation_fact::<Name>();
```

Затем правило добавляется в `RuleRegistry` и используется в `Parser`.

## 4) Нормализация и трансформации значений

В `renert` основные методы трансформации значений факта такие же по смыслу, как в
Yargy: `normalized`, `inflected`, `custom` и `const` (`r#const` в Rust, потому что
`const` — ключевое слово языка).

### `normalized()`

Без `normalized()` в поле попадает поверхностная форма из текста:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::{dictionary, gte, lte};
use renert::{and, pred, Parser, RuleRegistry};

fact!(Date => [year, month, day]);

static MONTHS: &[&str] = &[
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
];

let day = pred(and(vec![gte(1), lte(31)]));
let month_name = pred(dictionary(MONTHS));
let year = pred(and(vec![gte(1900), lte(2100)]));

let date = (day.interpretation(Date::day)
    + month_name.interpretation(Date::month)
    + year.interpretation(Date::year))
    .interpretation_fact::<Date>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(date.build(()));
let parser = Parser::new(&registry, root_id);

let m = parser.r#match("8 июня 2015").expect("match expected");
println!("{}", m.fact(&registry).unwrap());
```

Output:

```text
Date(
    year='2015',
    month='июня',
    day='8'
)
```

С `normalized()` слово `"июня"` меняется на `"июнь"`:

```rust
let date = (day.interpretation(Date::day)
    + month_name.interpretation(Date::month.normalized())
    + year.interpretation(Date::year))
    .interpretation_fact::<Date>();
```

Output:

```text
Date(
    year='2015',
    month='июнь',
    day='8'
)
```

Если в `normalized()` попадает несколько токенов, каждый приводится к нормальной форме
отдельно, без согласования:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::normalized;
use renert::{pred, rule, Parser, RuleRegistry};

fact!(Geo => [name]);

let geo = rule([
    pred(normalized("Красная")),
    pred(normalized("площадь")),
])
.interpretation(Geo::name.normalized())
.interpretation_fact::<Geo>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(geo.build(()));
let parser = Parser::new(&registry, root_id);

for m in parser.findall("на Красной площади") {
    println!("{}", m.fact(&registry).unwrap());
}
```

Output:

```text
Geo(
    name='красный площадь'
)
```

После газеттира результат `normalized()` — это ключ газеттира:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleBuilder, RuleRegistry};

fact!(Geo => [name]);

let geo = RuleBuilder::from_arc(morph_pipeline([
    "красная площадь",
    "первомайская улица",
]))
.interpretation(Geo::name.normalized())
.interpretation_fact::<Geo>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(geo.build(()));
let parser = Parser::new(&registry, root_id);

for m in parser.findall("с Красной площади на Первомайскую улицу") {
    println!("{}", m.fact(&registry).unwrap());
}
```

Output:

```text
Geo(
    name='красная площадь'
)
Geo(
    name='первомайская улица'
)
```

### `inflected(forms)`

`inflected(forms)` склоняет значение к заданным граммемам. Для канонического вида часто
используют именительный падеж и единственное число:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::gram;
use renert::{pred, Parser, RuleRegistry};

fact!(Name => [first]);

static FORMS: &[&str] = &["nomn", "sing"];

let name = pred(gram("Name"))
    .interpretation(Name::first.inflected(FORMS))
    .interpretation_fact::<Name>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(name.build(()));
let parser = Parser::new(&registry, root_id);

for m in parser.findall("Саше, Маше, Вадиму") {
    println!("{}", m.fact(&registry).unwrap());
}
```

Output:

```text
Name(
    first='саша'
)
Name(
    first='маша'
)
Name(
    first='вадим'
)
```

`inflected` принимает любой набор граммем:

```rust
static FORMS: &[&str] = &["accs", "plur"];

let name = pred(gram("Name"))
    .interpretation(Name::first.inflected(FORMS))
    .interpretation_fact::<Name>();
```

### `custom(...)`

`custom(...)` применяет к значению произвольную функцию:

```rust
use renert::fact;
use renert::interpretation::{FactValue, RuleInterpretation};
use renert::predicates::is_token_type;
use renert::{pred, term, Parser, RuleRegistry};

fact!(Float => [value]);

fn parse_float_string(value: FactValue) -> String {
    value.as_str().unwrap().to_string()
}

let int = pred(is_token_type("Int"));
let float = (int.clone() + term(".") + int)
    .interpretation(Float::value.custom(parse_float_string))
    .interpretation_fact::<Float>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(float.build(()));
let parser = Parser::new(&registry, root_id);

let m = parser.r#match("3.1415").expect("match expected");
println!("{}", m.fact(&registry).unwrap());
```

Output:

```text
Float(
    value='3.1415'
)
```

`custom(...)` можно применять вместе с `normalized()`: сначала слово ставится в нормальную
форму, потом к нему применяется функция.

```rust
use std::collections::HashMap;
use renert::fact;
use renert::interpretation::{FactValue, RuleInterpretation};
use renert::predicates::{dictionary, gte, lte};
use renert::{and, pred, Parser, RuleRegistry};

fact!(Date => [year, month, day]);

static MONTHS: &[&str] = &[
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
];

fn parse_int(value: FactValue) -> i64 {
    value.as_str().unwrap().parse::<i64>().unwrap()
}

let months: HashMap<&'static str, i64> = HashMap::from([
    ("январь", 1), ("февраль", 2), ("март", 3), ("апрель", 4),
    ("май", 5), ("июнь", 6), ("июль", 7), ("август", 8),
    ("сентябрь", 9), ("октябрь", 10), ("ноябрь", 11), ("декабрь", 12),
]);

let day = pred(and(vec![gte(1), lte(31)]));
let month_name = pred(dictionary(MONTHS));
let year = pred(and(vec![gte(1900), lte(2100)]));

let date = (day.interpretation(Date::day.custom(parse_int))
    + month_name.interpretation(
        Date::month.normalized().custom(move |value: String| {
            months.get(value.as_str()).copied().unwrap()
        })
    )
    + year.interpretation(Date::year.custom(parse_int)))
    .interpretation_fact::<Date>();
```

Для `"8 июня 2015"` получится:

```text
Date(
    year=2015,
    month=6,
    day=8
)
```

### `r#const(value)` / `with_const("...")`

`const` заменяет слово или словосочетание фиксированным значением. В Rust метод называется
`r#const(...)`, потому что `const` — ключевое слово:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::{or_, Parser, RuleBuilder, RuleRegistry};

fact!(Era => [value]);

let bc = RuleBuilder::from_arc(
    morph_pipeline(["до нашей эры", "до н.э."])
        .interpretation(Era::value.r#const("BC"))
);

let ad = RuleBuilder::from_arc(
    morph_pipeline(["наша эра", "н.э."])
        .interpretation(Era::value.r#const("AD"))
);

let era = or_([bc, ad]).interpretation_fact::<Era>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(era.build(()));
let parser = Parser::new(&registry, root_id);

for m in parser.findall("наша эра, до н.э.") {
    println!("{}", m.fact(&registry).unwrap());
}
```

Output:

```text
Era(
    value='AD'
)
Era(
    value='BC'
)
```
### `repeatable()`

Поле может принимать несколько значений (список).

```rust
fact!(Tags => [tag]);

let rule = pred(gram("NOUN"))
    .repeatable()
    .interpretation(Tags::tag.repeatable())
    .interpretation_fact::<Tags>();
```

## 5) Извлечение фактов из `Match`

После `parser.find(...)` / `findall(...)`:

```rust
let m = parser.find(text).expect("match expected");
```

доступно:

- `m.fact(&registry)` — один факт (обычно корневой);
- `m.facts(&registry)` — все факты из дерева, включая вложенные.

Пример:

```rust
if let Some(fact) = m.fact(&registry) {
    println!("name: {}", fact.name());
    println!("year: {:?}", fact.get("year"));
    println!("as_json: {:?}", fact.as_json());
    println!("json_value: {}", fact.to_json_value());
}
```

## 6) `as_json()` vs `to_json_value()`

- `as_json()` — внутреннее типизированное представление:
  `Vec<(String, FactValue)>`;
- `to_json_value()` — готовый `serde_json::Value` формата:
  `{ "name": ..., "fields": ..., "spans": ... }`.

`spans` — диапазоны текста, из которого собран факт.

## 7) Полный пример: дата

```rust
use std::collections::HashMap;
use renert::fact;
use renert::interpretation::{FactValue, RuleInterpretation};
use renert::predicates::{dictionary, gte, lte};
use renert::{and, pred, term, Parser, RuleRegistry};

fact!(Date => [year, month, day]);

static MONTHS: &[&str] = &[
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
];

fn parse_int(value: FactValue) -> i64 {
    value.as_str().unwrap().parse::<i64>().unwrap()
}

let months: HashMap<&'static str, i64> = HashMap::from([
    ("январь", 1), ("февраль", 2), ("март", 3), ("апрель", 4),
    ("май", 5), ("июнь", 6), ("июль", 7), ("август", 8),
    ("сентябрь", 9), ("октябрь", 10), ("ноябрь", 11), ("декабрь", 12),
]);

let month_name = pred(dictionary(MONTHS));
let month_num = pred(and(vec![gte(1), lte(12)]));
let day = pred(and(vec![gte(1), lte(31)]));
let year = pred(and(vec![gte(1900), lte(2100)]));

let date_rule = (
    day.clone().interpretation(Date::day.custom(parse_int))
        + month_name.interpretation(
            Date::month.normalized().custom(move |value: String| {
                months.get(value.as_str()).copied().unwrap()
            })
        )
        + year.clone().interpretation(Date::year.custom(parse_int))
) | (
    year.clone().interpretation(Date::year.custom(parse_int))
        + term("-")
        + month_num.interpretation(Date::month.custom(parse_int))
        + term("-")
        + day.interpretation(Date::day.custom(parse_int))
);

let root = date_rule.interpretation_fact::<Date>().build(());

let mut registry = RuleRegistry::new();
let root_id = registry.add(root);
registry.validate().expect("registry must be valid");

let parser = Parser::new(&registry, root_id);
let m = parser.find("18 июня 2016").expect("match expected");
let fact = m.fact(&registry).expect("fact expected");

println!("{}", fact);
// Date(year=2016, month=6, day=18) в человекочитаемом виде
```

## 8) Практические советы

- Для большинства задач используйте `fact!` + `interpretation_fact::<T>()` — меньше
  ошибок и лучше автодополнение.
- Всегда вызывайте `registry.validate()` на сложных грамматиках с интерпретациями.
- Если `fact(&registry)` возвращает `None`, проверьте:
  1) что корневое правило размечено `interpretation_fact`;
  2) что нужные поля реально помечены `.interpretation(...)`;
  3) что матч действительно найден (`find`/`findall`).
- Для внешнего API/БД обычно удобнее `to_json_value()`;
  для логики в Rust — `as_json()`/`get(...)` с `FactValue`.

## 9) Продвинутые примеры (из `src/interpretation/mod.rs`)

Ниже — те же сценарии, что в rustdoc модуля `interpretation`, но встроенные в формат
этого гайда.

### 9.1 Извлечение дат в разных форматах

Идея: одной грамматикой разобрать несколько форматов даты и собрать единый `Date` факт.

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::predicates::{dictionary, gte, lte};
use renert::{and, pred, term, Parser, RuleRegistry};

fact!(Date => [year, month, day]);

static MONTHS: &[&str] = &[
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
];

let month_name = pred(dictionary(MONTHS));
let month_num = pred(and(vec![gte(1), lte(12)]));
let day = pred(and(vec![gte(1), lte(31)]));
let year = pred(and(vec![gte(1900), lte(2100)]));

let date_rule = (
    day.clone().interpretation(Date::day)
        + month_name.interpretation(Date::month)
        + year.clone().interpretation(Date::year)
) | (
    year.clone().interpretation(Date::year)
        + term("-")
        + month_num.clone().interpretation(Date::month)
        + term("-")
        + day.clone().interpretation(Date::day)
) | (
    year.interpretation(Date::year) + term("г") + term(".")
);

let mut registry = RuleRegistry::new();
let root_id = registry.add(date_rule.interpretation_fact::<Date>().build(()));
registry.validate().expect("registry must be valid");

let parser = Parser::new(&registry, root_id);
let text = "2015г.\n18 июля 2016\n2016-01-02\n";

for m in parser.findall(text) {
    if let Some(fact) = m.fact(&registry) {
        println!("{}", fact);
        println!("year = {:?}", fact.get("year"));
        println!("as_json = {:?}\n", fact.as_json());
    }
}
```
Output
```
Date(
    year='2015',
    month=None,
    day=None
)
year = Some(Str("2015"))
as_json = [("year", Str("2015"))]
```

Что показывает пример:
- одна `or`-грамматика собирает несколько форматов записи;
- `fact.get("...")` удобно для точечной проверки;
- `as_json()` даёт компактную проекцию только заполненных полей.

### 9.2 Нормализация и кастомные преобразования

Идея: не просто вытащить `"18 июня 2016"`, а получить `day=18, month=6, year=2016`.

```rust
use std::collections::HashMap;
use renert::fact;
use renert::interpretation::{FactValue, RuleInterpretation};
use renert::predicates::{dictionary, gte, lte};
use renert::{and, pred, term, Parser, RuleRegistry};

fact!(Date => [year, month, day]);

static MONTHS: &[&str] = &[
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
];

fn parse_int(value: FactValue) -> i64 {
    value.as_str().unwrap().parse::<i64>().unwrap()
}

let months: HashMap<&'static str, i64> = HashMap::from([
    ("январь", 1), ("февраль", 2), ("март", 3), ("апрель", 4),
    ("май", 5), ("июнь", 6), ("июль", 7), ("август", 8),
    ("сентябрь", 9), ("октябрь", 10), ("ноябрь", 11), ("декабрь", 12),
]);

let month_name = pred(dictionary(MONTHS));
let month_num = pred(and(vec![gte(1), lte(12)]));
let day = pred(and(vec![gte(1), lte(31)]));
let year = pred(and(vec![gte(1900), lte(2100)]));

let date_rule = (
    day.clone().interpretation(Date::day.custom(parse_int))
        + month_name.interpretation(
            Date::month.normalized().custom(move |value: String| {
                months.get(value.as_str()).copied().unwrap()
            })
        )
        + year.clone().interpretation(Date::year.custom(parse_int))
) | (
    year.clone().interpretation(Date::year.custom(parse_int))
        + term("-")
        + month_num.interpretation(Date::month.custom(parse_int))
        + term("-")
        + day.interpretation(Date::day.custom(parse_int))
);

let mut registry = RuleRegistry::new();
let root_id = registry.add(date_rule.interpretation_fact::<Date>().build(()));
registry.validate().expect("registry must be valid");

let parser = Parser::new(&registry, root_id);
let m = parser.r#match("18 июня 2016").expect("expected match");
let fact = m.fact(&registry).expect("expected date fact");

println!("{}", fact);
```
Output
```
Date(
    year=2016,
    month=6,
    day=18
)
```

Здесь важна цепочка:
- `normalized()` приводит `"июня"` к лемме `"июнь"`;
- `custom(...)` переводит лемму месяца в номер;
- `custom(parse_int)` приводит числовые строки к `i64`.

### 9.3 Инфлексия для канонической формы факта

Идея: вход может быть в косвенном падеже, а в факте хранить каноническую форму (без сохранения прописных букв).

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::predicates::gram;
use renert::{pred, rule, Parser, RuleBuilder, RuleRegistry};

fact!(Person => [position, name]);
fact!(Name => [first, last]);

static FORMS: &[&str] = &["nomn", "sing"];

fn build_person_rule<'a>() -> RuleBuilder<'a> {
    let position = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]));

    let name = rule([
        pred(gram("Name")).interpretation(Name::first.inflected(FORMS)),
        pred(gram("Surn")).interpretation(Name::last.inflected(FORMS)),
    ])
    .interpretation_fact::<Name>();

    rule([
        position.interpretation(Person::position.inflected(FORMS)),
        name.interpretation(Person::name),
    ])
    .interpretation_fact::<Person>()
}

let mut registry = RuleRegistry::new();
let root_id = registry.add(build_person_rule().build(()));
registry.validate().expect("registry must be valid");

let parser = Parser::new(&registry, root_id);
let text = "по приказу президента Владимира Путина";

for m in parser.findall(text) {
    if let Some(fact) = m.fact(&registry) {
        println!("{}", fact);
    }
}
```
Output
```
Person(
    position='президент',
    name={first: 'владимир', last: 'путин'}
)
```

Практический эффект:
- в тексте: `президента`, `Владимира`, `Путина`;
- в факте: `президент`, `владимир`, `путин`.

После inflected(FORMS) морф-библиотека отдаёт строчные буквы. Чтобы получить заглавную первую букву, нужно добавить .custom(...) в цепочку — InflectedAttribute поддерживает это через inflected(...).custom(fn), где функция принимает String (уже инфлексированную форму).

Добавьте вспомогательную функцию и измените вызовы интерпретации:
```rust
fn capitalize(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn build_person_rule<'a>() -> RuleBuilder<'a> {
    let position = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]));

    let name = rule([
        pred(gram("Name")).interpretation(Name::first.inflected(FORMS).custom(capitalize)),
        pred(gram("Surn")).interpretation(Name::last.inflected(FORMS).custom(capitalize)),
    ])
    .interpretation_fact::<Name>();

    rule([
        position.interpretation(Person::position.inflected(FORMS).custom(capitalize)),
        name.interpretation(Person::name),
    ])
    .interpretation_fact::<Person>()
}
```

Тогда программа будет выводить:
```
Person(
    position='Президент',
    name={first: 'Владимир', last: 'Путин'}
)
```