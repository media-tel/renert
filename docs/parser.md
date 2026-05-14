# Парсер

`Parser` — основной вход в разбор текста по правилам. Он берёт корневое правило из
`RuleRegistry`, токенизирует текст, строит Earley-чарт, превращает завершённые состояния
в деревья разбора и возвращает совпадения (`Match` / `MatchBorrowed`).

В прикладном коде чаще всего нужны методы:

- `find(text)` — первое совпадение в тексте;
- `findall(text)` — все непересекающиеся совпадения в тексте;
- `find_text(text)` / `findall_text(text)` — то же, но сразу только значения токенов;
- `r#match(text)` — проверить, что правило покрывает весь вход;
- `parse(tokens)` — разобрать уже готовый срез `Token`.

## Минимальный пример

Сначала собирается правило, затем оно добавляется в `RuleRegistry`, после чего создаётся
`Parser`.

```rust
use renert::{term, Parser, RuleRegistry};

let rule = (term("Russian") + term("Federation")).build(());

let mut registry = RuleRegistry::new();
let rule_id = registry.add(rule);

let parser = Parser::new(&registry, rule_id);

let found = parser.find_text("Russian Federation").unwrap();
assert_eq!(found, vec!["Russian".to_string(), "Federation".to_string()]);
```

`Parser::new(...)` использует глобальный `MorphTokenizer` и `PassTagger`. Это удобно для
обычного русского текста и морфологических предикатов (`gram`, `dictionary`, `normalized`).

## `find`: первое совпадение

`find` ищет первое подходящее совпадение где угодно в тексте.

```rust
use renert::{term, Parser, RuleRegistry};

let mut registry = RuleRegistry::new();
let rule_id = registry.add((term("Нижний") + term("Новгород")).build(()));
let parser = Parser::new(&registry, rule_id);

let m = parser.find("город Нижний Новгород").expect("match expected");

assert_eq!(m.matched_text(), "Нижний Новгород");
assert_eq!(m.token_range(), (1, 3));

let values: Vec<String> = m
    .tokens()
    .into_iter()
    .map(|t| t.value.into_owned())
    .collect();

assert_eq!(values, vec!["Нижний".to_string(), "Новгород".to_string()]);
```

Возвращаемый тип для строки — `Option<Match>`. `Match` владеет исходным текстом и
компактным представлением токенов, поэтому его можно хранить после выхода из функции.

## `findall`: все непересекающиеся совпадения

`findall` возвращает все найденные непересекающиеся совпадения. Если варианты пересекаются,
парсер разрешает конфликт по диапазонам.

```rust
use renert::predicates::{dictionary, gram};
use renert::{pred, Parser, RuleRegistry};

let rule = (pred(gram("ADJF")).optional().repeatable()
    + pred(dictionary(&["федерация", "республика"])))
    .build(());

let mut registry = RuleRegistry::new();
let rule_id = registry.add(rule);
let parser = Parser::new(&registry, rule_id);

let text = "Чеченская республика. Российская Федерация.";

for m in parser.findall(text) {
    println!("{} -> {:?}", m.matched_text(), m.token_range());
}
```
Output
```
Чеченская республика -> (0, 2)
Российская Федерация -> (3, 5)
```

Используйте `findall`, когда нужны диапазоны, дерево разбора, факты или сами токены.

## `find_text` и `findall_text`: только значения токенов

Если нужны только строки токенов, без дерева и span, используйте короткие методы:

```rust
use renert::{term, Parser, RuleRegistry};

let mut registry = RuleRegistry::new();
let rule_id = registry.add((term("a") + term("@") + term("mail")).build(()));
let parser = Parser::new(&registry, rule_id);

let one = parser.find_text("a @ mail");
assert_eq!(
    one,
    Some(vec!["a".to_string(), "@".to_string(), "mail".to_string()])
);

let all = parser.findall_text("a @ mail b @ mail");
assert_eq!(all.len(), 2);
```

Это тонкая обёртка над `find` / `findall`: внутри берётся `Match` и вызывается
`into_token_strings()`.

## `r#match`: разобрать весь вход

`match` — ключевое слово Rust, поэтому метод называется `r#match`.

`r#match(text)` возвращает `Some(Match)` только если корневое правило покрывает весь вход
от первого до последнего токена. Лишний хвост приводит к `None`.

```rust
use renert::{term, Parser, RuleRegistry};

let mut registry = RuleRegistry::new();
let rule_id = registry.add((term("Russian") + term("Federation")).build(()));
let parser = Parser::new(&registry, rule_id);

assert!(parser.r#match("Russian Federation").is_some());
assert!(parser.r#match("Russian Federation extra").is_none());
```

Используйте `r#match`, когда надо валидировать всю строку: например, поле формы,
короткую команду или отдельный фрагмент, который не должен содержать лишних токенов.

## `parse`: готовые токены

