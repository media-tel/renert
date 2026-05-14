# Газеттиры (pipelines)

Газеттир — это компактный способ описать словарь фраз, которые нужно искать в тексте. В `renert` реализованы три вида газеттиров: `morph_pipeline`, `caseless_pipeline` и `pipeline`.

## Зачем нужны газеттиры

Словарь должностей или географических объектов можно описать вручную через `or_`, `pred`, `normalized`:

```rust
use renert::predicates::{caseless, normalized};
use renert::{or_, pred, rule, term};

let position = or_([
    rule([
        pred(normalized("генеральный")),
        pred(normalized("директор")),
    ]),
    rule([pred(normalized("бухгалтер"))]),
]);

let geo = or_([
    rule([
        pred(normalized("Ростов")),
        term("-"),
        pred(caseless("на")),
        term("-"),
        pred(caseless("Дону")),
    ]),
    rule([pred(normalized("Москва"))]),
]);
```

Это громоздко и легко ошибиться в словоформах. Газеттиры решают эту проблему: вы передаёте список строк — библиотека сама строит правила.

## `morph_pipeline` — морфологический газеттир

`morph_pipeline` перед сравнением приводит каждое слово к нормальной форме. Это позволяет находить слово в любом падеже, числе и роде.

```rust
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleRegistry};

let rule = morph_pipeline(["электронный дневник"]);

let mut registry = RuleRegistry::new();
let root_id = registry.add(rule);
let parser = Parser::new(&registry, root_id);

let text = "электронным дневником, электронные дневники, электронное дневнику";
for m in parser.findall(text) {
    let tokens: Vec<String> = m.tokens()
        .into_iter()
        .map(|t| t.value.into_owned())
        .collect();
    println!("{:?}", tokens);
}
```

```text
["электронным", "дневником"]
["электронные", "дневники"]
["электронное", "дневнику"]
```

Словарь может содержать любое количество фраз:

```rust
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleRegistry};

let position = morph_pipeline([
    "генеральный директор",
    "финансовый директор",
    "главный бухгалтер",
    "президент",
]);

let mut registry = RuleRegistry::new();
let root_id = registry.add(position);
let parser = Parser::new(&registry, root_id);

let text = "встреча с генеральным директором и главным бухгалтером";
for m in parser.findall(text) {
    let span = m.span();
    println!("{:?}", &text[span.start..span.stop]);
}
```

```text
"генеральным директором"
"главным бухгалтером"
```

### Как работает морфологическая нормализация

При построении словаря `morph_pipeline` нормализует каждое слово из переданных строк с помощью морфологического анализатора. Для каждого терма формируется набор возможных нормальных форм. Во время поиска токен из текста также сравнивается с нормальными формами — не с поверхностным написанием. Поэтому "дневником" (творительный падеж) совпадёт с записью "электронный дневник" (именительный).

Небуквенные токены (цифры, знаки препинания) сравниваются точно.

## `caseless_pipeline` — газеттир без учёта регистра

`caseless_pipeline` ищет фразы без морфологической нормализации, но без учёта регистра букв. Полезен для имён собственных из языков без развитой морфологии, аббревиатур, технических терминов.

```rust
use renert::pipeline::caseless_pipeline;
use renert::{Parser, RuleRegistry};

let name = caseless_pipeline([
    "Абд Аль-Азиз Бин Мухаммад",
    "Абд ар-Рахман Наср ас-Са ди",
]);

let mut registry = RuleRegistry::new();
let root_id = registry.add(name);
let parser = Parser::new(&registry, root_id);

let text = "Абд Аль-Азиз Бин Мухаммад, АБД АР-РАХМАН НАСР АС-СА ДИ";
for m in parser.findall(text) {
    let tokens: Vec<String> = m.tokens()
        .into_iter()
        .map(|t| t.value.into_owned())
        .collect();
    println!("{:?}", tokens);
}
```

```text
["Абд", "Аль", "-", "Азиз", "Бин", "Мухаммад"]
["АБД", "АР", "-", "РАХМАН", "НАСР", "АС", "-", "СА", "ДИ"]
```

Газеттир нашёл обе записи независимо от регистра: "Аль" == "АЛЬ" (без учёта регистра).

## `pipeline` — точный газеттир

`pipeline` ищет фразы с точным совпадением значения токена. Регистр и форма слова имеют значение.

