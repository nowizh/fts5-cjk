#!/usr/bin/env python3
"""Generate the checked-in effective SQLite unicode61 classifier table."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path
from typing import Iterable

INPUT_SHA256 = "3066262585a3c4f407b16db787e6d3a6e033b90f27405b6c76d1babefffca6ad"
SQLITE_DIACRITIC_INPUT_SHA256 = (
    "8ec0c1ea2f07b1bf2bed6f72683eee56bd1f170131629b821fddfec8b41fc8e6"
)
EXPECTED_RANGE_COUNT = 403
EXPECTED_TOKEN_SCALAR_COUNT = 1_104_042
EXPECTED_REMOVED_DIACRITIC_COUNT = 25
MAX_CODEPOINT = 0x10FFFF
SURROGATE_START = 0xD800
SURROGATE_END = 0xDFFF
FORCED_SEPARATORS = frozenset((0x0000, 0xFFFE, 0xFFFF))

TOOL_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOL_DIR.parent.parent
INPUT_PATH = TOOL_DIR / "UnicodeData-6.1.0.txt"
SQLITE_DIACRITIC_INPUT_PATH = TOOL_DIR / "sqlite3-3.53.2-fts5-diacritics.c"
OUTPUT_PATH = REPO_ROOT / "src" / "unicode61_data.rs"


def verify_input_hash(path: Path, expected: str, label: str) -> bytes:
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected:
        raise ValueError(f"{label} input hash mismatch: expected {expected}, got {actual}")
    return data


def parse_removed_diacritics(data: bytes) -> list[int]:
    source = data.decode("utf-8")
    mask0_match = re.search(r"mask0\s*=\s*0x([0-9A-Fa-f]+)", source)
    mask1_match = re.search(r"mask1\s*=\s*0x([0-9A-Fa-f]+)", source)
    bounds_match = re.search(r"c<(\d+)\s*\|\|\s*c>(\d+)", source)
    split_match = re.search(r"c\s*<\s*(\d+)\+(\d+)", source)
    if not all((mask0_match, mask1_match, bounds_match, split_match)):
        raise ValueError("malformed SQLite FTS5 diacritic source excerpt")

    assert mask0_match is not None
    assert mask1_match is not None
    assert bounds_match is not None
    assert split_match is not None
    mask0 = int(mask0_match.group(1), 16)
    mask1 = int(mask1_match.group(1), 16)
    lower = int(bounds_match.group(1))
    upper = int(bounds_match.group(2))
    split = int(split_match.group(1)) + int(split_match.group(2))
    if (lower, upper, split) != (768, 817, 800):
        raise ValueError(
            "SQLite FTS5 diacritic bounds drifted: "
            f"expected (768, 817, 800), got {(lower, upper, split)}"
        )

    codepoints = [
        cp
        for cp in range(lower, upper + 1)
        if ((mask0 >> (cp - lower)) & 1)
        if cp < split
    ]
    codepoints.extend(
        cp
        for cp in range(split, upper + 1)
        if (mask1 >> (cp - split)) & 1
    )
    if len(codepoints) != EXPECTED_REMOVED_DIACRITIC_COUNT:
        raise ValueError(
            "removed-diacritic count drifted: "
            f"expected {EXPECTED_REMOVED_DIACRITIC_COUNT}, got {len(codepoints)}"
        )
    return codepoints


def parse_unicode_data(data: bytes) -> list[str | None]:
    categories: list[str | None] = [None] * (MAX_CODEPOINT + 1)
    pending_range: tuple[int, str, str] | None = None

    for line_number, raw_line in enumerate(data.decode("utf-8").splitlines(), 1):
        fields = raw_line.split(";")
        if len(fields) < 3:
            raise ValueError(f"malformed UnicodeData row at line {line_number}")
        cp = int(fields[0], 16)
        name = fields[1]
        category = fields[2]

        if name.endswith(", First>"):
            if pending_range is not None:
                raise ValueError(f"nested First record at line {line_number}")
            pending_range = (cp, name.removesuffix(", First>"), category)
            continue

        if name.endswith(", Last>"):
            if pending_range is None:
                raise ValueError(f"Last record without First at line {line_number}")
            start, first_name, first_category = pending_range
            last_name = name.removesuffix(", Last>")
            if first_name != last_name or first_category != category or cp < start:
                raise ValueError(f"mismatched First/Last records at line {line_number}")
            for range_cp in range(start, cp + 1):
                if categories[range_cp] is not None:
                    raise ValueError(f"duplicate codepoint U+{range_cp:04X}")
                categories[range_cp] = category
            pending_range = None
            continue

        if pending_range is not None:
            raise ValueError(f"unterminated First record before line {line_number}")
        if categories[cp] is not None:
            raise ValueError(f"duplicate codepoint U+{cp:04X}")
        categories[cp] = category

    if pending_range is not None:
        raise ValueError("unterminated First record at end of UnicodeData")
    return categories


def is_valid_rust_scalar(cp: int) -> bool:
    return 0 <= cp <= MAX_CODEPOINT and not SURROGATE_START <= cp <= SURROGATE_END


def is_effective_token(cp: int, categories: list[str | None]) -> bool:
    if not is_valid_rust_scalar(cp) or cp in FORCED_SEPARATORS:
        return False
    category = categories[cp]
    if category is None:
        return True
    return category.startswith(("L", "N")) or category == "Co"


def merge_ranges(token_scalars: Iterable[int]) -> list[tuple[int, int]]:
    ranges: list[tuple[int, int]] = []
    start: int | None = None
    previous: int | None = None

    for cp in token_scalars:
        if start is None:
            start = previous = cp
        elif previous is not None and cp == previous + 1:
            previous = cp
        else:
            assert previous is not None
            ranges.append((start, previous))
            start = previous = cp

    if start is not None:
        assert previous is not None
        ranges.append((start, previous))
    return ranges


def render_rust(ranges: list[tuple[int, int]], removed_diacritics: list[int]) -> bytes:
    scalar_count = sum(end - start + 1 for start, end in ranges)
    if len(ranges) != EXPECTED_RANGE_COUNT:
        raise ValueError(
            f"effective range count drifted: expected {EXPECTED_RANGE_COUNT}, got {len(ranges)}"
        )
    if scalar_count != EXPECTED_TOKEN_SCALAR_COUNT:
        raise ValueError(
            "effective token-scalar count drifted: "
            f"expected {EXPECTED_TOKEN_SCALAR_COUNT}, got {scalar_count}"
        )

    lines = [
        "// Generated by tools/unicode61/generate.py. Do not edit by hand.",
        "// Source: UnicodeData-6.1.0.txt",
        f"// Input SHA-256: {INPUT_SHA256}",
        "// Removed-diacritic source: SQLite 3.53.2 ext/fts5/fts5_unicode2.c",
        f"// Source-excerpt SHA-256: {SQLITE_DIACRITIC_INPUT_SHA256}",
        "// Effective SQLite unicode61 policy: absent category-0 scalars are tokens;",
        "// U+0000, U+FFFE, and U+FFFF are forced separators.",
        f"// {EXPECTED_RANGE_COUNT} ranges; {EXPECTED_TOKEN_SCALAR_COUNT} token scalars.",
        f"// {EXPECTED_REMOVED_DIACRITIC_COUNT} removed-diacritic continuations.",
        "",
        "pub(crate) const UNICODE61_TOKEN_RANGES: &[(u32, u32)] = &[",
    ]
    lines.extend(f"    (0x{start:06X}, 0x{end:06X})," for start, end in ranges)
    lines.extend(
        [
            "];",
            "",
            "pub(crate) const UNICODE61_REMOVED_DIACRITICS: &[u32] = &[",
        ]
    )
    for offset in range(0, len(removed_diacritics), 9):
        row = removed_diacritics[offset : offset + 9]
        lines.append("    " + " ".join(f"0x{cp:06X}," for cp in row))
    lines.extend(["];"])
    return ("\n".join(lines) + "\n").encode("utf-8")


def check_output(expected: bytes, output_path: Path) -> None:
    try:
        actual = output_path.read_bytes()
    except FileNotFoundError as error:
        raise ValueError(f"generated output is missing: {output_path}") from error
    if actual != expected:
        raise ValueError(
            f"generated output differs: run {sys.executable} {Path(__file__)}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in output without modifying it",
    )
    args = parser.parse_args()

    data = verify_input_hash(INPUT_PATH, INPUT_SHA256, "Unicode 6.1")
    sqlite_diacritic_data = verify_input_hash(
        SQLITE_DIACRITIC_INPUT_PATH,
        SQLITE_DIACRITIC_INPUT_SHA256,
        "SQLite 3.53.2 FTS5 diacritic source",
    )
    categories = parse_unicode_data(data)
    removed_diacritics = parse_removed_diacritics(sqlite_diacritic_data)
    token_scalars = (
        cp
        for cp in range(MAX_CODEPOINT + 1)
        if is_effective_token(cp, categories)
    )
    rendered = render_rust(merge_ranges(token_scalars), removed_diacritics)

    if args.check:
        check_output(rendered, OUTPUT_PATH)
    else:
        OUTPUT_PATH.write_bytes(rendered)
        print(f"wrote {OUTPUT_PATH.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, UnicodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