Если токены уже получены заранее, можно не токенизировать текст повторно:

```rust
use renert::token::Tokenizer;
use renert::{term, Parser, RuleRegistry};

let tokenizer = Tokenizer::new();
let tokens = tokenizer.tokenize("Russian Federation");

let mut registry = RuleRegistry::new();
let rule_id = registry.add((term("Russian") + term("Federation")).build(()));
let parser = Parser::new(&registry, rule_id);

let m = parser.parse(&tokens).expect("match expected");
assert_eq!(m.text("Russian Federation"), "Russian Federation");
```

Для готовых `Token` возвращается `MatchBorrowed`: он заимствует исходный срез токенов.
Его нельзя свободно хранить дольше, чем живёт `tokens`.

Важно: `parse(&[Token])` работает с plain-токенами. Если правило использует морфологические
предикаты (`gram`, `dictionary`), обычно проще передавать в парсер строку, чтобы
`Parser::new` использовал `MorphTokenizer`.

`find` и `findall` тоже полиморфны: им можно передавать и строку, и срез токенов.

```rust
let first = parser.find(&tokens);
let all = parser.findall(&tokens);
```

## Что лежит в `Match`

Для текстового входа `find` / `findall` возвращают `Match` (`MatchOwned`). Основные методы:

```rust
let m = parser.find("город Нижний Новгород").unwrap();

let span = m.span();              // Span совпадения в исходном тексте
let text = m.matched_text();      // подстрока совпадения
let token_range = m.token_range(); // диапазон токенов (start, stop)
let tokens = m.tokens();          // листовые токены дерева разбора
let tree = m.tree();              // дерево разбора
```

Формат вывода таких данных:
```rust
use renert::{term, Parser, RuleRegistry};

fn main() {
    let mut registry = RuleRegistry::new();

    let rule_id = registry.add(
        (term("Нижний") + term("Новгород")).build(())
    );

    let parser = Parser::new(&registry, rule_id);

    let m = parser
        .find("город Нижний Новгород")
        .expect("match expected");

    let span = m.span();
    let text = m.matched_text();
    let token_range = m.token_range();
    let tokens = m.tokens();
    let tree = m.tree();

    println!("span: {:?}", span);
    println!("matched_text: {:?}", text);
    println!("token_range: {:?}", token_range);
    println!("tokens: {:#?}", tokens);
    println!("tree: {:#?}", tree);
}
```
Output
```
span: Span { start: 11, stop: 40 }
matched_text: "Нижний Новгород"
token_range: (1, 3)
tokens: [
    Token {
        value: "Нижний",
        span: Span {
            start: 11,
            stop: 23,
        },
        token_type: Russian,
    },
    Token {
        value: "Новгород",
        span: Span {
            start: 24,
            stop: 40,
        },
        token_type: Russian,
    },
]
tree: Tree {
    root: Node {
        rule_id: RuleId(
            0,
        ),
        production_index: 0,
        rank: 0,
        children: [
            Leaf(
                Leaf {
                    predicate_id: 0,
                    token_index: 1,
                    span: Span {
                        start: 11,
                        stop: 23,
                    },
                    matched_forms: None,
                },
            ),
            Leaf(
                Leaf {
                    predicate_id: 0,
                    token_index: 2,
                    span: Span {
                        start: 24,
                        stop: 40,
                    },
                    matched_forms: None,
                },
            ),
        ],
    },
    range: (
        1,
        3,
    ),
}
```

Для готовых токенов (`parse`, `find(&tokens)`, `findall(&tokens)`) возвращается
`MatchBorrowed`. У него похожие методы, но текст нужно передавать явно:

```rust
let m = parser.parse(&tokens).unwrap();
let text = m.text("Russian Federation");
let tokens = m.tokens();
```

## Низкоуровневые методы

Обычно они нужны для отладки, профилирования или разработки самого парсера.

### `chart(text, all)`

Строит Earley-чарт.

```rust
let chart = parser.chart("Russian Federation", false);
println!("columns: {}", chart.len());
```

Параметр `all`:

- `false` — стартовать только с начала текста;
- `true` — стартовать из каждой позиции, как при поиске по тексту.

`Chart` содержит `N + 1` колонок для `N` токенов. Колонка `0` — позиция до первого токена,
колонка `i` — позиция после токена `i - 1`.

### `matches(text, all)`

Возвращает финальные `State` для корневого правила.

```rust
let states = parser.matches("Russian Federation", false);
for state in states {
    println!("{:?}", state.range());
}
```

Это ещё не `Match`: состояния потом превращаются в деревья и нормализуются.

### `extract(text, all)`

Полный pipeline, но с явным флагом `all`.

```rust
let matches_from_start = parser.extract("Russian Federation", false);
let matches_from_anywhere = parser.extract("text Russian Federation", true);
```

Пример с выводом:

