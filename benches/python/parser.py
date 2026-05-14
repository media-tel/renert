from yargy import Parser  # type: ignore[import-not-found]

from rules import ADDR


def build_address_parser() -> Parser:
    return Parser(ADDR)
