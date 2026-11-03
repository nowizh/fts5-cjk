# Changelog

Versions follow Semantic Versioning for the Rust API only. Index
compatibility does not follow the crate version; it follows the transform
profile policy below.

## Transform profile and semver policy

Any change to transform output requires a new transform profile and a full
reindex, regardless of the semver bump.
Crate semver never signals index compatibility.

Output changes that require a new profile include changes to the classifier
ranges, boundary classification, NFC ordering, UDF semantics, literal escaping
and phrase construction, and the tokenizer contract. A patch or minor release
is not exempt: if any of these change, the profile identity changes too.

Consumers persist the complete `ACTIVE_PROFILE` beside each index and compare
it with the compiled profile at startup. On any mismatch the consumer refuses
startup until an explicit rebuild or reindex with the compiled profile
succeeds; only then may it stamp the new profile metadata.

Every release entry below states whether any profile changed.

| Build | `id` | `algorithm_version` | `nfc` | `udf_name` | `recommended_tokenizer` |
|-------|------|---------------------|-------|------------|-------------------------|
| default (`nfc` feature off) | `fts5-cjk/v2;unicode61=6.1-default;nfc=off` | `2` | `false` | `fts5_cjk_v2` | `unicode61 remove_diacritics 2` |
| `features = ["nfc"]` | `fts5-cjk/v2;unicode61=6.1-default;nfc=on` | `2` | `true` | `fts5_cjk_v2_nfc` | `unicode61 remove_diacritics 2` |

## [0.1.1] - 2026-11-03

### Changed

- The `nfc` feature now requires `unicode-normalization` `>=0.1.25, <0.1.26`
  instead of `0.1.25` (a caret range), and the `nfc` build fails at compile
  time unless the normalization tables report Unicode 17.0.0.
  `unicode-normalization` 0.1.26 moves to Unicode 18.0.0 tables, which would
  change NFC output under the unchanged NFC-on profile. A dependency graph
  that requires a later `unicode-normalization` 0.1.x release elsewhere now
  fails resolution in `nfc` builds instead of silently changing index tokens.

### Development

Documentation and repository tooling. Except for `README.md` and
`CHANGELOG.md`, the files named here are not part of the published package.

- `README.md` documents the `unicode-normalization` bound, the MSRV, and how to
  select the development and MSRV toolchains.
- `mise.toml` and `mise.lock` pin the development toolchain, and
  `rust-toolchain.toml` mirrors them for rustup.
- `ci/check.sh` checks the active `rustc` and `cargo` against `mise.toml`,
  including before `clippy`, and gains a `portable` mode for source trees
  without Git metadata and an `msrv` mode that builds and tests both profiles
  on the declared MSRV toolchain.
- `.gitignore` ignores `target/`.

### Profiles

- No profile changed. Both profile identities and the transform output are
  unchanged. Adopting newer normalization tables is a separate decision that
  needs a new transform profile and a full reindex.

## [0.1.0] - 2026-09-29

### Added

- The v2 `normalize_cjk` transform for index and query input. It inserts one
  ASCII space between adjacent supported CJK scalars and at mixed
  CJK/token-character boundaries that SQLite `unicode61` would otherwise keep
  in one token, including the 25 combining marks consumed inside a token by
  `unicode61 remove_diacritics 2`. Unchanged input is returned borrowed.
- `denormalize_cjk`, a lossy display-only helper that collapses CJK-CJK
  spacing.
- `is_cjk`, which classifies one scalar against eight fixed ranges: Han core,
  Han Extension A, Han Extension B, Hiragana, Katakana, Hangul Syllables,
  Hangul Jamo and Hangul Compatibility Jamo.
- The literal helpers `quote_fts5_phrase` and `literal_query`, which build
  normalized, escaped FTS5 phrases in source order.
- `ACTIVE_PROFILE`, `TransformProfile`, `TRANSFORM_VERSION` and `UDF_NAME`,
  which expose the complete compile-time transform profile.
- The default-off `nfc` feature, which applies canonical NFC composition
  before the transform and selects the NFC-on profile.

### Profiles

- Introduces both `fts5-cjk/v2;unicode61=6.1-default;nfc=off` and
  `fts5-cjk/v2;unicode61=6.1-default;nfc=on`.

The declared `rust-version` is 1.97.1. The crate has no SQLite binding
dependency; consumers register the transform with their own SQLite binding.
