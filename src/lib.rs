//! Fixed CJK classification and compile-time transform profile metadata.
//!
//! [`is_cjk`] classifies one Rust Unicode scalar (`char`) using eight fixed,
//! versioned ranges covering Han core, Han Extension A, Han Extension B,
//! Hiragana, Katakana, Hangul Syllables, Hangul Jamo, and Hangul Compatibility
//! Jamo. This is a compatibility policy, not an exhaustive or automatically
//! updated definition of the latest Unicode CJK repertoire.
//!
//! The exact inclusive ranges are `U+4E00..U+9FFF`, `U+3400..U+4DBF`,
//! `U+20000..U+2A6DF`, `U+3040..U+309F`, `U+30A0..U+30FF`,
//! `U+AC00..U+D7A3`, `U+1100..U+11FF`, and `U+3130..U+318F`.
//!
//! [`ACTIVE_PROFILE`] is the complete identity of the contract compiled into
//! this crate. The default build selects the NFC-off profile; the default-off
//! `nfc` Cargo feature selects a distinct NFC-on profile and SQL UDF name.
//! Cargo features are additive and may be unified by another dependency path,
//! so consumers must inspect the compiled profile rather than infer it from
//! deployment configuration.
//!
//! Persist and compare the complete profile: its ID, algorithm version, NFC
//! state, SQL UDF name, and recommended tokenizer. Package semver and
//! [`TRANSFORM_VERSION`] alone are insufficient because the NFC-off and NFC-on
//! profiles share algorithm version 2 but are incompatible identities.
//!
//! ## Transform profile and semver policy
//!
//! Any change to transform output requires a new transform profile and a full
//! reindex, regardless of the semver bump.
//! Crate semver never signals index compatibility.
//!
//! Crate semver describes Rust API compatibility only; every release entry in
//! the changelog states whether any profile changed.
//!
//! | Build | `id` | `algorithm_version` | `nfc` | `udf_name` | `recommended_tokenizer` |
//! |-------|------|---------------------|-------|------------|-------------------------|
//! | default (`nfc` feature off) | `fts5-cjk/v2;unicode61=6.1-default;nfc=off` | `2` | `false` | `fts5_cjk_v2` | `unicode61 remove_diacritics 2` |
//! | `features = ["nfc"]` | `fts5-cjk/v2;unicode61=6.1-default;nfc=on` | `2` | `true` | `fts5_cjk_v2_nfc` | `unicode61 remove_diacritics 2` |
//!
//! docs.rs builds this crate with default features, so the constant values
//! shown on docs.rs, such as [`UDF_NAME`], are the NFC-off profile's; the table
//! above lists both profiles.
//!
//! Every output- or search-compatibility change requires a new profile
//! identity. This includes classifier range changes, boundary-classification
//! changes, NFC ordering, UDF semantics, literal escaping and phrase
//! construction, and the tokenizer contract.
//!
//! If persisted index metadata differs from the compiled [`ACTIVE_PROFILE`],
//! the consumer must refuse startup. Recovery requires an explicit rebuild or
//! reindex with the compiled contract; only after that succeeds may the
//! consumer stamp the new profile metadata. A consumer must never
//! automatically restamp stale metadata, because that would falsely certify
//! old index tokens as compatible.
//!
//! ## Pure transforms
//!
//! [`normalize_cjk`] applies the active profile's CJK token-boundary transform
//! for both index and query input. It inserts one ASCII space between adjacent
//! supported CJK scalars and at mixed boundaries where SQLite `unicode61`
//! would otherwise retain one token. This includes the 25 combining marks
//! consumed inside a token by the pinned `unicode61 remove_diacritics 2`
//! tokenizer; it is not inferred from Unicode general categories alone.
//! Existing punctuation and whitespace boundaries remain unchanged, and
//! unchanged bytes use the `Cow::Borrowed` fast path. With the `nfc` feature,
//! NFC composition happens before spacing.
//!
//! [`denormalize_cjk`] is a lossy, display-only helper. It collapses eligible
//! CJK-CJK spacing but cannot distinguish original spaces or restore source
//! text.
//!
//! ```
//! use fts5_cjk::{denormalize_cjk, normalize_cjk};
//!
//! assert_eq!(normalize_cjk("AI개발중"), "AI 개 발 중");
//! assert_eq!(normalize_cjk("개\u{0301}발"), "개 \u{0301} 발");
//! assert_eq!(normalize_cjk("개\u{0345}발"), "개\u{0345}발");
//! assert_eq!(denormalize_cjk("AI 개 발 중"), "AI 개발중");
//! ```
//!
//! ## Literal query strings
//!
//! [`quote_fts5_phrase`] normalizes one complete raw value, doubles embedded
//! double quotes, and adds exactly one outer quote pair. [`literal_query`]
//! applies `split_whitespace` to raw input first, quotes each original term
//! independently, preserves order, and joins phrases with one ASCII space.
//!
//! ```
//! use fts5_cjk::{literal_query, quote_fts5_phrase};
//!
//! assert_eq!(quote_fts5_phrase("AI개"), "\"AI 개\"");
//! assert_eq!(literal_query("개발 hello"), "\"개 발\" \"hello\"");
//! assert_eq!(quote_fts5_phrase(""), "\"\"");
//! assert_eq!(literal_query(" \t\n"), "");
//! ```
//!
//! Consumers must reject an empty [`literal_query`] result before `MATCH`.
//! These helpers accept literal values only; they do not implement Boolean,
//! column, `NEAR`, prefix, negation, or parenthesized MATCH grammar.
//!
//! ## Consumer integration
//!
//! This crate has no SQLite dependency. The consumer owns its SQLite binding
//! and registers the active transform as a scalar SQL function through it.
//!
//! ### Register the active transform
//!
//! ```rust,ignore
//! // Consumer code, rusqlite 0.40 with the `functions` feature.
//! // fts5-cjk itself has no SQLite dependency.
//! use fts5_cjk::{ACTIVE_PROFILE, normalize_cjk};
//! use rusqlite::{Connection, functions::FunctionFlags};
//!
//! fn register_fts5_cjk(conn: &Connection) -> rusqlite::Result<()> {
//!     conn.create_scalar_function(
//!         ACTIVE_PROFILE.udf_name,
//!         1,
//!         FunctionFlags::SQLITE_UTF8
//!             | FunctionFlags::SQLITE_DETERMINISTIC
//!             | FunctionFlags::SQLITE_INNOCUOUS,
//!         |ctx| {
//!             let raw: Option<String> = ctx.get(0)?;
//!             Ok(raw.map(|raw| normalize_cjk(&raw).into_owned()))
//!         },
//!     )
//! }
//! ```
//!
//! Register exactly one text argument under `ACTIVE_PROFILE.udf_name` with the
//! binding's UTF-8, deterministic, and `SQLITE_INNOCUOUS` flags, so that
//! consumer-owned triggers and views may invoke it under trusted-schema
//! hardening. Never use `SQLITE_DIRECTONLY`: index maintenance invokes the UDF
//! from schema objects. If the binding cannot honor `SQLITE_INNOCUOUS`, keep
//! `trusted_schema` enabled on those connections. SQL NULL maps to NULL, which
//! FTS5 accepts, while any other non-text input errors. Propagate conversion and
//! registration errors, return owned text, never copy the transform, and never
//! derive the UDF name from package semver or deployment configuration.
//!
//! ### Keep index and query symmetric
//!
//! Every index write must reach an SQL object that invokes the registered
//! active UDF, and the FTS5 table must use the exact
//! `ACTIVE_PROFILE.recommended_tokenizer`. It must be an external-content
//! table whose content source applies the UDF to raw source text; a contentful
//! FTS5 table that stores UDF output is unsupported. Triggers, `'delete'`
//! commands, and the content source read by `'rebuild'` must all supply
//! UDF-normalized text. Never write the source table with
//! `REPLACE` conflict resolution (`INSERT OR REPLACE`, `REPLACE INTO`,
//! `UPDATE OR REPLACE`, or `ON CONFLICT REPLACE`): with SQLite's default
//! `recursive_triggers = OFF` it deletes rows without firing DELETE triggers
//! and leaves stale tokens. Write with UPSERT
//! (`INSERT ... ON CONFLICT DO UPDATE`) instead. Query text must pass through
//! [`literal_query`] or [`quote_fts5_phrase`] from the same compiled build.
//! Bind the result as the `MATCH` value; never interpolate it into SQL text.
//! Reject an empty [`literal_query`] result before `MATCH`.
//!
//! ### Rebuild and stamp
//!
//! Only an explicit rebuild transaction may certify index bytes:
//!
//! 1. Register the UDF under `ACTIVE_PROFILE.udf_name` before preparing SQL.
//! 2. Begin a write transaction.
//! 3. Create the FTS table with `ACTIVE_PROFILE.recommended_tokenizer`, as an
//!    external-content table whose content source applies the UDF to raw
//!    source text.
//! 4. Run the FTS5 `'rebuild'` command for the table; it must read raw source
//!    text through the UDF. A contentful FTS5 table that stores UDF output is
//!    unsupported, because `'rebuild'` cannot reindex it.
//! 5. Run a known positive probe and a known adjacency-negative probe.
//! 6. Only after both pass, stamp all five profile fields and the schema identity.
//! 7. Commit; roll back the whole transaction on any error.
//!
//! ### Read-only boot check
//!
//! 1. Register the UDF.
//! 2. Read, and never write, the persisted profile and schema identity.
//! 3. Compare every field exactly; missing metadata is a mismatch.
//! 4. Run the same probes with read-only SQL.
//! 5. Serve only if all checks pass; otherwise refuse before serving.
//!
//! Refusal diagnostics distinguish observed and expected fields and never
//! include raw user query text. Boot never restamps; recovery is always the
//! explicit rebuild above.
//!
//! ```
//! use fts5_cjk::ACTIVE_PROFILE;
//!
//! // Owned copy of the five fields as read back from consumer storage.
//! type Stored = (String, u32, bool, String, String);
//!
//! fn stamp() -> Stored {
//!     let p = ACTIVE_PROFILE;
//!     (p.id.into(), p.algorithm_version, p.nfc, p.udf_name.into(), p.recommended_tokenizer.into())
//! }
//!
//! // `None` is missing metadata: a mismatch, never a state for boot to fill.
//! fn boot_allowed(stored: Option<&Stored>) -> bool {
//!     stored == Some(&stamp())
//! }
//!
//! let stored = stamp();
//! assert!(boot_allowed(Some(&stored)));
//! let flipped = (stored.0.clone(), stored.1, !stored.2, stored.3.clone(), stored.4.clone());
//! assert!(!boot_allowed(Some(&flipped)));
//! assert!(!boot_allowed(None));
//! ```
//!
//! ## Real FTS5 evidence
//!
//! The repository integration target executes both public helper outputs as
//! bound MATCH values in one direct SQLite process and records its version and
//! source identity. Both explicit profiles pass a fixed 10-case
//! recall/precision matrix plus two Hangul canonical-direction cases, including
//! mixed-boundary and embedded-quote positives and separated/reversed
//! adjacency negatives. The Hangul directions match under `nfc`; default
//! evidence is limited to the observed distinction for the pinned `가`/`가`
//! fixture and makes no general canonical-equivalence or
//! canonical-inequivalence claim.
//!
//! ## Scope and limits
//!
//! - The fixed eight ranges above operate on UTF-8 decoded Rust Unicode
//!   scalars; they are not an automatically expanding Unicode repertoire.
//! - Search is substring-like only within an ordered, adjacent run of supported
//!   CJK scalars. It is not morphology, stemming, transliteration, dictionary
//!   lookup, or a general substring search engine.
//! - Spacing at mixed CJK/token-character boundaries is intentional and
//!   remains visible after [`denormalize_cjk`]. Denormalization removes
//!   eligible CJK-CJK spaces only and is lossy, display-only output handling.
//! - The `nfc` feature is default-off and composes before spacing. It selects a
//!   distinct complete profile, so index and query paths must use the same
//!   compiled profile; default makes no general canonical claim.
//! - The helpers implement literal MATCH grammar only. An empty
//!   [`literal_query`] result is a rejection signal, not an empty search.
//! - The consumer owns its SQLite binding, UDF registration, connection
//!   ordering, schema, migrations, ranking, snippets, and runtime profile
//!   selection.
//!
//! The README repeats this contract with a full persistence example. This
//! crate implements no consumer schema or migration.

#![forbid(unsafe_code)]

mod classification;
mod literal;
mod profile;
mod transform;

pub use classification::is_cjk;
pub use literal::{literal_query, quote_fts5_phrase};
pub use profile::{ACTIVE_PROFILE, TRANSFORM_VERSION, TransformProfile, UDF_NAME};
pub use transform::{denormalize_cjk, normalize_cjk};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
