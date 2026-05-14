"""Unit tests for benches/scripts/compare.py (run: python3 benches/scripts/test_compare_loader.py)."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

# Import compare.py as a plain module (directory is not a Python package).
_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

import compare  # noqa: E402


MINIMAL_ESTIMATE = {
    "median": {"point_estimate": 1_649_000_000},
}


class TestLoadRustMetrics(unittest.TestCase):
    def test_renert_parse_sanitized_path(self) -> None:
        """Criterion 0.5 uses a single path segment instead of `renert/parse`."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            est = root / "renert_parse" / "findall" / "address_small" / "estimates.json"
            est.parent.mkdir(parents=True)
            est.write_text(json.dumps(MINIMAL_ESTIMATE), encoding="utf-8")

            metrics = compare.load_rust_metrics(root)
            self.assertIn("address_small", metrics)
            self.assertAlmostEqual(metrics["address_small"].median_ms, 1649.0, places=3)

    def test_tokenize_path_ignored(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            est = root / "renert_tokenize" / "plain" / "address_small" / "estimates.json"
            est.parent.mkdir(parents=True)
            est.write_text(json.dumps(MINIMAL_ESTIMATE), encoding="utf-8")

            metrics = compare.load_rust_metrics(root)
            self.assertEqual(metrics, {})


if __name__ == "__main__":
    unittest.main()
