# Frozen SQLite unicode61 classifier data

This directory contains the development inputs and generator for the private
effective SQLite `unicode61` token-character and removed-diacritic
continuation data in `src/unicode61_data.rs`. Cargo compiles only the
checked-in Rust data. It never invokes this generator or reads the upstream
inputs during a crate build.

## Pinned upstream input

- Unicode version: 6.1.0
- Official source: <https://www.unicode.org/Public/6.1.0/ucd/UnicodeData.txt>
- Local copy: `tools/unicode61/UnicodeData-6.1.0.txt`
- SHA-256: `3066262585a3c4f407b16db787e6d3a6e033b90f27405b6c76d1babefffca6ad`
- Upstream data license: `tools/unicode61/LICENSE-UNICODE`
- Official license source: <https://www.unicode.org/license.txt>

The Unicode data license is separate from the crate's root MIT `LICENSE`.
The generator validates the exact SHA-256 before parsing any records and
recognizes paired `<..., First>` and `<..., Last>` records.

The exact persisted tokenizer is `unicode61 remove_diacritics 2`. Its special
combining-mark continuation policy is pinned separately to SQLite 3.53.2:

- Official source: <https://github.com/sqlite/sqlite/blob/version-3.53.2/ext/fts5/fts5_unicode2.c>
- Frozen source excerpt: `tools/unicode61/sqlite3-3.53.2-fts5-diacritics.c`
- SQLite source ID: `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`
- Source-excerpt SHA-256: `8ec0c1ea2f07b1bf2bed6f72683eee56bd1f170131629b821fddfec8b41fc8e6`

The excerpt preserves the exact `sqlite3Fts5UnicodeIsdiacritic` masks from the
pinned implementation. The generator validates its hash and derives the
frozen set from those masks instead of inferring it from Unicode categories.

## Effective SQLite policy

The generated table models executable SQLite behavior rather than only the
assigned Unicode 6.1 `L*`, `N*`, and `Co` rows. A valid Rust scalar absent
from UnicodeData 6.1 occupies SQLite's category 0, which the default
`unicode61` tokenizer treats as a token character. For example, unassigned
U+0378 is a token character. U+0000 is forced to a separator by SQLite, and
SQLite's UTF-8 decoding makes U+FFFE and U+FFFF separators as well. Surrogate
code points are excluded because they are not valid Rust `char` values.

This policy deterministically produces exactly 403 sorted inclusive ranges
covering 1,104,042 valid token scalars. SQLite additionally treats exactly 25
combining marks as continuations inside a token and removes them when
`remove_diacritics 2` is active. The generated representation keeps the range
table and that sorted 25-scalar continuation set separate. It uses fixed-width
uppercase hexadecimal values and has no runtime dependency.

## Offline regeneration

The generator uses only the Python standard library, performs no network
access, and reads the pinned local input. Regenerate the checked-in Rust bytes
from the repository root:

```sh
python3 tools/unicode61/generate.py
```

Verify both the input hash and byte-exact checked-in output without modifying
the working tree:

```sh
python3 tools/unicode61/generate.py --check
```

There is no `build.rs` and neither Cargo nor rustdoc invokes this tool. A
reviewer can therefore reproduce or check the table offline after cloning the
repository.

## Executable SQLite oracle

The private Rust oracle starts one `sqlite3 -batch -bail :memory:` child
directly, sends a numeric SQL script over stdin, and observes
`fts5vocab(instance)` token counts. It retains all 1,449 unique cases formed
from the 403 range endpoints and every immediate valid-scalar neighbour, then
adds 11 missing semantic representatives for exactly 1,460 classifier cases.
The representatives cover BMP and astral text, unassigned and private-use
scalars, a genuine combining-mark separator, punctuation, underscore,
whitespace, digits, U+0000, U+FFFE, and U+FFFF.

The same SQLite process also exercises every one of the 25 frozen removed
diacritics in both CJK transition directions. Those 50 transition cases prove
that each mark continues the surrounding token under the exact persisted
tokenizer. The complete executable matrix therefore contains 1,510
observations: 1,449 range-edge cases, 11 additional representatives, and 50
removed-diacritic transitions. U+0345 remains a separator counterexample.
Missing `sqlite3`, missing FTS5 support, child failure, malformed output,
missing rows, or any mismatch fails the test; no case is silently skipped.

Run the mandatory oracle with:

```sh
cargo test --locked --no-default-features \
  classification::oracle::effective_unicode61_classifier_matches_sqlite \
  -- --exact --nocapture
```

The oracle prints `sqlite_version()` and `sqlite_source_id()`. A recorded run
on 2026-08-30 used SQLite 3.45.1 with source ID
`2024-01-30 16:01:20 e876e51a0ed5c5b3126f52e532044363a014bc594cfefa87ffb5b82257ccalt1`
and found zero mismatches. That run identifies the executable evidence used
here; it is not the CI engine pin. The CI engine pin is SQLite 3.53.2, which
`ci/check.sh` provisions and identity-checks together with the real-FTS5
MATCH-behavior test in `tests/fts5_contract.rs`. This oracle proves
tokenizer continuation semantics only; it does not execute or claim
helper-generated `MATCH` results.

Unicode generation and the crate tests depend on nothing outside this
repository at build time.
