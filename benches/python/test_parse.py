import pytest


def _run_parse(parser, lines: list[str]) -> int:
    total = 0
    for line in lines:
        total += len(list(parser.findall(line)))
    return total


@pytest.mark.benchmark(group="python_yargy/parse")
def test_parse_address_small(benchmark, parser, lines_small):
    facts_count = benchmark(lambda: _run_parse(parser, lines_small))
    benchmark.extra_info["dataset"] = "address_small"
    benchmark.extra_info["facts_count"] = facts_count


@pytest.mark.benchmark(group="python_yargy/parse")
def test_parse_address_medium(benchmark, parser, lines_medium):
    facts_count = benchmark(lambda: _run_parse(parser, lines_medium))
    benchmark.extra_info["dataset"] = "address_medium"
    benchmark.extra_info["facts_count"] = facts_count


@pytest.mark.benchmark(group="python_yargy/parse")
def test_parse_address(benchmark, parser, lines_full):
    facts_count = benchmark(lambda: _run_parse(parser, lines_full))
    benchmark.extra_info["dataset"] = "address"
    benchmark.extra_info["facts_count"] = facts_count
