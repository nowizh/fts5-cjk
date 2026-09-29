use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};

use super::unicode61_data::{UNICODE61_REMOVED_DIACRITICS, UNICODE61_TOKEN_RANGES};
use super::{is_unicode61_removed_diacritic, is_unicode61_token_char, needs_boundary};

const EXPECTED_RANGE_COUNT: usize = 403;
const EXPECTED_TOKEN_SCALAR_COUNT: u32 = 1_104_042;
const EXPECTED_EDGE_CASE_COUNT: usize = 1_449;
const EXPECTED_MATRIX_CASE_COUNT: usize = 1_460;
const EXPECTED_REMOVED_DIACRITIC_COUNT: usize = 25;
const EXPECTED_DIACRITIC_TRANSITION_COUNT: usize = 50;
const EXPECTED_TOTAL_OBSERVATION_COUNT: usize = 1_510;

#[derive(Debug)]
struct OracleCase {
    cp: u32,
    expected_token: bool,
    label: String,
}

#[derive(Debug)]
struct OracleResult {
    sqlite_version: String,
    sqlite_source_id: String,
    observed_token: BTreeMap<u32, bool>,
    observed_transition: BTreeMap<usize, bool>,
}

#[derive(Clone, Copy, Debug)]
enum TransitionDirection {
    CjkLeft = 0,
    CjkRight = 1,
}

#[derive(Debug)]
struct DiacriticTransition {
    cp: u32,
    direction: TransitionDirection,
    expected_boundary: bool,
}

fn valid_rust_scalar(cp: u32) -> bool {
    char::from_u32(cp).is_some()
}

fn representative_cases() -> &'static [(u32, &'static str)] {
    &[
        (0x0000, "NUL separator"),
        (0x0020, "ASCII whitespace separator"),
        (0x0021, "ASCII punctuation separator"),
        (0x0035, "ASCII number token"),
        (0x0041, "ASCII letter token"),
        (0x005F, "underscore separator"),
        (0x0345, "combining-mark separator"),
        (0x0378, "Unicode-6.1-unassigned category-0 token"),
        (0x4E00, "BMP CJK token"),
        (0xE000, "private-use token"),
        (0xFFFE, "UTF-8 replacement separator U+FFFE"),
        (0xFFFF, "UTF-8 replacement separator U+FFFF"),
        (0x10400, "astral letter token"),
        (0x20000, "astral CJK token"),
        (0x1F600, "astral symbol separator"),
    ]
}

fn matrix_cases() -> Vec<OracleCase> {
    let mut cases = BTreeMap::<u32, String>::new();

    for &(start, end) in UNICODE61_TOKEN_RANGES {
        for (cp, label) in [
            (start.checked_sub(1), "below range"),
            (Some(start), "range start"),
            (Some(end), "range end"),
            (end.checked_add(1), "above range"),
        ] {
            if let Some(cp) = cp
                && valid_rust_scalar(cp)
            {
                cases.entry(cp).or_insert_with(|| label.to_owned());
            }
        }
    }

    assert_eq!(
        cases.len(),
        EXPECTED_EDGE_CASE_COUNT,
        "unicode61 edge matrix size drifted"
    );

    for &(cp, label) in representative_cases() {
        cases.entry(cp).or_insert_with(|| label.to_owned());
    }

    assert_eq!(
        cases.len(),
        EXPECTED_MATRIX_CASE_COUNT,
        "unicode61 full oracle matrix size drifted"
    );

    cases
        .into_iter()
        .map(|(cp, label)| OracleCase {
            cp,
            expected_token: is_unicode61_token_char(
                char::from_u32(cp).expect("matrix contains only Rust scalars"),
            ),
            label,
        })
        .collect()
}

fn diacritic_transitions() -> Vec<DiacriticTransition> {
    assert_eq!(
        UNICODE61_REMOVED_DIACRITICS.len(),
        EXPECTED_REMOVED_DIACRITIC_COUNT,
        "frozen removed-diacritic count drifted"
    );
    assert!(
        UNICODE61_REMOVED_DIACRITICS
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "frozen removed diacritics must be sorted and unique"
    );

    let mut transitions = Vec::with_capacity(EXPECTED_DIACRITIC_TRANSITION_COUNT);
    for &cp in UNICODE61_REMOVED_DIACRITICS {
        let diacritic = char::from_u32(cp).expect("frozen diacritics are Rust scalars");
        assert!(!is_unicode61_token_char(diacritic));
        assert!(is_unicode61_removed_diacritic(diacritic));
        transitions.push(DiacriticTransition {
            cp,
            direction: TransitionDirection::CjkLeft,
            expected_boundary: needs_boundary('개', diacritic),
        });
        transitions.push(DiacriticTransition {
            cp,
            direction: TransitionDirection::CjkRight,
            expected_boundary: needs_boundary(diacritic, '발'),
        });
    }
    assert_eq!(
        transitions.len(),
        EXPECTED_DIACRITIC_TRANSITION_COUNT,
        "removed-diacritic transition matrix size drifted"
    );
    transitions
}

