use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

use fts5_cjk::{ACTIVE_PROFILE, literal_query, normalize_cjk, quote_fts5_phrase};

#[derive(Debug)]
struct ContractCase {
    id: &'static str,
    indexed_text: String,
    match_operand: String,
    expected: bool,
}

fn sqlite3_program() -> OsString {
    std::env::var_os("FTS5_CJK_SQLITE3")
        .filter(|program| !program.is_empty())
        .unwrap_or_else(|| OsString::from("sqlite3"))
}

fn checked_literal(raw: &str) -> Result<String, &'static str> {
    let query = literal_query(raw);
    if query.is_empty() {
        Err("empty literal query")
    } else {
        Ok(query)
    }
}

fn text_hex(value: &str) -> String {
    let mut hex = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        write!(&mut hex, "{byte:02X}").expect("writing to String cannot fail");
    }
    hex
}

fn sql_text(value: &str) -> String {
    format!("CAST(X'{}' AS TEXT)", text_hex(value))
}

fn parameter_set(name: &str, value: &str) -> String {
    format!(
        ".parameter set {name} \"CAST(X'{}' AS TEXT)\"",
        text_hex(value)
    )
}

fn run_contract(program: OsString, cases: &[ContractCase]) {
    assert!(!cases.is_empty(), "FTS5 contract matrix must not be empty");
    assert_eq!(
        ACTIVE_PROFILE.recommended_tokenizer, "unicode61 remove_diacritics 2",
        "the FTS5 contract table must use the active profile tokenizer"
    );

    let mut expected = BTreeMap::new();
    let mut sql = String::from(
        ".parameter init\n\
         CREATE VIRTUAL TABLE ft USING fts5(x, tokenize='unicode61 remove_diacritics 2');\n\
         SELECT printf('IDENTITY|%s|%s', sqlite_version(), sqlite_source_id());\n",
    );
    for (offset, case) in cases.iter().enumerate() {
        assert!(
            expected.insert(case.id, case.expected).is_none(),
            "duplicate fixed FTS5 contract case id {:?}",
            case.id
        );
        let rowid = offset + 1;
        writeln!(
            &mut sql,
            "INSERT INTO ft(rowid, x) VALUES ({rowid}, {});",
            sql_text(&case.indexed_text)
        )
        .expect("writing to String cannot fail");
        let query_parameter = format!("@query_{offset:02}");
        writeln!(
            &mut sql,
            "{}",
            parameter_set(&query_parameter, &case.match_operand)
        )
        .expect("writing to String cannot fail");
        writeln!(
            &mut sql,
            "SELECT 'CASE|' || {} || '|' || typeof({query_parameter}) || '|' || CASE WHEN EXISTS(SELECT 1 FROM ft WHERE rowid = {rowid} AND ft MATCH {query_parameter}) THEN '1' ELSE '0' END;",
            sql_text(case.id),
        )
        .expect("writing to String cannot fail");
    }

    let mut child = Command::new(program)
        .args(["-batch", "-bail", ":memory:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!(
                "required FTS5-capable sqlite3 is unavailable; set FTS5_CJK_SQLITE3 or install sqlite3 on PATH: {error}"
            )
        });

    child
        .stdin
        .as_mut()
        .expect("sqlite3 stdin is piped")
        .write_all(sql.as_bytes())
        .expect("write fixed FTS5 contract script to sqlite3");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for sqlite3 contract");
    let stdout = String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("sqlite3 contract stdout is not UTF-8: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "sqlite3 FTS5 contract failed (status {}): {stderr}\nstdout:\n{stdout}",
        output.status
    );

    let mut identity = None;
    let mut observed = BTreeMap::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        match fields.as_slice() {
            ["IDENTITY", version, source_id] => {
                assert!(identity.is_none(), "duplicate SQLite identity row");
                assert!(!version.is_empty(), "empty sqlite_version() identity field");
                assert!(
                    !source_id.is_empty(),
                    "empty sqlite_source_id() identity field"
                );
                identity = Some(((*version).to_owned(), (*source_id).to_owned()));
            }
            ["CASE", id, "text", value] => {
                assert!(
                    expected.contains_key(id),
                    "sqlite3 returned unknown fixed case id {id:?}"
                );
                let value = match *value {
                    "0" => false,
                    "1" => true,
                    other => panic!("unexpected Boolean {other:?} for fixed case id {id:?}"),
                };
                assert!(
                    observed.insert((*id).to_owned(), value).is_none(),
                    "duplicate sqlite3 result for fixed case id {id:?}"
                );
            }
            ["CASE", id, storage_class, _] => {
                panic!(
                    "fixed case id {id:?} MATCH parameter has storage class {storage_class:?}, expected text"
                )
            }
            _ => panic!("malformed sqlite3 contract output line: {line:?}"),
        }
    }

    let (sqlite_version, sqlite_source_id) =
        identity.expect("sqlite3 contract output omitted version/source identity");
    assert_eq!(
        observed.len(),
        expected.len(),
        "sqlite3 contract omitted CASE rows under SQLite {sqlite_version} ({sqlite_source_id})"
    );
    for (id, expected_value) in expected {
        assert_eq!(
            observed.get(id),
            Some(&expected_value),
            "fixed case id {id:?} failed under SQLite {sqlite_version} ({sqlite_source_id})"
        );
    }

    println!(
        "SQLite {sqlite_version} ({sqlite_source_id}) — executed {} FTS5 contract cases for {}",
        observed.len(),
        ACTIVE_PROFILE.id
    );
}