```rust
use renert::pipeline::pipeline;
use renert::{Parser, RuleRegistry};

let keywords = pipeline(["OK", "N/A", "TODO"]);

let mut registry = RuleRegistry::new();
let root_id = registry.add(keywords);
let parser = Parser::new(&registry, root_id);

let text = "статус: OK, задача: TODO";
for m in parser.findall(text) {
    let span = m.span();
    println!("{}", &text[span.start..span.stop]);
}
```

```text
OK
TODO
```

## Газеттир в составном правиле

Все три функции (`pipeline`, `caseless_pipeline`, `morph_pipeline`) возвращают `Arc<Rule>`. Чтобы использовать газеттир как часть более сложного правила, оберните его в `RuleBuilder::from_arc`:

```rust
use renert::pipeline::morph_pipeline;
use renert::{pred, rule, Parser, RuleBuilder, RuleRegistry};
use renert::predicates::gram;

let position = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]));

let person_rule = rule([
    position,
    pred(gram("Name")),
    pred(gram("Surn")),
]);

let mut registry = RuleRegistry::new();
let root_id = registry.add(person_rule.build(()));
let parser = Parser::new(&registry, root_id);

let text = "выступление президента Владимира Путина";
for m in parser.findall(text) {
    let span = m.span();
    println!("{:?}", &text[span.start..span.stop]);
}
```

```text
"президента Владимира Путина"
```

## Газеттир с интерпретацией

### `normalized()` — ключ газеттира

Когда газеттир используется с `.interpretation(field.normalized())`, поле получает **канонический ключ словаря** — ту строку, которую вы передали при создании газеттира, — а не поверхностную форму токена из текста.

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleBuilder, RuleRegistry};

fact!(Geo => [name]);

let rule = RuleBuilder::from_arc(
    morph_pipeline(["красная площадь", "первомайская улица"])
)
.interpretation(Geo::name.normalized())
.interpretation_fact::<Geo>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(rule.build(()));
let parser = Parser::new(&registry, root_id);

let text = "c Красной площади на Первомайскую улицу";
for m in parser.findall(text) {
    if let Some(fact) = m.fact(&registry) {
        println!("{}", fact);
    }
}
```

```text
Geo(
    name='красная площадь'
)
Geo(
    name='первомайская улица'
)
```

Несмотря на то что в тексте стоят "Красной площади" (родительный падеж) и "Первомайскую улицу" (винительный), поле `name` содержит именно ключ из словаря.

### `inflected()` — приведение к нужной форме

Используйте `.inflected(forms)`, чтобы привести текстовую форму к нужному падежу/числу:

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::{Parser, RuleBuilder, RuleRegistry};

fact!(Position => [name]);

static FORMS: &[&str] = &["nomn", "sing"];

let rule = RuleBuilder::from_arc(
    morph_pipeline(["премьер министр", "президент"])
)
.interpretation(Position::name.inflected(FORMS))
.interpretation_fact::<Position>();

let mut registry = RuleRegistry::new();
let root_id = registry.add(rule.build(()));
let parser = Parser::new(&registry, root_id);

let text = "по приказу президента";
for m in parser.findall(text) {
    if let Some(fact) = m.fact(&registry) {
        println!("{}", fact);
    }
}
```

```text
Position(
    name='президент'
)
```

`inflected(["nomn", "sing"])` преобразует найденный токен в именительный падеж единственного числа.

> **Внимание:** `morph-rs` возвращает приведённые формы в нижнем регистре. Чтобы получить слово с заглавной буквы, используйте `.custom(|s| capitalize(s))`, где `capitalize` — ваша вспомогательная функция.

### `custom()` — произвольное преобразование значения

```rust
fn capitalize(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
```

```rust
// ...
.interpretation(Position::name.inflected(FORMS).custom(capitalize))
// ...
```

## Схемы (`*Scheme`) — отложенная активация

Схемы позволяют хранить список строк отдельно от токенизатора. Это удобно, когда словарь загружается из файла или базы данных, а активация происходит позже.

```rust
use renert::pipeline::pipeline_scheme;

let scheme = pipeline_scheme(["генеральный директор", "бухгалтер"]);

// Активация со стандартным токенизатором:
let pipeline = scheme.activate_default();
let rule = pipeline.into_rule();
```

Для морфологического пайплайна:

```rust
use renert::pipeline::morph_pipeline_scheme;

let scheme = morph_pipeline_scheme(["электронный дневник", "личный кабинет"]);
let pipeline = scheme.activate_default();
let rule = pipeline.into_rule();
```