```rust
use renert::{term, Parser, RuleRegistry};

let mut registry = RuleRegistry::new();
let rule_id = registry.add((term("Russian") + term("Federation")).build(()));
let parser = Parser::new(&registry, rule_id);

let only_from_start = parser.extract("text Russian Federation", false);
let from_anywhere = parser.extract("text Russian Federation", true);

println!("all=false, matches={}", only_from_start.len());
for m in &only_from_start {
    println!("  {:?} {:?}", m.matched_text(), m.token_range());
}

println!("all=true, matches={}", from_anywhere.len());
for m in &from_anywhere {
    println!("  {:?} {:?}", m.matched_text(), m.token_range());
}
```
Output:
```text
all=false, matches=0
all=true, matches=1
  "Russian Federation" (1, 3)
```

- `all = false` — искать разбор, который заканчивается в последней колонке;
- `all = true` — искать совпадения с любой стартовой позиции.

Для обычного прикладного поиска чаще используйте `find` / `findall`.

## Кастомный токенизатор и тэггер

`Parser::new` использует глобальный `MorphTokenizer`. Если нужна другая токенизация,
используйте `with_tokenizer`.

```rust
use std::sync::Arc;
use renert::token::Tokenizer;
use renert::{term, Parser, RuleRegistry};

let mut registry = RuleRegistry::new();
let rule_id = registry.add(term("hello").build(()));

let tokenizer = Arc::new(Tokenizer::new());
let parser = Parser::with_tokenizer(&registry, rule_id, tokenizer);
```

Такой парсер получает plain-токены, без морфологических форм. Поэтому предикаты вроде
`gram("NOUN")` для него не подходят.

Если нужно модифицировать токены после токенизации (например, добавить теги), используйте
`with_tokenizer_and_tagger`. Тэггер реализует trait `Tagger`:

```rust
use renert::parser::PassTagger;
use renert::token::Tokenizer;
use renert::{term, Parser, RuleRegistry};
use std::sync::Arc;

let mut registry = RuleRegistry::new();
let rule_id = registry.add(term("hello").build(()));
let parser = Parser::with_tokenizer_and_tagger(
    &registry,
    rule_id,
    Arc::new(Tokenizer::new()),
    Arc::new(PassTagger),
);
```

`PassTagger` — тэггер по умолчанию, он ничего не меняет.

## Как выбрать метод

| Задача | Метод |
| ------ | ----- |
| Найти первое совпадение в тексте | `find(text)` |
| Найти все непересекающиеся совпадения | `findall(text)` |
| Получить только строки токенов первого матча | `find_text(text)` |
| Получить только строки токенов всех матчей | `findall_text(text)` |
| Проверить, что правило покрывает весь вход | `r#match(text)` |
| Разобрать готовые `Token` | `parse(&tokens)` или `find(&tokens)` |
| Посмотреть дерево, span, tokens | `Match` / `MatchBorrowed` |
| Извлечь факт | `match.fact(&registry)` / `match.facts(&registry)` |
| Отладить Earley-чарт | `chart(text, all)` |
| Получить финальные состояния | `matches(text, all)` |
| Запустить полный pipeline вручную | `extract(text, all)` |

## Типичный рабочий шаблон

```rust
use renert::predicates::{and, dictionary, gram, is_title};
use renert::{pred, Parser, RuleRegistry};

let text = r#"
                В Чеченской республике на день рождения ...
                Донецкая народная республика провозгласила ...
                Башня Федерация — одна из самых высоких ...
                "#;

// 1. Собираем правило.
let geo_rule = (
   pred(and(vec![
         gram("ADJF"),
         is_title(), // аналог is_capitalized()
     ]))
     + pred(gram("ADJF")).optional().repeatable()
     + pred(dictionary(&["федерация", "республика"]))
).build(());

// 2. Регистрируем правило.
let mut registry = RuleRegistry::new();
let geo_id = registry.add(geo_rule);

// 3. Создаём парсер.
let parser = Parser::new(&registry, geo_id);

// 4. Ищем совпадения.
for m in parser.findall(text) {
     let tokens = m.tokens();
     let values: Vec<&str> = tokens.iter().map(|t| t.value.as_ref()).collect();
     println!("{:?}", values);
}
// Сразу Vec<String> на каждый матч:
for values in parser.findall_text(text) {
     println!("{:?}", values);
}
```
Output:
```text
["Чеченской", "республике"]
["Донецкая", "народная", "республика"]
```

Если правило сложное, сначала проверьте его BNF:

```rust
println!("{}", my_rule.normalized().as_bnf());
```

Если совпадений нет, проверьте по шагам:

1. Как текст токенизируется (`Tokenizer` / `MorphTokenizer`).
2. Какие предикаты проходят на нужных токенах.
3. Как выглядит BNF правила.
4. Используете ли вы `find`/`findall` или `r#match` по смыслу задачи.