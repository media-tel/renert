# RENERT. Эксперименты производительности.

## Условия прогона

- Машина: Intel(R) Core(TM) i3-10100F CPU @ 3.60GHz (8 потоков)
- ОС: Linux 6.17.0-20-generic
- Python: 3.12.3 (CPython)
- RAM: 16.0 GiB
- Дата фиксации результатов: 2026-04-21
- Источник цифр: `bash benches/scripts/run_all.sh`, stdout и артефакты `target/criterion/`

---

## I. Парсинг адресов в Rust (`criterion`)

### Что измеряется

Полный проход по строкам датасета и подсчёт всех совпадений `findall`.

```rust
group.bench_function(BenchmarkId::new("findall", dataset_name), |b| {
    b.iter(|| {
        let mut total_matches = 0usize;
        for line in &lines {
            let matches = parser.findall(black_box(line.as_str()));
            total_matches += matches.len();
        }
        black_box(total_matches);
    });
});
```

(фрагмент из `benches/rust/parse_addresses.rs`)

### Результаты

### renert/parse/findall/address_small

- time:   [1.6295 s **1.6310 s** 1.6329 s]
- thrpt:  [33.633 KiB/s **33.671 KiB/s** 33.702 KiB/s]

### renert/parse/findall/address_medium

- time:   [6.5858 s **6.5988 s** 6.6140 s]
- thrpt:  [32.875 KiB/s **32.951 KiB/s** 33.016 KiB/s]

### renert/parse/findall/address

- time:   [16.546 s **16.565 s** 16.589 s]
- thrpt:  [32.909 KiB/s **32.957 KiB/s** 32.994 KiB/s]

---

## II. Токенизация в Rust: plain vs morph

### Что измеряется

Сравнение обычной токенизации и морфо-токенизации на тех же датасетах.

```rust
group.bench_function(BenchmarkId::new("plain", dataset_name), |b| {
    b.iter(|| {
        let mut total_tokens = 0usize;
        for line in &lines {
            total_tokens += plain_tokenizer.tokenize(black_box(line.as_str())).len();
        }
        black_box(total_tokens);
    });
});

group.bench_function(BenchmarkId::new("morph", dataset_name), |b| {
    b.iter(|| {
        let mut total_tokens = 0usize;
        for line in &lines {
            total_tokens += morph_tokenizer.tokenize(black_box(line.as_str())).len();
        }
        black_box(total_tokens);
    });
});
```

(фрагмент из `benches/rust/tokenize_addresses.rs`)

### Результаты

### address_small

- plain time:  [2.4270 ms **2.4341 ms** 2.4449 ms]
- plain thrpt: [21.936 MiB/s **22.033 MiB/s** 22.098 MiB/s]
- morph time:  [40.108 ms **40.163 ms** 40.225 ms]
- morph thrpt: [1.3333 MiB/s **1.3353 MiB/s** 1.3372 MiB/s]

### address_medium

- plain time:  [9.6274 ms **9.7179 ms** 9.8500 ms]
- plain thrpt: [21.557 MiB/s **21.850 MiB/s** 22.056 MiB/s]
- morph time:  [165.19 ms **165.36 ms** 165.59 ms]
- morph thrpt: [1.2823 MiB/s **1.2841 MiB/s** 1.2854 MiB/s]

### address

- plain time:  [24.047 ms **24.083 ms** 24.125 ms]
- plain thrpt: [22.098 MiB/s **22.137 MiB/s** 22.170 MiB/s]
- morph time:  [405.61 ms **406.14 ms** 406.97 ms]
- morph thrpt: [1.3100 MiB/s **1.3126 MiB/s** 1.3144 MiB/s]

---

## III. Инициализация (cold start в отдельном процессе)

### Что измеряется

Время на `renert::load(...)` + построение правил в отдельном процессе (fresh process).

```rust
fn run_child_workload() -> Result<(), String> {
    let start = Instant::now();
    renert::load(common::dict_dir()).map_err(|err| err.to_string())?;
    let _ = rules::build_address_rules();
    println!("{}", start.elapsed().as_nanos());
    Ok(())
}
```

(фрагмент из `benches/rust/init_bench.rs`)

### Результат

### renert/init/load_and_build_rules/fresh_process

- time:   [1.3890 s **1.3908 s** 1.3925 s]

---

## IV. Python yargy + pymorphy2 (`pytest-benchmark`)

### Что измеряется

Подсчёт количества фактов через `parser.findall(...)` на трёх датасетах.

```python
def _run_parse(parser, lines: list[str]) -> int:
    total = 0
    for line in lines:
        total += len(list(parser.findall(line)))
    return total
```

(фрагмент из `benches/python/test_parse.py`)

Сессия: 3 passed, 1 warning, ~7268 s (~2:01:08), `facts_count` фиксируется в `benchmark.extra_info`.

### Результаты

### test_parse_address_small (`address_small`, facts: 2580)

- time (median): **32.066 s** (31.915 s ... 32.190 s)

### test_parse_address_medium (`address_medium`, facts: 10187)

- time (median): **140.418 s** (140.189 s ... 141.109 s)

### test_parse_address (`address`, facts: 25525)

- time (median): **387.910 s** (387.281 s ... 388.849 s)

---

## V. Сводка Rust vs Python (parse)

| dataset        | Rust median (findall) | Python median | speedup (python/rust) |
|----------------|----------------------:|--------------:|----------------------:|
| address_small  | 1.630 s               | 32.066 s      | 19.68x                |
| address_medium | 6.576 s               | 140.418 s     | 21.35x                |
| address        | 16.542 s              | 387.910 s     | 23.45x                |

Медианы взяты из текущего локального отчёта и сопоставлены в формате таблицы `compare.py`.
