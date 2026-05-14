#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


DATASETS = ("address_small", "address_medium", "address")


@dataclass
class RustMetric:
    median_ms: float


@dataclass
class PythonMetric:
    median_ms: float
    facts_count: int | None


def _is_parse_findall_estimate(path: Path) -> bool:
    """Criterion 0.5 turns group `renert/parse` into a path segment like `renert_parse`."""
    posix = path.as_posix()
    return "findall" in posix


def load_rust_metrics(criterion_root: Path) -> dict[str, RustMetric]:
    metrics: dict[str, RustMetric] = {}
    for estimate_path in criterion_root.rglob("estimates.json"):
        parts = set(estimate_path.parts)
        dataset = next((name for name in DATASETS if name in parts), None)
        if dataset is None:
            continue
        if not _is_parse_findall_estimate(estimate_path):
            continue

        data = json.loads(estimate_path.read_text(encoding="utf-8"))
        median_ns = data["median"]["point_estimate"]
        metrics[dataset] = RustMetric(median_ms=median_ns / 1_000_000.0)
    return metrics


def load_python_metrics(path: Path) -> dict[str, PythonMetric]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    metrics: dict[str, PythonMetric] = {}

    for benchmark in raw.get("benchmarks", []):
        extra = benchmark.get("extra_info", {})
        dataset = extra.get("dataset")
        if dataset not in DATASETS:
            continue
        median_seconds = benchmark["stats"]["median"]
        metrics[dataset] = PythonMetric(
            median_ms=median_seconds * 1_000.0,
            facts_count=extra.get("facts_count"),
        )

    return metrics


def render_table(rust: dict[str, RustMetric], python: dict[str, PythonMetric]) -> str:
    lines = [
        "| dataset | rust_median_ms | python_median_ms | speedup (python/rust) | facts_count_python |",
        "|---|---:|---:|---:|---:|",
    ]

    for dataset in DATASETS:
        rust_metric = rust.get(dataset)
        python_metric = python.get(dataset)

        rust_ms = rust_metric.median_ms if rust_metric else None
        py_ms = python_metric.median_ms if python_metric else None
        speedup = (py_ms / rust_ms) if (rust_ms and py_ms) else None
        facts_count = python_metric.facts_count if python_metric else None

        lines.append(
            "| {dataset} | {rust_ms} | {py_ms} | {speedup} | {facts_count} |".format(
                dataset=dataset,
                rust_ms=f"{rust_ms:.3f}" if rust_ms is not None else "n/a",
                py_ms=f"{py_ms:.3f}" if py_ms is not None else "n/a",
                speedup=f"{speedup:.2f}x" if speedup is not None else "n/a",
                facts_count=facts_count if facts_count is not None else "n/a",
            )
        )

    return "\n".join(lines)


def append_results(results_path: Path, table: str) -> None:
    results_path.parent.mkdir(parents=True, exist_ok=True)
    if not results_path.exists():
        results_path.write_text("# Benchmark Results\n\n", encoding="utf-8")

    with results_path.open("a", encoding="utf-8") as fh:
        fh.write(f"\n## Run {datetime.now(timezone.utc).isoformat()}\n\n")
        fh.write(table)
        fh.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Compare Criterion and pytest-benchmark outputs.")
    parser.add_argument("--criterion-root", type=Path, required=True)
    parser.add_argument("--python-json", type=Path, required=True)
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="If set, write the comparison table to this path (UTF-8).",
    )
    parser.add_argument("--append-results", type=Path)
    args = parser.parse_args()

    rust = load_rust_metrics(args.criterion_root)
    python = load_python_metrics(args.python_json)
    table = render_table(rust, python)

    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(table + "\n", encoding="utf-8")
    print(table)

    if args.append_results:
        append_results(args.append_results, table)


if __name__ == "__main__":
    main()
