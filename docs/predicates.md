# Предикаты

Предикат принимает токен, возвращает True или False. В RENERT встроен набор готовых предикатов. Операторы and, or и not комбинируют предикаты:
```rust
use renert::error;
use renert::predicates::{eq, is_title, and, not};
use renert::token::MorphTokenizer;

fn main() -> error::Result<()> {
    let tokenizer = MorphTokenizer::open()?;
    let token = tokenizer.tokenize("Стали").into_iter().next().unwrap();

    // is_capitalized() в yargy ~= is_title() в renert
    let predicate = is_title();
    println!("is_title: {:?}", predicate.check(&token));
    assert!(predicate.check(&token));

    let predicate = and(vec![
        is_title(),
        not(eq("марки")),
    ]);
    println!("and: {:?}", predicate.check(&token));
    assert!(predicate.check(&token));

    Ok(())
}
```
Output
```bash
is_title: true
and: true
```

Ниже представен полный список готовых предикатов библиотеки RENERT.

## Предикаты сравнения

### `eq`

**Что проверяет:** точное совпадение `token.value` со строкой `value`.  
**Сигнатура:** `eq(value: &str) -> PredicateKind`

```rust
use renert::predicates::eq;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("1").into_iter().next().unwrap();
assert!(eq("1").check(&token));
```

**Примечание:** регистрозависимое сравнение.

### `eq_owned`

**Что проверяет:** то же, что `eq`, но принимает `String`.  
**Сигнатура:** `eq_owned(value: String) -> PredicateKind`

```rust
use renert::predicates::eq_owned;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("1").into_iter().next().unwrap();
assert!(eq_owned(String::from("1")).check(&token));
```

**Примечание:** удобно, когда значение уже хранится в `String`.

### `caseless`

**Что проверяет:** совпадение без учета регистра.  
**Сигнатура:** `caseless(value: &str) -> PredicateKind`

```rust
use renert::predicates::caseless;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("РАНО").into_iter().next().unwrap();
assert!(caseless("Рано").check(&token));
```

**Примечание:** обе строки сравниваются в lowercase.

### `caseless_owned`

**Что проверяет:** то же, что `caseless`, но принимает `String`.  
**Сигнатура:** `caseless_owned(value: String) -> PredicateKind`

```rust
use renert::predicates::caseless_owned;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("РАНО").into_iter().next().unwrap();
assert!(caseless_owned(String::from("Рано")).check(&token));
```

**Примечание:** удобно для динамически собранных строк.

### `in_`

**Что проверяет:** вхождение `token.value` в набор значений.  
**Сигнатура:** `in_(values: &[&str]) -> PredicateKind`

```rust
use renert::predicates::in_;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("M").into_iter().next().unwrap();
assert!(in_(&["S", "M", "L"]).check(&token));
```

**Примечание:** регистрозависимое сравнение.

### `in_caseless`

**Что проверяет:** вхождение в набор значений без учета регистра.  
**Сигнатура:** `in_caseless(values: &[&str]) -> PredicateKind`

```rust
use renert::predicates::in_caseless;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("МОСКВА")
    .into_iter()
    .next()
    .unwrap();
assert!(in_caseless(&["москва", "питер"]).check(&token));
```

**Примечание:** значения словаря приводятся к lower-case.

## Морфологические предикаты

### `normalized`

**Что проверяет:** совпадает ли нормальная форма токена с заданной леммой.  
**Сигнатура:** `normalized(value: &str) -> PredicateKind`

```rust
use renert::predicates::normalized;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("улицей")
    .into_iter()
    .next()
    .unwrap();
assert!(normalized("улица").check(&token));
```

**Примечание:** для plain-токенов используется fallback к `lowercase`.

### `dictionary`

**Что проверяет:** входит ли лемма токена в словарь лемм.  
**Сигнатура:** `dictionary(values: &[&str]) -> PredicateKind`

```rust
use renert::predicates::dictionary;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("улицей")
    .into_iter()
    .next()
    .unwrap();
assert!(dictionary(&["улица"]).check(&token));
```

**Примечание:** словарь расширяется всеми нормализованными формами входных слов.

### `gram`

**Что проверяет:** есть ли у токена морфологический разбор с указанной граммемой.  
**Сигнатура:** `gram(g: &str) -> PredicateKind`

```rust
use renert::predicates::gram;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("стали")
    .into_iter()
    .next()
    .unwrap();
assert!(gram("VERB").check(&token));
```

**Примечание:** для plain-токенов без форм обычно возвращает `false`.

## Предикаты по форме токена

### `is_title`

**Что проверяет:** токен в формате Title Case.  
**Сигнатура:** `is_title() -> PredicateKind`

```rust
use renert::predicates::is_title;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("Москва")
    .into_iter()
    .next()
    .unwrap();
assert!(is_title().check(&token));
```

