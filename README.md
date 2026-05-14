# RENERT

RENERT (Rust Extractor of Named Entities from Russian Texts) — Rust-библиотека для извлечения структурированной информации из русскоязычных текстов.

## Требования

- Rust **edition 2021** (см. `Cargo.toml`).
- Файл словаря OpenCorpora в формате **XML** (`dict.opcorpora.xml`) — для первичной сборки или для утилиты `add_dictionary`.
- Перед использованием `Parser`, `morph_pipeline` / `caseless_pipeline` / `pipeline` и глобального морфо-токенизатора один раз вызовите `renert::init` или `renert::load` (см. ниже; подробности в rustdoc крейта).

Морфология и бинарный словарь совместимы с OpenCorpora, представленный на [сайте](https://opencorpora.org/dict.php).

## Установка

Добавьте зависимость в `Cargo.toml` своего проекта:

```toml
[dependencies]
renert = "0.1.0"
```

Прямая зависимость крейта объявляет **`morph-rs`** (и прочие crates из `Cargo.toml`).

## Подготовка словаря

В каталоге кэша должны появиться **`dict.json`** и **`dict.fst`** (формат morph-rs).

1. **Из кода** — `renert::init(path_to_xml, cache_dir)`: при отсутствии файлов в `cache_dir` выполняется сборка из XML (первый запуск может занять 5-10 минут). Последующие запуски будут пропускать этот шаг.
2. **Только открыть готовый кэш** — `renert::load(cache_dir)`, если оба файла уже есть.
3. **Утилита в этом репозитории** — собрать словарь без своего `main`:

   ```bash
   cargo run --bin add_dictionary -- path/to/dict.opcorpora.xml --out data/dict
   ```

   Если `--out` не указан, по умолчанию используется каталог `data/dict`.

Переменная окружения **`YARGY_DICT_DIR`** и пути по умолчанию описаны в rustdoc модуля `morph`; надёжный вариант для приложений — явный `init` / `load` при старте.

## Пример

```rust
use renert::error::Result;
use renert::fact;
use renert::interpretation::RuleInterpretation;
use renert::pipeline::morph_pipeline;
use renert::predicates::gram;
use renert::relations::{gnc_relation, RuleBuilderRelationExt};
use renert::{and, not, pred, rule, Parser, RuleBuilder, RuleRegistry};

fn main() -> Result<()> {
    renert::init("dict.opcorpora.xml", "data/dict")?;
    //или
    //renert::load("path/to/dict");

    fact!(Person => [position, name]);
    fact!(Name => [first, last]);

    static FORMS: &[&str] = &["nomn", "sing"];
    let gnc = gnc_relation();

    let name = rule([
        pred(and(vec![gram("Name"), not(gram("Abbr"))]))
            .interpretation(Name::first.inflected(FORMS))
            .match_relation(gnc.clone()),
        pred(and(vec![gram("Surn"), not(gram("Abbr"))]))
            .interpretation(Name::last.inflected(FORMS))
            .match_relation(gnc.clone()),
    ])
    .interpretation_fact::<Name>();

    let position = RuleBuilder::from_arc(morph_pipeline([
        "управляющий директор",
        "вице-мэр",
    ]));

    let person = rule([
        position
            .interpretation(Person::position.inflected(FORMS))
            .match_relation(gnc.clone()),
        name.interpretation(Person::name),
    ])
    .interpretation_fact::<Person>();

    let mut registry = RuleRegistry::new();
    let root_id = registry.add(person.build(()));
    registry.validate()?;           // Шаг не обязательный, но полезный

    let parser = Parser::new(&registry, root_id);
    let m = parser
        .r#match("управляющий директор Иван Ульянов")
        .expect("expected match");
    if let Some(fact) = m.fact(&registry) {
        println!("{fact}");
    }
    Ok(())
}
```

Результат:

```text
Person(
    position='управляющий директор',
    name=Name(
        first='иван',
        last='ульянов'
    )
)
```

Дополнительно: интерпретации, предикаты и отношения — в rustdoc модуля `interpretation`, `relations` и в примерах у `parser`.

## Соответствие Python yargy

В Python `and_` / `or_` / `not_` перегружены по типам аргументов. В Rust уровни разведены по именам:

| Python (`api.py`) | С корня `renert` |
|-------------------|----------------------|
| `rule(...)` | `rule()` |
| `or_` (правила) | `or_()` |
| `or_` (предикаты) | `or()` |
| `or_` (отношения) | `or_relation()` |
| `and_` (предикаты) | `and()` |
| `and_` (отношения) | `and_relation()` |
| `not_` (предикат) | `not()` |
| `not_` (отношение) | `not_relation()` |

Литералы в продукциях: `term("…")` или `pred(eq("…"))`. Полная таблица и примеры — в документации крейта (`cargo doc --no-deps --open`) или в примерах в модуле [docs](/docs).

## Производительность

Результаты нагрузочного тестирования может быть найден в [benchmarks.md](/benches/benchmarks.md). Там же находятся результаты сравнительного тестирования с `Yargy` на примере разложения адресов на сущности. Инструкция по запуску тестирования указана [здесь](/benches/README.md).

## Документация

- Локально: `cargo doc --no-deps --open`.
- Подробное описание библиотеки с примерами в модуле [docs](/docs).
- После публикации на [crates.io](https://crates.io).

## Благодарности

* Разработчикам [yargy](https://github.com/natasha/yargy) за создание источника вдохновения при разработке RENERT.
* Разработчикам [morph-rs](https://github.com/kribrum-os/morph-rs) за создание Rust версии морфологического анализатора для русского языка.
* Создателям [OpenCorpora](https://opencorpora.org/) за формирование словаря, который RENERT использует для морфологического анализа.