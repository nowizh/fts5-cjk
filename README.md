# fts5-cjk

`fts5-cjk` provides a pure Rust CJK transform and narrow literal-query string
helpers for SQLite FTS5. The index writer and query builder must use the same
compiled `ACTIVE_PROFILE`.

The default build has no dependencies. The optional `nfc` feature adds only
`unicode-normalization`. The crate has no SQLite dependency: the consumer
registers the transform with its own SQLite binding.

## Installation

Default profile (no NFC composition):

```toml
[dependencies]
fts5-cjk = "0.1"
```

NFC-on profile:

```toml
[dependencies]
fts5-cjk = { version = "0.1", features = ["nfc"] }
```

Choose the profile deliberately. The two profiles produce different index
tokens, and Cargo feature unification can enable `nfc` from another dependency
path in the same build, so always check the compiled `ACTIVE_PROFILE`.

## Fixed classifier policy

`is_cjk(char)` classifies one Rust Unicode scalar against eight fixed,
versioned ranges: Han core, Han Extension A, Han Extension B, Hiragana,
Katakana, Hangul Syllables, Hangul Jamo, and Hangul Compatibility Jamo. These
ranges are a compatibility policy, not an exhaustive or automatically updated
definition of the latest Unicode CJK repertoire.

The exact inclusive ranges are `U+4E00..U+9FFF`, `U+3400..U+4DBF`,
`U+20000..U+2A6DF`, `U+3040..U+309F`, `U+30A0..U+30FF`,
`U+AC00..U+D7A3`, `U+1100..U+11FF`, and `U+3130..U+318F`.

## Compiled profile

`ACTIVE_PROFILE` is the complete identity of the contract compiled into the
crate. The default build selects the NFC-off profile; enabling the default-off
`nfc` Cargo feature selects a distinct NFC-on profile and SQL UDF name. Cargo
features are additive and may be unified by another dependency path, so a
consumer must inspect the compiled `ACTIVE_PROFILE` rather than infer it from
deployment configuration.

Persist and compare the complete profile, including its ID, algorithm version,
NFC state, SQL UDF name, and recommended tokenizer. Package semver and
`TRANSFORM_VERSION` alone are insufficient: the NFC-off and NFC-on profiles
share algorithm version 2 while remaining incompatible identities.

| Build | `id` | `algorithm_version` | `nfc` | `udf_name` | `recommended_tokenizer` |
|-------|------|---------------------|-------|------------|-------------------------|
| default (`nfc` feature off) | `fts5-cjk/v2;unicode61=6.1-default;nfc=off` | `2` | `false` | `fts5_cjk_v2` | `unicode61 remove_diacritics 2` |
| `features = ["nfc"]` | `fts5-cjk/v2;unicode61=6.1-default;nfc=on` | `2` | `true` | `fts5_cjk_v2_nfc` | `unicode61 remove_diacritics 2` |

## Transform profile and semver policy

Any change to transform output requires a new transform profile and a full
reindex, regardless of the semver bump.
Crate semver never signals index compatibility.

Crate semver describes Rust API compatibility only. Every release entry in
CHANGELOG.md states whether any profile changed.

Any change that affects output or search compatibility requires a new profile
identity. This includes classifier range changes, boundary-classification
changes, NFC ordering, UDF semantics, literal escaping and phrase
construction, and the tokenizer contract.

The NFC-on profile is defined against the Unicode 17.0.0 normalization tables
of `unicode-normalization` 0.1.25. The `nfc` feature therefore uses the
upper-bounded requirement `>=0.1.25, <0.1.26` rather than a caret range, and a
compile-time check rejects any other table version. Newer tables are accepted
only together with a new transform profile and a full reindex. A build whose
dependency graph also requires a later `unicode-normalization` 0.1.x release
fails dependency resolution when the `nfc` feature is enabled instead of silently changing index tokens.