**Примечание:** аналог `is_capitalized()` из Python yargy.

### `is_upper`

**Что проверяет:** все буквы в верхнем регистре.  
**Сигнатура:** `is_upper() -> PredicateKind`

```rust
use renert::predicates::is_upper;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("ТВЕРЬ")
    .into_iter()
    .next()
    .unwrap();
assert!(is_upper().check(&token));
```

**Примечание:** небуквенные символы влияют по правилам реализации `is_upper`.

### `is_lower`

**Что проверяет:** все буквы в нижнем регистре.  
**Сигнатура:** `is_lower() -> PredicateKind`

```rust
use renert::predicates::is_lower;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("тверь")
    .into_iter()
    .next()
    .unwrap();
assert!(is_lower().check(&token));
```

**Примечание:** полезно для фильтрации слов в "обычном" написании.

### `is_digit`

**Что проверяет:** токен состоит только из ASCII-цифр.  
**Сигнатура:** `is_digit() -> PredicateKind`

```rust
use renert::predicates::is_digit;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("12345")
    .into_iter()
    .next()
    .unwrap();
assert!(is_digit().check(&token));
```

**Примечание:** это именно ASCII-цифры, не все Unicode-цифры.

### `is_word`

**Что проверяет:** токен состоит из буквенных символов.  
**Сигнатура:** `is_word() -> PredicateKind`

```rust
use renert::predicates::is_word;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("улица")
    .into_iter()
    .next()
    .unwrap();
assert!(is_word().check(&token));
```

**Примечание:** цифры и знаки пунктуации не проходят.

### `is_non_alnum`

**Что проверяет:** токен содержит хотя бы один не-alphanumeric символ.  
**Сигнатура:** `is_non_alnum() -> PredicateKind`

```rust
use renert::predicates::is_non_alnum;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("*").into_iter().next().unwrap();
assert!(is_non_alnum().check(&token));
```

**Примечание:** удобно для фильтрации пунктуации и служебных символов.

### `length_eq`

**Что проверяет:** длина `token.value` равна `n`.  
**Сигнатура:** `length_eq(n: usize) -> PredicateKind`

```rust
use renert::predicates::length_eq;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("дом").into_iter().next().unwrap();
assert!(length_eq(6).check(&token));
```

**Примечание:** длина считается в байтах UTF-8 (`"дом"` = 6 байт).

### `is_token_type`

**Что проверяет:** тип токена (`Int`, `Russian`, `Punct` и т.д.).  
**Сигнатура:** `is_token_type(type_name: &str) -> PredicateKind`

```rust
use renert::predicates::is_token_type;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("42").into_iter().next().unwrap();
assert!(is_token_type("Int").check(&token));
```

**Примечание:** неизвестное имя типа маппится в `TokenType::Other`.

### `gte`

**Что проверяет:** числовое значение токена `>= n`.  
**Сигнатура:** `gte(value: i64) -> PredicateKind`

```rust
use renert::predicates::gte;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("15").into_iter().next().unwrap();
assert!(gte(10).check(&token));
```

**Примечание:** применяйте к числовым токенам (`TokenType::Int`).

### `lte`

**Что проверяет:** числовое значение токена `<= n`.  
**Сигнатура:** `lte(value: i64) -> PredicateKind`

```rust
use renert::predicates::lte;
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("7").into_iter().next().unwrap();
assert!(lte(10).check(&token));
```

**Примечание:** обычно используется вместе с `gte` для диапазонов.

## Логические композиции

### `and`

**Что проверяет:** все предикаты из списка должны вернуть `true`.  
**Сигнатура:** `and(preds: Vec<PredicateKind>) -> PredicateKind`

```rust
use renert::predicates::{and, eq, is_title, not};
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer()
    .tokenize("Стали")
    .into_iter()
    .next()
    .unwrap();
let p = and(vec![is_title(), not(eq("марки"))]);
assert!(p.check(&token));
```

**Примечание:** порядок предикатов обычно не важен.

### `or`

**Что проверяет:** хотя бы один предикат из списка должен вернуть `true`.  
**Сигнатура:** `or(preds: Vec<PredicateKind>) -> PredicateKind`

```rust
use renert::predicates::{eq, or};
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("дом").into_iter().next().unwrap();
let p = or(vec![eq("улица"), eq("дом")]);
assert!(p.check(&token));
```

**Примечание:** удобен для альтернативных условий.

### `not`

**Что проверяет:** инвертирует результат вложенного предиката.  
**Сигнатура:** `not(pred: PredicateKind) -> PredicateKind`

```rust
use renert::predicates::{eq, not};
use renert::token::global_morph_tokenizer;

let token = global_morph_tokenizer().tokenize("дом").into_iter().next().unwrap();
assert!(not(eq("улица")).check(&token)); // not(False) -> True
```

**Примечание:** применяется к одному предикату.