fn sqlite3_program() -> OsString {
    std::env::var_os("FTS5_CJK_SQLITE3")
        .filter(|program| !program.is_empty())
        .unwrap_or_else(|| OsString::from("sqlite3"))
}

fn run_sqlite_oracle(cases: &[OracleCase], transitions: &[DiacriticTransition]) -> OracleResult {
    assert!(!cases.is_empty(), "oracle matrix must not be empty");
    assert!(
        !transitions.is_empty(),
        "diacritic transition matrix must not be empty"
    );

    let mut sql = String::from(
        "CREATE VIRTUAL TABLE ft USING fts5(x, tokenize='unicode61 remove_diacritics 2');\n\
         CREATE VIRTUAL TABLE vocab USING fts5vocab(ft, instance);\n\
         CREATE TABLE cases(id INTEGER PRIMARY KEY, cp INTEGER NOT NULL, expected INTEGER NOT NULL);\n\
         CREATE TABLE transitions(id INTEGER PRIMARY KEY, cp INTEGER NOT NULL, direction INTEGER NOT NULL, expected INTEGER NOT NULL);\n\
         SELECT printf('IDENTITY|%s|%s', sqlite_version(), sqlite_source_id());\n",
    );
    for (id, case) in cases.iter().enumerate() {
        let expected = u8::from(case.expected_token);
        sql.push_str(&format!(
            "INSERT INTO cases(id, cp, expected) VALUES ({id}, {}, {expected});\n",
            case.cp
        ));
    }
    for (id, transition) in transitions.iter().enumerate() {
        let expected = u8::from(transition.expected_boundary);
        sql.push_str(&format!(
            "INSERT INTO transitions(id, cp, direction, expected) VALUES ({id}, {}, {}, {expected});\n",
            transition.cp, transition.direction as u8
        ));
    }
    sql.push_str(
        "INSERT INTO ft(rowid, x) SELECT id, 'a' || char(cp) || 'b' FROM cases;\n\
         SELECT printf('CASE|%d|%d|%d', c.id, c.cp, count(v.term))\n\
         FROM cases AS c LEFT JOIN vocab AS v ON v.doc = c.id\n\
         GROUP BY c.id, c.cp ORDER BY c.id;\n\
         INSERT INTO ft(rowid, x)\n\
         SELECT 100000 + id,\n\
                CASE direction\n\
                  WHEN 0 THEN char(44060) || char(cp) || 'a'\n\
                  ELSE 'a' || char(cp) || char(48156)\n\
                END\n\
         FROM transitions;\n\
         SELECT printf('TRANSITION|%d|%d|%d|%d', t.id, t.cp, t.direction, count(v.term))\n\
         FROM transitions AS t LEFT JOIN vocab AS v ON v.doc = 100000 + t.id\n\
         GROUP BY t.id, t.cp, t.direction ORDER BY t.id;\n",
    );

    let mut child = Command::new(sqlite3_program())
        .args(["-batch", "-bail", ":memory:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!(
                "required FTS5-capable sqlite3 is unavailable; install/configure sqlite3: {error}"
            )
        });

    child
        .stdin
        .as_mut()
        .expect("sqlite3 stdin is piped")
        .write_all(sql.as_bytes())
        .expect("write fixed numeric classifier script to sqlite3");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for sqlite3 oracle");
    let stdout = String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("sqlite3 oracle stdout is not UTF-8: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "sqlite3 classifier oracle failed (status {}): {stderr}\nstdout:\n{stdout}",
        output.status
    );

    let mut sqlite_version = None;
    let mut sqlite_source_id = None;
    let mut observed_token = BTreeMap::new();
    let mut observed_transition = BTreeMap::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        match fields.as_slice() {
            ["IDENTITY", version, source_id] => {
                assert!(sqlite_version.is_none(), "duplicate SQLite identity row");
                sqlite_version = Some((*version).to_owned());
                sqlite_source_id = Some((*source_id).to_owned());
            }
            ["CASE", id, cp, token_count] => {
                let id = id
                    .parse::<usize>()
                    .unwrap_or_else(|error| panic!("malformed oracle case id {id:?}: {error}"));
                let cp = cp
                    .parse::<u32>()
                    .unwrap_or_else(|error| panic!("malformed oracle codepoint {cp:?}: {error}"));
                let token_count = token_count.parse::<u8>().unwrap_or_else(|error| {
                    panic!("malformed oracle token count {token_count:?}: {error}")
                });
                let expected_case = cases
                    .get(id)
                    .unwrap_or_else(|| panic!("sqlite3 returned unknown case id {id}"));
                assert_eq!(
                    cp, expected_case.cp,
                    "sqlite3 returned wrong codepoint for case id {id}"
                );
                let observed = match token_count {
                    1 => true,
                    2 => false,
                    other => panic!(
                        "unexpected token count {other} for U+{cp:04X} ({})",
                        expected_case.label
                    ),
                };
                assert!(
                    observed_token.insert(cp, observed).is_none(),
                    "duplicate oracle result for U+{cp:04X}"
                );
            }
            ["TRANSITION", id, cp, direction, token_count] => {
                let id = id.parse::<usize>().unwrap_or_else(|error| {
                    panic!("malformed diacritic transition id {id:?}: {error}")
                });
                let cp = cp.parse::<u32>().unwrap_or_else(|error| {
                    panic!("malformed diacritic transition codepoint {cp:?}: {error}")
                });
                let direction = direction.parse::<u8>().unwrap_or_else(|error| {
                    panic!("malformed diacritic transition direction {direction:?}: {error}")
                });
                let token_count = token_count.parse::<u8>().unwrap_or_else(|error| {
                    panic!("malformed diacritic transition token count {token_count:?}: {error}")
                });
                let expected_transition = transitions.get(id).unwrap_or_else(|| {
                    panic!("sqlite3 returned unknown diacritic transition id {id}")
                });
                assert_eq!(cp, expected_transition.cp);
                assert_eq!(direction, expected_transition.direction as u8);
                let observed = match token_count {
                    1 => true,
                    2 => false,
                    other => panic!(
                        "unexpected token count {other} for diacritic transition U+{cp:04X} direction {direction}"
                    ),
                };
                assert!(
                    observed_transition.insert(id, observed).is_none(),
                    "duplicate diacritic transition result id {id}"
                );
            }
            _ => panic!("malformed sqlite3 oracle output line: {line:?}"),
        }
    }

    OracleResult {
        sqlite_version: sqlite_version.expect("sqlite3 output omitted sqlite_version()"),
        sqlite_source_id: sqlite_source_id.expect("sqlite3 output omitted sqlite_source_id()"),
        observed_token,
        observed_transition,
    }
}

