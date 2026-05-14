# Benchmark Results

Все контрольные замеры проводились на машине с процессором Intel(R) Core(TM) i3-10100F CPU @ 3.60GHz (8 потоков), ОС Linux 6.17.0-20-generic, Python 3.12.3 (CPython) и RAM-памятью 16,0 GiB.

Запуск: `bash benches/scripts/run_all.sh` (режим по умолчанию, без `YARGY_BENCH_ARCHIVE`). Источник цифр: stdout прогона; артефакты Criterion в `target/criterion/`. Дата фиксации отчёта: 2026-04-21.

---

# Rust: RENERT (Criterion)

## Парсинг адресов (`findall`, весь датасет построчно)

### renert/parse/findall/address_small

100 samples

- time:   [1.6295 s **1.6310 s** 1.6329 s]
- thrpt:  [33.633 KiB/s **33.671 KiB/s** 33.702 KiB/s]

### renert/parse/findall/address_medium

100 samples

- time:   [6.5858 s **6.5988 s** 6.6140 s]
- thrpt:  [32.875 KiB/s **32.951 KiB/s** 33.016 KiB/s]

### renert/parse/findall/address

100 samples

- time:   [16.546 s **16.565 s** 16.589 s]
- thrpt:  [32.909 KiB/s **32.957 KiB/s** 32.994 KiB/s]

## Инициализация (холодный процесс)

### renert/init/load_and_build_rules/fresh_process

10 samples

- time:   [1.3890 s **1.3908 s** 1.3925 s]

## Токенизация

### renert/tokenize/plain/address_small

100 samples

- time:   [2.4270 ms **2.4341 ms** 2.4449 ms]
- thrpt:  [21.936 MiB/s **22.033 MiB/s** 22.098 MiB/s]

### renert/tokenize/morph/address_small

100 samples

- time:   [40.108 ms **40.163 ms** 40.225 ms]
- thrpt:  [1.3333 MiB/s **1.3353 MiB/s** 1.3372 MiB/s]

### renert/tokenize/plain/address_medium

100 samples

- time:   [9.6274 ms **9.7179 ms** 9.8500 ms]
- thrpt:  [21.557 MiB/s **21.850 MiB/s** 22.056 MiB/s]

### renert/tokenize/morph/address_medium

100 samples

- time:   [165.19 ms **165.36 ms** 165.59 ms]
- thrpt:  [1.2823 MiB/s **1.2841 MiB/s** 1.2854 MiB/s]

### renert/tokenize/plain/address

100 samples

- time:   [24.047 ms **24.083 ms** 24.125 ms]
- thrpt:  [22.098 MiB/s **22.137 MiB/s** 22.170 MiB/s]

### renert/tokenize/morph/address

100 samples

- time:   [405.61 ms **406.14 ms** 406.97 ms]
- thrpt:  [1.3100 MiB/s **1.3126 MiB/s** 1.3144 MiB/s]

---

# Python: yargy + pymorphy2 (`pytest-benchmark`, группа `python_yargy/parse`)

5 rounds × 1 iteration; `facts_count` — число извлечённых фактов на датасете. Сессия: 3 passed, 1 warning в ~7268 s (~2:01:08); предупреждение из `pymorphy2` / `pkg_resources`.

### test_parse_address_small (`address_small`, facts: 2580)

- time (median): **32.066 s** (31.915 s … 32.190 s по min/max раундов)

### test_parse_address_medium (`address_medium`, facts: 10187)

- time (median): **140.418 s** (140.189 s … 141.109 s по min/max раундов)

### test_parse_address (`address`, facts: 25525)

- time (median): **387.910 s** (387.281 s … 388.849 s по min/max раундов)

---

# Сводка Rust vs Python (parse)

Оценка ускорения Python/Rust по медианам из сводной таблицы `compare.py` того же прогона (медианы в таблице — в миллисекундах; здесь переведены в секунды для сопоставления с Criterion).

| dataset        | Rust median (findall) | Python median | speedup (python/rust) |
|----------------|----------------------:|---------------:|----------------------:|
| address_small  | 1.630 s               | 32.066 s       | 19.68×                |
| address_medium | 6.576 s               | 140.418 s      | 21.35×                |
| address        | 16.542 s              | 387.910 s      | 23.45×                |
