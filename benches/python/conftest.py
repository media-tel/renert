from pathlib import Path

import pytest

from parser import build_address_parser


BENCHES_ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = BENCHES_ROOT / "data"


def _load_lines(name: str) -> list[str]:
    path = DATA_ROOT / name
    return path.read_text(encoding="utf-8").splitlines()


@pytest.fixture(scope="session")
def parser():
    return build_address_parser()


@pytest.fixture(scope="session")
def lines_small() -> list[str]:
    return _load_lines("address_small.txt")


@pytest.fixture(scope="session")
def lines_medium() -> list[str]:
    return _load_lines("address_medium.txt")


@pytest.fixture(scope="session")
def lines_full() -> list[str]:
    return _load_lines("address.txt")
