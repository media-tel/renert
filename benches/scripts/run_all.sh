#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

PY_JSON="$(mktemp)"
cleanup() {
  rm -f "$PY_JSON"
}
trap cleanup EXIT

(cd benches/data && sha256sum -c SHA256SUMS)

cargo bench --bench tokenize_addresses
cargo bench --bench parse_addresses
cargo bench --bench init_bench

python3 -m pytest benches/python -q \
  --benchmark-json="$PY_JSON" \
  --benchmark-warmup=on

CRIT_ROOT="$ROOT_DIR/target/criterion"

if [[ "${YARGY_BENCH_ARCHIVE:-0}" == "1" ]]; then
  TIMESTAMP="$(date -u +%Y%m%d-%H%M%S)"
  REPORT_DIR="benches/reports/$TIMESTAMP"
  mkdir -p "$REPORT_DIR/criterion"
  cp "$PY_JSON" "$REPORT_DIR/python.json"
  cp -R "$CRIT_ROOT/." "$REPORT_DIR/criterion/"
  python3 benches/scripts/compare.py \
    --criterion-root "$CRIT_ROOT" \
    --python-json "$PY_JSON" \
    --output "$REPORT_DIR/comparison.md" \
    --append-results benches/RESULTS.md
  echo "Reports saved to $REPORT_DIR"
else
  python3 benches/scripts/compare.py \
    --criterion-root "$CRIT_ROOT" \
    --python-json "$PY_JSON"
fi