When persisted index metadata differs from the compiled `ACTIVE_PROFILE`, the
consumer must refuse startup. Recovery requires an explicit rebuild or reindex
using the compiled contract; only after that succeeds may the consumer stamp
the new profile metadata. It must never automatically restamp stale metadata,
because doing so would falsely certify old index tokens as compatible.

## Pure transform API

`normalize_cjk` inserts one ASCII space between adjacent supported CJK scalars
and at mixed boundaries where SQLite `unicode61` would otherwise keep the CJK
and non-CJK scalars in one token. This includes the 25 combining marks that
the pinned `unicode61 remove_diacritics 2` tokenizer consumes inside a token;
it is not inferred from Unicode general categories alone. Existing
punctuation and whitespace boundaries remain unchanged. Unchanged input uses
the `Cow::Borrowed` fast path.

```rust
use fts5_cjk::{denormalize_cjk, normalize_cjk};

assert_eq!(normalize_cjk("AI개발중"), "AI 개 발 중");
assert_eq!(normalize_cjk("개\u{0301}발"), "개 \u{0301} 발");
assert_eq!(normalize_cjk("개\u{0345}발"), "개\u{0345}발");
assert_eq!(denormalize_cjk("AI 개 발 중"), "AI 개발중");
```

`denormalize_cjk` is lossy and display-only. It removes eligible single spaces
only between supported CJK scalars; it cannot tell original spaces from spaces
inserted by the transform and must not be used to reconstruct source text.

With the default-off `nfc` feature, `normalize_cjk` composes NFC before adding
boundaries. Both index input and query helpers therefore follow the same active
transform; enabling the feature selects a different persisted profile.

## Literal query strings

`quote_fts5_phrase` normalizes one complete raw value, doubles every embedded
double quote, and wraps the result in exactly one quote pair.
`literal_query` first applies `split_whitespace` to the raw input, quotes each
term independently through `quote_fts5_phrase`, preserves source order, and
joins the phrases with one ASCII space for implicit AND.

```rust
use fts5_cjk::{literal_query, quote_fts5_phrase};

assert_eq!(quote_fts5_phrase("AI개"), "\"AI 개\"");
assert_eq!(literal_query("개발 hello"), "\"개 발\" \"hello\"");
assert_eq!(quote_fts5_phrase(""), "\"\"");
assert_eq!(literal_query(" \t\n"), "");
```

An empty `literal_query` result is a rejection signal: consumers must reject
it before binding a value to `MATCH`. In contrast, `quote_fts5_phrase("")`
deliberately returns the one-phrase string `""`.

These helpers accept literal values only. They do not parse or support Boolean,
column, `NEAR`, prefix, negation, or parenthesized MATCH grammar.

## Integrating with SQLite

### Register the active transform

`fts5-cjk` has no SQLite dependency; the consumer owns its SQLite binding and
registers the transform as a scalar SQL function through it. With `rusqlite`,
that is the consumer's own dependency line:

```toml
[dependencies]
rusqlite = { version = "0.40", features = ["functions"] }
```

```rust,ignore
// Consumer code, rusqlite 0.40 with the `functions` feature.
// fts5-cjk itself has no SQLite dependency.
use fts5_cjk::{ACTIVE_PROFILE, normalize_cjk};
use rusqlite::{Connection, functions::FunctionFlags};

fn register_fts5_cjk(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function(
        ACTIVE_PROFILE.udf_name,
        1,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| {
            let raw: Option<String> = ctx.get(0)?;
            Ok(raw.map(|raw| normalize_cjk(&raw).into_owned()))
        },
    )
}
```

Register exactly one text argument under `ACTIVE_PROFILE.udf_name`, using the
binding's UTF-8, deterministic, and `SQLITE_INNOCUOUS`-equivalent
scalar-function flags. The innocuous flag is required when consumer-owned
views or triggers invoke the UDF while SQLite's trusted-schema hardening is
active. Never use `SQLITE_DIRECTONLY`: index maintenance invokes this UDF from
schema objects, so that flag would reject the supported write path.

