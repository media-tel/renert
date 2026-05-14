# Benchmarks: RENERT vs python yargy

Этот каталог содержит воспроизводимую инфраструктуру сравнения :

- Rust-бенчи (`criterion`) для `tokenize`, `parse`, `init` в `benches/rust/`.
- Python-бенчи (`pytest-benchmark`) в `benches/python/`.
- Наборы данных в `benches/data/`.
- Скрипты запуска/агрегации в `benches/scripts/`.

## Структура

- `data/address.txt` - полный тестовый датасет (5000 строк).
- `data/address_small.txt` - малый тестовый датасет (500 строк).
- `data/address_medium.txt` - средний тестовый датасет (2000 строк, seed=42).
- `data/SHA256SUMS` - контрольные суммы датасетов.
- `rust/rules.rs` и `python/rules.py` - правила разбора.
- `rust/address_parity.rs` - интеграционный parity-тест (Rust vs Python).
- `scripts/run_all.sh` — полный прогон; по умолчанию только вывод в терминал.
- `scripts/compare.py` — сведение Rust/Python в Markdown-таблицу (читает `target/criterion/`).
- `RESULTS.md` — опциональный журнал: создаётся и дописывается только в архивном режиме (см. ниже).

## Предусловия

- Rust toolchain (`cargo`, `rustc`) и словарь `dict.json` + `dict.fst`.
- Python 3.10+ и зависимости из `python/requirements.txt`.
- Переменная `YARGY_DICT_DIR` (опционально), иначе используется `data/dict`.

Для более воспроизводимых результатов рекомендуется фиксировать CPU и минимизировать фоновые процессы.

## Набор данных

Для тестирования библиотеки использован открытый датасет [Russian houses](https://www.kaggle.com/datasets/gasfarmuhametdinov/russian-houses) с платформы Kaggle, содержащий текстовые адреса на русском языке. Датасет распространяется под лицензией CC0 (Public Domain), что позволяет свободно использовать его в исследовательских целях.

## Быстрый запуск

```bash
python3 -m venv .venv
source .venv/bin/activate

python -m pip install -U pip "setuptools<81"
python -m pip install -r benches/python/requirements.txt

bash benches/scripts/run_all.sh
```

Скрипт:

1. Проверяет `sha256sum -c benches/data/SHA256SUMS`.
2. Запускает `cargo bench` для `tokenize_addresses`, `parse_addresses`, `init_bench`.
3. Запускает `pytest-benchmark` для Python-бенчей (JSON во временный файл).
4. Вызывает `compare.py` с `--criterion-root <repo>/target/criterion` и печатает сводную таблицу в stdout.

По умолчанию **ничего не копируется** в `benches/reports/` и **`benches/RESULTS.md` не изменяется**.

### Примечание для macOS

Скрипт `benches/scripts/run_all.sh` использует `sha256sum`.
В macOS по умолчанию обычно доступен `shasum -a 256`, а `sha256sum` может отсутствовать.

Установите GNU coreutils:

```bash
brew install coreutils
```

### Архивный прогон с сохранением результатов в файлы

```bash
YARGY_BENCH_ARCHIVE=1 bash benches/scripts/run_all.sh
```

Создаётся `benches/reports/<timestamp>/` с копией `python.json`, снимком `criterion/`, файлом `comparison.md`; таблица дописывается в `benches/RESULTS.md` (файл создаётся автоматически при первом таком запуске).

### Тест агрегатора Rust-метрик

```bash
python3 benches/scripts/test_compare_loader.py -v
```

### Интеграционный parity-тест Rust/Python

Файл теста: `benches/rust/address_parity.rs`. Для корректного запуска необходимо запустить виртуальную среду venv и установить requirements.

```bash
cargo test --test address_parity -- --ignored
```

### Ручной вызов `compare.py`

```bash
python3 benches/scripts/compare.py \
  --criterion-root target/criterion \
  --python-json /path/to/python.json \
  --output /tmp/comparison.md
```

Флаг `--output` можно опустить: таблица только в stdout.

## Локальный запуск отдельных сценариев

```bash
cargo bench --bench tokenize_addresses
cargo bench --bench parse_addresses
cargo bench --bench init_bench

python3 -m pytest benches/python --benchmark-warmup=on
```