#[test]
fn real_fts5_contract_matches_public_helpers() {
    assert_eq!(
        checked_literal(" \t\n"),
        Err("empty literal query"),
        "whitespace-only literal input must be rejected before case construction"
    );
    assert_eq!(
        quote_fts5_phrase(""),
        "\"\"",
        "empty phrase formatting remains a distinct public contract"
    );
    assert_eq!(
        ACTIVE_PROFILE.recommended_tokenizer,
        "unicode61 remove_diacritics 2"
    );

    let cases = [
        ContractCase {
            id: "korean-particle",
            indexed_text: normalize_cjk("결제를 완료했다").into_owned(),
            match_operand: quote_fts5_phrase("결제"),
            expected: true,
        },
        ContractCase {
            id: "longer-compound-suffix",
            indexed_text: normalize_cjk("플랫폼개발중").into_owned(),
            match_operand: quote_fts5_phrase("플랫폼"),
            expected: true,
        },
        ContractCase {
            id: "latin-cjk-boundary",
            indexed_text: normalize_cjk("AI개발중").into_owned(),
            match_operand: quote_fts5_phrase("개발"),
            expected: true,
        },
        ContractCase {
            id: "digit-cjk-boundary",
            indexed_text: normalize_cjk("2단계개발").into_owned(),
            match_operand: quote_fts5_phrase("단계"),
            expected: true,
        },
        ContractCase {
            id: "punctuation",
            indexed_text: normalize_cjk("개발,진행").into_owned(),
            match_operand: quote_fts5_phrase("개발,진행"),
            expected: true,
        },
        ContractCase {
            id: "embedded-quote",
            indexed_text: normalize_cjk("인용\"개발").into_owned(),
            match_operand: quote_fts5_phrase("인용\"개발"),
            expected: true,
        },
        ContractCase {
            id: "removed-diacritic-continuation",
            indexed_text: normalize_cjk("개\u{0301}발을").into_owned(),
            match_operand: quote_fts5_phrase("개\u{0301}발"),
            expected: true,
        },
        ContractCase {
            id: "implicit-and",
            indexed_text: normalize_cjk("결제를 완료했다").into_owned(),
            match_operand: checked_literal("결제 완료").expect("non-empty literal query"),
            expected: true,
        },
        ContractCase {
            id: "separated-compound",
            indexed_text: normalize_cjk("개 X 발").into_owned(),
            match_operand: quote_fts5_phrase("개발"),
            expected: false,
        },
        ContractCase {
            id: "reversed-compound",
            indexed_text: normalize_cjk("발개").into_owned(),
            match_operand: quote_fts5_phrase("개발"),
            expected: false,
        },
        // The default false expectation is limited to this exact Hangul pair;
        // it is not a general canonical-inequivalence claim.
        ContractCase {
            id: "hangul-nfd-index-nfc-query",
            indexed_text: normalize_cjk("\u{1100}\u{1161}").into_owned(),
            match_operand: quote_fts5_phrase("가"),
            expected: ACTIVE_PROFILE.nfc,
        },
        ContractCase {
            id: "hangul-nfc-index-nfd-query",
            indexed_text: normalize_cjk("가").into_owned(),
            match_operand: quote_fts5_phrase("\u{1100}\u{1161}"),
            expected: ACTIVE_PROFILE.nfc,
        },
    ];

    assert_eq!(
        cases.len(),
        12,
        "final FTS5 contract matrix must stay exact"
    );

    run_contract(sqlite3_program(), &cases);
}