If the selected binding or SQLite version cannot expose or honor
`SQLITE_INNOCUOUS`, treat that as an explicit compatibility branch: keep
`trusted_schema` enabled on connections that execute these consumer-owned
schema objects, or upgrade the binding and SQLite before disabling it. Do not
claim hardened `trusted_schema=OFF` support without an effective innocuous
flag.

Reading the argument as an `Option<String>` maps SQL NULL to NULL, which FTS5
accepts as a column value, so nullable document columns work through triggers
and `'rebuild'`. Any other non-text input still follows the binding's normal
error path. The `?` operators propagate argument-conversion,
callback, and registration errors instead of substituting an untransformed
value. Returning `normalize_cjk(&raw).into_owned()` gives SQLite owned text
independent of the callback input lifetime.

Do not copy the transform into consumer code, and do not choose a UDF name
from package semver or deployment configuration. The compiled
`ACTIVE_PROFILE` is the identity shared by the UDF and the query helpers.

### Keep index and query transforms symmetric

Every index write must reach an SQL object that invokes the registered active
UDF, and the FTS5 table must use the exact tokenizer in
`ACTIVE_PROFILE.recommended_tokenizer`. Query text must pass through
`quote_fts5_phrase` or `literal_query` from the same compiled crate profile.
Mixing profiles, bypassing either transform, or registering an alternate
implementation can create silent search misses.

The helpers produce FTS5 MATCH grammar, not SQL escaping. Bind their output as
a SQL value rather than interpolating it into SQL text, and reject an empty
`literal_query` result before executing `MATCH`. Keep raw text in an ordinary
table, and index it with an external-content FTS5 table whose content source
reads that raw text through the UDF. This layout is required: a contentful
FTS5 table that stores UDF output is unsupported. For the default profile:

```sql
-- Raw source text; this is what a later profile change reindexes from.
CREATE TABLE notes(id INTEGER PRIMARY KEY, body TEXT);

-- Content source: applies the UDF to raw text for 'rebuild' and column reads.
CREATE VIEW notes_fts_source(id, body) AS
    SELECT id, fts5_cjk_v2(body) FROM notes;

CREATE VIRTUAL TABLE notes_fts USING fts5(
    body,
    content = 'notes_fts_source',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Index maintenance: every path feeds UDF-normalized text.
CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, body) VALUES (new.id, fts5_cjk_v2(new.body));
END;
CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, body)
        VALUES ('delete', old.id, fts5_cjk_v2(old.body));
END;
CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, body)
        VALUES ('delete', old.id, fts5_cjk_v2(old.body));
    INSERT INTO notes_fts(rowid, body) VALUES (new.id, fts5_cjk_v2(new.body));
END;

-- Index write: ?1 is the row id, ?2 is the raw document text.
-- Use UPSERT, never REPLACE conflict resolution: REPLACE removes the old row
-- without firing notes_ad and leaves stale tokens in notes_fts.
INSERT INTO notes(id, body) VALUES (?1, ?2)
    ON CONFLICT(id) DO UPDATE SET body = excluded.body;

-- Query: ?1 is the bound, non-empty output of literal_query.
SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?1;
```

Under the `nfc` profile the UDF is named `fts5_cjk_v2_nfc`, so code should
build SQL from `ACTIVE_PROFILE.udf_name` instead of hardcoding either name.
Every path that feeds the index, including triggers, `'delete'` commands, and
the content source read by `'rebuild'`, must supply UDF-normalized text, and
`'rebuild'` must read raw source text through the UDF. A contentful FTS5 table
that stores UDF output is unsupported: `'rebuild'` would re-tokenize text that
is already normalized under the old profile, so the rebuild transaction below
cannot reindex it.