#[test]
fn effective_unicode61_classifier_matches_sqlite() {
    assert_eq!(
        UNICODE61_TOKEN_RANGES.len(),
        EXPECTED_RANGE_COUNT,
        "generated range count drifted"
    );
    assert!(
        UNICODE61_TOKEN_RANGES
            .windows(2)
            .all(|pair| pair[0].0 <= pair[0].1 && pair[0].1 < pair[1].0),
        "generated ranges must be sorted, inclusive, and non-overlapping"
    );
    let token_scalar_count: u32 = UNICODE61_TOKEN_RANGES
        .iter()
        .map(|&(start, end)| end - start + 1)
        .sum();
    assert_eq!(token_scalar_count, EXPECTED_TOKEN_SCALAR_COUNT);

    assert!(is_unicode61_token_char('\u{0378}'));
    for separator in ['\0', '\u{FFFE}', '\u{FFFF}'] {
        assert!(!is_unicode61_token_char(separator));
    }
    assert!(needs_boundary('A', '개'));
    assert!(needs_boundary('개', 'A'));
    assert!(needs_boundary('1', '개'));
    assert!(needs_boundary('개', '1'));
    assert!(!needs_boundary('!', '개'));
    assert!(!needs_boundary('개', '_'));
    assert!(needs_boundary('개', '\u{0301}'));
    assert!(needs_boundary('\u{0301}', '발'));
    assert!(!needs_boundary('개', '\u{0345}'));
    assert!(!needs_boundary('\u{0345}', '발'));

    let cases = matrix_cases();
    let transitions = diacritic_transitions();
    assert_eq!(
        cases.len() + transitions.len(),
        EXPECTED_TOTAL_OBSERVATION_COUNT,
        "total SQLite oracle observation count drifted"
    );
    let result = run_sqlite_oracle(&cases, &transitions);
    assert_eq!(
        result.observed_token.len(),
        cases.len(),
        "sqlite3 oracle omitted result rows"
    );
    assert_eq!(
        result.observed_transition.len(),
        transitions.len(),
        "sqlite3 oracle omitted diacritic transition rows"
    );

    let mut mismatches = Vec::new();
    for case in &cases {
        let observed = result
            .observed_token
            .get(&case.cp)
            .unwrap_or_else(|| panic!("missing result for U+{:04X}", case.cp));
        if *observed != case.expected_token {
            mismatches.push(format!(
                "U+{:04X} {}: Rust expected {}, SQLite observed {}",
                case.cp, case.label, case.expected_token, observed
            ));
        }
    }
    for (id, transition) in transitions.iter().enumerate() {
        let observed = result
            .observed_transition
            .get(&id)
            .unwrap_or_else(|| panic!("missing diacritic transition result id {id}"));
        if *observed != transition.expected_boundary {
            mismatches.push(format!(
                "U+{:04X} {:?}: Rust expected boundary {}, SQLite observed continuation {}",
                transition.cp, transition.direction, transition.expected_boundary, observed
            ));
        }
    }

    assert!(
        mismatches.is_empty(),
        "unicode61 classifier mismatch against SQLite {} ({}):\n{}",
        result.sqlite_version,
        result.sqlite_source_id,
        mismatches.join("\n")
    );
    println!(
        "SQLite {} ({}) — compared {} unicode61 classifier cases and {} removed-diacritic transitions ({} observations), zero mismatches",
        result.sqlite_version,
        result.sqlite_source_id,
        cases.len(),
        transitions.len(),
        cases.len() + transitions.len()
    );
}
