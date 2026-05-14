# Построение правил

Правило (`Rule`) — это узел контекстно-свободной грамматики. С его помощью описывают, какие
последовательности токенов считать совпадением. В `renert` правила собираются цепочкой
комбинаторов, после чего нормализуются и регистрируются в `RuleRegistry`, который потом
подаётся в `Parser`.

Этот туториал устроен так, чтобы после прочтения вы могли самостоятельно собирать
большие грамматики: от одного терминала до рекурсивных правил.

## Содержание

1. [Базовые понятия](#базовые-понятия)
2. [Терминалы: `term` и `pred`](#терминалы-term-и-pred)
3. [Конкатенация: `rule([...])` и `+`](#конкатенация-rule-и-)
4. [Альтернатива: `or_([...])` и `|`](#альтернатива-or_-и-)
5. [Именование: `.named("...")`](#именование-named)
6. [Квантификаторы: `.optional()` и `.repeatable()`](#квантификаторы-optional-и-repeatable)
7. [Рекурсия: `forward()` и `.define(...)`](#рекурсия-forward-и-define)
8. [Главный терм: `main_term`](#главный-терм-main_term)
9. [Нормализация и BNF: `.normalized()` и `.as_bnf()`](#нормализация-и-bnf-normalized-и-as_bnf)
10. [Запуск: `RuleRegistry` + `Parser`](#запуск-ruleregistry--parser)
11. [Большой пример: адрес](#большой-пример-адрес)
12. [Шпаргалка](#шпаргалка)

## Базовые понятия

- **Терминал** — проверка одного токена (через предикат, например `eq("дом")`).
- **Правило** — последовательность терминалов и/или вложенных правил.
- **Альтернатива** — выбор из нескольких правил.
- **Грамматика** — набор связанных правил. Корневое правило задаётся при создании парсера.

В `renert` все правила собираются через `RuleBuilder`. Главные импорты:

```rust
use renert::{empty, forward, main_term, or_, pred, rule, term};
use renert::predicates::{and, caseless, dictionary, eq, gram, in_, is_title, not, or};
```

Готовое правило — это `Arc<Rule<'a>>` (получается через `.build(())` или `.into_arc()`),
и его передают в `RuleRegistry`.

## Терминалы: `term` и `pred`

Самый простой терминал — `term("...")`, эквивалент `pred(eq("..."))`.

```rust
use renert::term;

let dom = term("дом"); // совпадает с токеном, у которого value == "дом"
```

Если нужна более сложная проверка — используйте `pred(...)` с любым предикатом
(см. [`docs/predicates.md`](predicates.md)):

```rust
use renert::pred;
use renert::predicates::{caseless, gram, in_, is_title};

let city_kind = pred(in_(&["город", "село", "деревня"]));
let proper_noun = pred(is_title());
let any_noun = pred(gram("NOUN"));
let case_insensitive = pred(caseless("Москва"));
```

Особое «пустое» правило — `empty()` (epsilon, ничего не потребляет):

```rust
use renert::empty;

let empty = empty(); // обычно используется внутри or_([...]) или как пустая ветка
```

## Конкатенация: `rule([...])` и `+`

Конкатенация — это последовательность термов «один за другим». Есть два способа:

```rust
use renert::{rule, term};

// Через rule(...)
let r1 = rule([term("ул"), term("."), term("Ленина")]);

// Через оператор +
let r2 = term("ул") + term(".") + term("Ленина");
```

Оба варианта собирают правило `'ул' '.' 'Ленина'`. `rule([...])` удобнее для длинных
последовательностей, `+` — для коротких inline-выражений.

`rule([])` (пустой список) даёт epsilon-правило — то же, что `empty()`.

## Альтернатива: `or_([...])` и `|`

Альтернатива — выбор одной из нескольких ветвей.

```rust
use renert::{or_, term};

// Через or_(...)
let key = or_([term("р"), term("размер"), term("size")]);

// Через оператор |
let key = term("р") | term("размер") | term("size");
```

Обычно `or_` удобнее для трёх и более вариантов; `|` хорошо смотрится для двух.

`or_([])` (пустой список) даёт epsilon. `or_([single])` возвращает `single` без обёртки.

## Именование: `.named("...")`

Именование — единственный способ дать правилу читаемое имя в BNF и при отладке.
Без имени узлы автоматически называются `R0`, `R1`, ...

```rust
use renert::{or_, rule, term};

let key = or_([
    rule([term("р"), term(".")]),
    rule([term("размер")]),
])
.named("KEY");

let value = or_([term("S"), term("M"), term("L")]).named("VALUE");

let size = rule([key, value]).named("SIZE");

println!("{}", size.normalized().as_bnf());
```

Output:
```text
SIZE -> KEY VALUE
KEY -> 'р' '.' | 'размер'
VALUE -> 'S' | 'M' | 'L'
```

Имя — это просто метка. На семантику разбора оно не влияет, но сильно помогает при чтении
BNF и сообщениях об ошибках.

## Квантификаторы: `.optional()` и `.repeatable()`

Квантификаторы вешаются на любое правило/терминал.

### `.optional()`

«0 или 1 вхождение».

```rust
use renert::{rule, term};

let dot = term(".").optional();
let abbr = rule([term("ул"), dot]); // "ул" или "ул."
```

Вариант `.optional_with(reverse)` управляет порядком альтернатив в результирующем BNF.
Это важно только для приоритета разборов в неоднозначных грамматиках.

### `.repeatable()`

«1 или больше вхождений», без верхней границы.

```rust
use renert::{pred, term};
use renert::predicates::is_word;

let words = pred(is_word()).repeatable(); // одно или больше слов подряд
```

### `.repeatable_with(min, max, reverse)`

Полный контроль над числом повторов:

| min          | max          | смысл                       |
| ------------ | ------------ | --------------------------- |
| `None`       | `None`       | без границ (= `repeatable`) |
| `Some(min)`  | `None`       | не меньше `min`             |
| `None`       | `Some(max)`  | не больше `max`             |
| `Some(min)`  | `Some(max)`  | диапазон `[min, max]`       |

Ограничения: `min >= 1`, `max >= 1`, `max >= min`. Иначе будет паника.

```rust
use renert::term;

let between_2_and_3 = term("a").repeatable_with(Some(2), Some(3), false).named("A23");
println!("{}", between_2_and_3.normalized().as_bnf());
```

Output:
```text
A23 -> 'a' R0
R0 -> 'a' 'a' | 'a'
```

### Комбинации

Квантификаторы можно вешать друг на друга. Например, `optional` + `repeatable` = «ноль или
больше повторов»:

```rust
use renert::{pred, term};
use renert::predicates::is_word;

let zero_or_more_words = pred(is_word()).repeatable().optional();
```

Параметр `reverse` для `optional`/`repeatable_with` влияет только на порядок альтернатив
в нормализованной BNF (важно для приоритетов в неоднозначных грамматиках).

## Рекурсия: `forward()` и `.define(...)`

Рекурсивные правила нужны, когда правило ссылается на самого себя (или на цикл из
нескольких правил). Прямо в `let r = ... r ...` это сделать нельзя — Rust не позволит.
Поэтому используется паттерн «forward declaration»:

1. Создаём заглушку: `let r = forward();`
2. Описываем правило, использующее `r.clone()` внутри.
3. Привязываем тело: `r.define(...)`.

Классический пример — арифметическое выражение `EXPR -> a | ( EXPR + EXPR )`:

```rust
use renert::{forward, or_, rule, term};

let expr = forward();
expr.define(
    or_([
        rule([term("a")]),
        rule([term("("), expr.clone(), term("+"), expr.clone(), term(")")]),
    ])
    .named("EXPR"),
);

println!("{}", expr.normalized().as_bnf());
```

Output:
```text
EXPR -> 'a' | R0 EXPR ')'
R0 -> '(' EXPR '+'
```

Ещё пример — текст в кавычках («любое количество слов между « и »»):

```rust
use renert::{eq, forward, not, or_, pred, rule, term};

let word = pred(not(eq("»")));
let text = forward();
text.define(or_([
    rule([word.clone()]),
    rule([word.clone(), text.clone()]),
]));

let title = rule([term("«"), text, term("»")]).named("TITLE");

println!("{}", title.normalized().as_bnf());
```

Output:
```text
TITLE -> '«' R0 '»'
R0 -> not('»') | not('»') R0
```

Тот же эффект — короче, через `.repeatable()` (внутренний forward создаётся автоматически):

```rust
use renert::{eq, not, pred, rule, term};

let title = rule([
    term("«"),
    pred(not(eq("»"))).repeatable(),
    term("»"),
])
.named("TITLE");
```

> Совет: используйте явный `forward()` только когда без него никак (взаимная рекурсия
> двух правил, нестандартный порядок альтернатив). Для всех «повторов» хватает
> `.repeatable()` / `.repeatable_with(...)`.

Ограничения `forward`:
- в прикладном коде обычно определяют один раз; технически повторный `define` сейчас
  перезапишет цель;
- нельзя `forward(forward(...))` — паника;
- если `forward` не определён до использования в `RuleRegistry`, валидация выдаст ошибку.

## Главный терм: `main_term`

В последовательности термов один может быть помечен как «главный». Это влияет на
согласование морфологических форм между токенами (см. модуль `relations`) и на
интерпретацию фактов.

```rust
use renert::{main_term, pred, rule, term};
use renert::predicates::gram;

let np = rule([
    pred(gram("ADJF")),                  // прилагательное
    main_term(pred(gram("NOUN"))),       // главное — существительное
]);
```

`main_term` не меняет порядок термов. Он сохраняет индекс главного элемента в
`Production::main`; это используется при согласовании морфологических форм и обходе
дерева разбора. Обычный вывод через `.normalized().as_bnf()` этот индекс не показывает.

Если `main_term` не указан, главным считается первый терм продукции.

## Нормализация и BNF: `.normalized()` и `.as_bnf()`

После сборки правил их нужно **нормализовать**: расширенные узлы (`optional`,
`repeatable`, `or` и т.п.) разворачиваются в эквивалентные базовые продукции.

```rust
let normalized = my_rule.normalized();      // Arc<Rule<'a>>
println!("{}", normalized.as_bnf());        // многострочный BNF
```

Что делает `normalized()` (последовательно):

1. Сжимает вложенные расширенные конструкции (`SquashExtended`).
2. Заменяет расширенные узлы базовыми (`ReplaceExtended`).
3. Превращает `Or` в плоские `Base` с N продукциями (`ReplaceOr`).
4. Удаляет `Empty` там, где можно (`ReplaceEmpty`).
5. Сплющивает одиночные обёртки (`Flatten`).

После этого граф готов к выводу BNF, к регистрации в `RuleRegistry` и к парсеру.

> Удобный шорткат: `RuleRegistry::add(rule)` сам вызывает `.normalized()` внутри,
> поэтому вручную нормализовывать перед `add(...)` не обязательно. Но для отладки
> через `.as_bnf()` нормализация нужна явно.

## Запуск: `RuleRegistry` + `Parser`

После сборки правил их регистрируют в реестре, а реестр передают в `Parser`:

```rust
use renert::predicates::{dictionary, gram, is_title};
use renert::{and, pred, Parser, RuleRegistry};

let geo = ((pred(and(vec![
    gram("ADJF"),
    is_title(), // аналог is_capitalized()
])) + pred(gram("ADJF")).optional().repeatable()
    + pred(dictionary(&["федерация", "республика"])))
.build(()));

let mut registry = RuleRegistry::new();
let geo_id = registry.add(geo);
let parser = Parser::new(&registry, geo_id);

let text = "В Чеченской республике сегодня солнечно";

for values in parser.findall_text(text) {
    println!("{:?}", values);
}
```

Что делает `Parser`:

- `find(text)` — первое совпадение;
- `findall(text)` — все непересекающиеся совпадения;
- `r#match(text)` — успешен, только если правило покрывает всю строку;
- `find_text` / `findall_text` — короткие варианты, возвращающие сразу `Vec<String>`.

Подробнее об API парсера — в rustdoc модуля `parser`.

## Большой пример: адрес

Соберём правило для адреса вида `ул. Ленина, д. 10` с альтернативами и опциональными
частями.

```rust
use renert::{main_term, or_, pred, rule, term, Parser, RuleRegistry};
use renert::predicates::{is_digit, is_title};

fn main() {
    // 1. Тип улицы: «ул», «улица», «пр», «проспект» — с опциональной точкой.
    let street_kind = rule([
        or_([
            term("ул"), term("улица"),
            term("пр"), term("проспект"),
            term("пер"), term("переулок"),
        ]),
        term(".").optional(),
    ])
    .named("STREET_KIND");

// 2. Имя улицы: одно или больше слов с заглавной буквы (имена, фамилии, …).
    let street_name = pred(is_title()).repeatable().named("STREET_NAME");

    // 3. Дом: «д.» (необязательно) + число.
    let house = rule([
        rule([term("д"), term(".").optional()]).optional(),
        main_term(pred(is_digit())),
    ])
    .named("HOUSE");

    // 4. Адрес целиком, разделённый запятой (необязательной).
    let address = rule([
        street_kind,
        street_name,
        term(",").optional(),
        house,
    ])
    .named("ADDRESS");

    println!("{}", address.clone().normalized().as_bnf());

    let mut registry = RuleRegistry::new();
    let id = registry.add(address.build(()));
    let parser = Parser::new(&registry, id);

    let text = "Адрес: ул. Ленина, д. 10. Доставка до двери.";

    for m in parser.findall(text) {
        let tokens = m.tokens();
        let values: Vec<&str> = tokens.iter().map(|t| t.value.as_ref()).collect();
        println!("\n{:?}", values);
    }
}
```
Output
```
ADDRESS -> R0 HOUSE
R0 -> R1 R2
HOUSE -> R3 is_digit()
R1 -> STREET_KIND STREET_NAME
R2 -> e | ','
R3 -> e | 'д' R4
STREET_KIND -> R5 R6
STREET_NAME -> R7
R4 -> e | '.'
R5 -> 'ул' | 'улица' | 'пр' | 'проспект' | 'пер' | 'переулок'
R6 -> e | '.'
R7 -> is_title() R7 | is_title()

["ул", ".", "Ленина", ",", "д", ".", "10"]
```

Что важного в примере:
- каждое подправило именовано (`STREET_KIND`, `STREET_NAME`, `HOUSE`, `ADDRESS`) —
  легко читать BNF и логи;
- `term(".").optional()` обрабатывает и «ул», и «ул.»;
- `pred(is_title()).repeatable()` ловит одно или несколько слов с заглавной буквы,
  например «Ленина» или «Большая Дмитровка»;
- `main_term(pred(is_digit()))` помечает номер дома как главный — пригодится при
  построении факта (`Address { house: i64, ... }`);
- `term(",").optional()` делает запятую необязательной.

## Шпаргалка

| Хочу...                                        | Использую                                                  |
| ---------------------------------------------- | ---------------------------------------------------------- |
| Совпасть с конкретным словом                   | `term("...")`                                              |
| Произвольный предикат                          | `pred(...)`                                                |
| Последовательность                             | `rule([a, b, c])` или `a + b + c`                          |
| Одну из нескольких альтернатив                 | `or_([a, b, c])` или `a | b | c`                           |
| Имя для BNF/отладки                            | `.named("FOO")`                                            |
| 0 или 1 вхождение                              | `.optional()`                                              |
| 1+ вхождений                                   | `.repeatable()`                                            |
| Точные границы повторов                        | `.repeatable_with(Some(min), Some(max), reverse)`          |
| Не меньше `min`                                | `.repeatable_with(Some(min), None, reverse)`               |
| Не больше `max`                                | `.repeatable_with(None, Some(max), reverse)`               |
| Рекурсивная ссылка                             | `let r = forward(); r.define(...)`                         |
| Главный терм (для согласования / интерпретаций) | `main_term(...)`                                          |
| Получить BNF представление                                  | `rule.normalized().as_bnf()`                               |
| Запустить парсер                               | `RuleRegistry::new() + .add(rule) + Parser::new(...)`      |

### Типичные ошибки и подсказки

- **«Forward не определён»** при валидации — забыли вызвать `r.define(...)`.
- **Паника `>1 main`** — в продукции более одного `main_term`, оставьте только один.
- **Паника `min == 0` / `max < min`** — границы у `repeatable_with` некорректны.
- **`as_bnf` без нормализации** показывает «сырой» граф; всегда вызывайте
  `.normalized().as_bnf()` для отладки.
- **Большое правило неудобно читать** — разбейте на именованные подправила и собирайте
  итоговое из них, как в примере с адресом.