Never write the source table with `REPLACE` conflict resolution. This covers
`INSERT OR REPLACE`, `REPLACE INTO`, `UPDATE OR REPLACE`, and
`ON CONFLICT REPLACE` constraints. With SQLite's default
`recursive_triggers = OFF`, REPLACE deletes the conflicting row without firing
its DELETE trigger, so its tokens stay in the index and FTS5
`'integrity-check'` fails. Write with UPSERT
(`INSERT ... ON CONFLICT DO UPDATE`) instead; its update fires the UPDATE
trigger.

### Explicit rebuild transaction

Only this explicit write path may certify index bytes with compatibility
metadata. Translate the generic object names below to consumer-owned schema
objects while preserving the order:

1. Open the connection and register the compiled UDF under
   `ACTIVE_PROFILE.udf_name` before preparing any object that invokes it.
2. Begin a write transaction.
3. Create or replace the normalized objects and the external-content FTS
   table. Configure the table with the exact byte value of
   `ACTIVE_PROFILE.recommended_tokenizer`.
4. Run the FTS5 external-content rebuild command for the consumer's table,
   schematically `INSERT INTO <fts-table>(<fts-table>) VALUES('rebuild')`.
   Its content source must read raw source text through the UDF; a contentful
   FTS5 table that stores UDF output is unsupported.
5. Query the rebuilt index with both a known positive probe and a known
   adjacency-negative probe. The latter must prove that separated or reversed
   CJK scalars do not satisfy the adjacent phrase.
6. Only after both probes pass, stamp the complete `ACTIVE_PROFILE` contract
   and the consumer schema identity: `id`, `algorithm_version`, `nfc`,
   `udf_name`, `recommended_tokenizer`, and the consumer's exact schema
   revision or equivalent identity.
7. Commit the transaction.

Roll back the whole transaction on any registration, object creation, rebuild,
probe, metadata, or commit error. In particular, never leave a new stamp that
certifies old or partially rebuilt index bytes.

### Read-only boot check

Normal boot validates the persisted contract; it does not repair or recertify
it:

1. Open the connection and register the compiled active-profile UDF.
2. Read the persisted profile metadata and consumer schema identity without
   writing either one.
3. Compare every observed field exactly with the compiled `ACTIVE_PROFILE` and
   the expected consumer schema identity. Missing metadata is a mismatch, not
   an uninitialized state that boot may fill.
4. If every identity field matches, run the same positive and
   adjacency-negative probes through the existing index using read-only SQL.
5. Serve search only when both the exact comparison and both probes succeed.
   Otherwise refuse before serving search.

A refusal diagnostic must distinguish the observed and expected profile ID,
algorithm version, NFC flag, UDF name, tokenizer, and consumer schema
identity. It must not include raw user query text. Probe failure also refuses
service; it does not authorize boot to update metadata. Boot never writes or
restamps the compatibility record. Recovery always returns to the explicit
rebuild transaction above, which writes the stamp only after rebuilding and
probing succeed.

The comparison itself needs only the standard library. The example below
works unchanged in both profiles:

```rust
use fts5_cjk::{ACTIVE_PROFILE, TransformProfile};

/// Owned copy of the profile as a consumer persists it next to the index.
#[derive(Debug, PartialEq, Eq)]
struct StoredProfile {
    id: String,
    algorithm_version: u32,
    nfc: bool,
    udf_name: String,
    recommended_tokenizer: String,
}

impl StoredProfile {
    fn stamp(profile: &TransformProfile) -> Self {
        Self {
            id: profile.id.to_owned(),
            algorithm_version: profile.algorithm_version,
            nfc: profile.nfc,
            udf_name: profile.udf_name.to_owned(),
            recommended_tokenizer: profile.recommended_tokenizer.to_owned(),
        }
    }
}

fn check_boot(stored: Option<&StoredProfile>) -> Result<(), String> {
    let expected = StoredProfile::stamp(&ACTIVE_PROFILE);
    match stored {
        None => Err("missing profile metadata: rebuild required".to_owned()),
        Some(observed) if *observed == expected => Ok(()),
        Some(observed) => Err(format!(
            "profile mismatch: observed {observed:?}, expected {expected:?}; rebuild required"
        )),
    }
}

let stamped = StoredProfile::stamp(&ACTIVE_PROFILE);
assert_eq!(check_boot(Some(&stamped)), Ok(()));
assert!(check_boot(None).is_err());

let other = StoredProfile { nfc: !ACTIVE_PROFILE.nfc, ..StoredProfile::stamp(&ACTIVE_PROFILE) };
assert!(check_boot(Some(&other)).is_err());

if ACTIVE_PROFILE.nfc {
    assert_eq!(ACTIVE_PROFILE.id, "fts5-cjk/v2;unicode61=6.1-default;nfc=on");
    assert_eq!(ACTIVE_PROFILE.udf_name, "fts5_cjk_v2_nfc");
} else {
    assert_eq!(ACTIVE_PROFILE.id, "fts5-cjk/v2;unicode61=6.1-default;nfc=off");
    assert_eq!(ACTIVE_PROFILE.udf_name, "fts5_cjk_v2");
}
```

## Real FTS5 evidence

The repository integration target executes output from both public query
helpers as bound MATCH values in one direct SQLite process and records that
process's version and source identity. Under both the default and `nfc`
profiles, it proves a fixed 10-case recall/precision matrix plus two Hangul
canonical-direction cases: positive mixed-boundary and embedded-quote searches
match, while separated and reversed CJK compounds do not. The two Hangul
directions match under `nfc`; default-profile evidence is deliberately limited
to the observed distinction for the pinned `가`/`가` fixture and makes no
general canonical-equivalence or canonical-inequivalence claim.

## Scope and limits

- The fixed eight ranges above operate on UTF-8 decoded Rust Unicode scalars;
  they are not an automatically expanding Unicode repertoire.
- Search is substring-like only within an ordered, adjacent run of supported
  CJK scalars. It is not morphology, stemming, transliteration, dictionary
  lookup, or a general substring search engine.
- Spacing at mixed CJK/token-character boundaries is intentional and remains
  visible after `denormalize_cjk`. Denormalization removes eligible CJK-CJK
  spaces only and is lossy, display-only output handling.
- The `nfc` feature is default-off and composes before boundary insertion. It
  selects a distinct complete profile, so index and query paths must enable the
  same compiled profile; the default profile makes no general canonical claim.
- Query helpers implement literal MATCH grammar only. An empty
  `literal_query` result is a rejection signal, not an empty search.
- The consumer owns its SQLite binding, UDF registration, connection ordering,
  schema, migrations, ranking, snippets, and runtime profile selection.

The consumer owns binding selection, connection setup, schema, migrations, and
transaction policy; this crate has no SQLite binding dependency.

## Minimum supported Rust version

The declared MSRV is `rust-version = "1.97.1"`. It is the exact toolchain that
`rust-toolchain.toml` pins, and on that toolchain the repository gate builds,
tests, documents, lints, and dry-run-packages both the default and `nfc`
profiles, and asserts that the pin and `rust-version` agree. No lower MSRV is
claimed. The Rust 2024 edition floor is 1.85, but declaring it would require
a separate toolchain matrix and a dependency MSRV proof for both profiles, and
neither exists.

## Running the test suite

`cargo test` needs an `sqlite3` shell with FTS5 and `.parameter` support
(SQLite CLI 3.28.0 or newer) on `PATH`, or the `FTS5_CJK_SQLITE3` environment
variable set to such a shell. The FTS5 contract test and the `unicode61`
oracle fail loudly rather than skip when it is missing. The repository gate
`ci/check.sh` provisions SQLite 3.53.2 in CI.

## License

MIT (see `LICENSE`). `src/unicode61_data.rs` is generated from Unicode
Character Database 6.1.0 data under the Unicode License v3, whose text ships in
the package as `tools/unicode61/LICENSE-UNICODE`.