Доступны три схемы:

| Схема | Активированный тип | Функция |
|---|---|---|
| `PipelineScheme` | `Pipeline` | `pipeline_scheme(...)` |
| `CaselessPipelineScheme` | `CaselessPipeline` | `caseless_pipeline_scheme(...)` |
| `MorphPipelineScheme` | `MorphPipeline` | `morph_pipeline_scheme(...)` |

Каждая схема имеет метод `into_rule()`, который активирует её немедленно и возвращает `Arc<Rule>` — эквивалент вызова фасадной функции.

## Расширенный API

### `productions()` — список продукций

После активации пайплайн хранит список `PipelineProduction`. Каждый элемент связывает исходный ключ словаря с продукцией правила:

```rust
use renert::pipeline::{Key, Pipeline};

let pipeline = Pipeline::new([
    Key::simple("генеральный директор", vec!["генеральный".into(), "директор".into()]),
    Key::simple("бухгалтер", vec!["бухгалтер".into()]),
]);

for prod in pipeline.productions() {
    println!("key: {:?}, terms: {}", prod.value, prod.terms.len());
}
```

```text
key: "генеральный директор", terms: 2
key: "бухгалтер", terms: 1
```

### `as_bnf()` — BNF-представление и `predict`

Метод `as_bnf()` возвращает BNF-слой пайплайна, который используется внутри Earley-парсера. Метод `predict(token)` возвращает продукции-кандидаты для заданного токена:

```rust
use renert::pipeline::pipeline_scheme;
use renert::token::Tokenizer;

let pipeline = pipeline_scheme(["бухгалтер", "генеральный директор"]).activate_default();
let bnf = pipeline.as_bnf();

let tokenizer = Tokenizer::new();
let tokens = tokenizer.tokenize("бухгалтер");
let candidates: Vec<_> = bnf.predict(&tokens[0]).collect();
println!("кандидатов: {}", candidates.len()); // 1
```

### `Key` — ключ словаря

`Key` — единица словаря: исходная строка `value` и список `terms`, где каждый элемент — набор вариантов для одной позиции. При морфологическом анализе каждая позиция получает несколько вариантов нормальных форм.

```rust
use renert::pipeline::Key;

// Ключ с одним вариантом на каждую позицию:
let key = Key::simple("красная площадь", vec!["красная".into(), "площадь".into()]);
assert_eq!(key.value, "красная площадь");
assert_eq!(key.terms.len(), 2);

// Ключ с несколькими вариантами на позицию (для ручной сборки):
let key = Key::new("текст", vec![
    vec!["текст".into(), "текста".into()],
]);
assert_eq!(key.terms[0].len(), 2);
```

## Сравнение трёх видов газеттиров

| | `pipeline` | `caseless_pipeline` | `morph_pipeline` |
|---|---|---|---|
| Регистр | строгий | игнорируется | игнорируется при нормализации |
| Морфология | нет | нет | да |
| Применение | технические термины, аббревиатуры | иностранные имена без морфологии | русские слова в любом падеже |
| Зависит от `morph-rs` | нет | нет | да |

## Полный пример: поиск должностей и извлечение фактов с помощью газеттиров

```rust
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::predicates::gram;
use renert::{pred, rule, Parser, RuleBuilder, RuleRegistry};

fn capitalize(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn main(){
    fact!(Person => [position, first, last]);

    static FORMS: &[&str] = &["nomn", "sing"];

    let position = RuleBuilder::from_arc(morph_pipeline(["премьер министр", "президент"]));

    let person_rule = rule([
        position.interpretation(Person::position.inflected(FORMS).custom(capitalize)),
        pred(gram("Name")).interpretation(Person::first.inflected(FORMS).custom(capitalize)),
        pred(gram("Surn")).interpretation(Person::last.inflected(FORMS).custom(capitalize)),
    ])
    .interpretation_fact::<Person>();

    let mut registry = RuleRegistry::new();
    let root_id = registry.add(person_rule.build(()));
    let parser = Parser::new(&registry, root_id);

    let text = "по приказу президента Владимира Путина";
    for m in parser.findall(text) {
        if let Some(fact) = m.fact(&registry) {
            println!("{}", fact);
        }
    }
}
```

```text
Person(
    position='Президент',
    first='Владимир',
    last='Путин'
)
```
