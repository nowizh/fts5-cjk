use std::borrow::Cow;

use proptest::prelude::*;

#[path = "support/legacy_v1.rs"]
mod legacy_v1;

const FTS5_OPERATOR_SYNTAX_CORPUS: &[&str] = &[
    "subject:project",
    "{subject body}:project",
    "NEAR(alpha beta, 3)",
    "NEAR(alpha beta)",
    "term*",
    "NOT term",
    "-term",
    "\"exact phrase\"",
    "\"he said \\\"hi\\\"\"",
];

fn literal_is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{3040}'..='\u{309F}'
        | '\u{30A0}'..='\u{30FF}'
        | '\u{AC00}'..='\u{D7A3}'
        | '\u{1100}'..='\u{11FF}'
        | '\u{3130}'..='\u{318F}'
    )
}

fn legacy_reference(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous = None;

    for current in input.chars() {
        if previous.is_some_and(literal_is_cjk) && literal_is_cjk(current) {
            output.push(' ');
        }
        output.push(current);
        previous = Some(current);
    }

    output
}

fn any_rust_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::char::any(), 0..257)
        .prop_map(|chars| chars.into_iter().collect())
}

#[test]
fn hangul_run_is_spaced_char_by_char() {
    assert_eq!(legacy_v1::normalize_cjk("프로젝트"), "프 로 젝 트");
}

#[test]
fn non_cjk_boundary_gets_no_space() {
    assert_eq!(legacy_v1::normalize_cjk("abc한글def"), "abc한 글def");
}

#[test]
fn ascii_only_input_is_borrowed_and_byte_identical() {
    let input = "subject:project";
    let result = legacy_v1::normalize_cjk(input);
    assert_eq!(result, input);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_and_single_char_are_borrowed_unchanged() {
    let empty = legacy_v1::normalize_cjk("");
    assert_eq!(empty, "");
    assert!(matches!(empty, Cow::Borrowed(_)));

    let single = legacy_v1::normalize_cjk("한");
    assert_eq!(single, "한");
    assert!(matches!(single, Cow::Borrowed(_)));

    for input in ["\0", "\u{1}", "\u{1F600}", "A\u{20000}B"] {
        let result = legacy_v1::normalize_cjk(input);
        assert_eq!(result, legacy_reference(input), "{input:?}");
        assert!(matches!(result, Cow::Borrowed(_)), "{input:?}");
    }
}

#[test]
fn each_named_script_is_classified_cjk() {
    let cases = [
        ("Han core", '\u{4E2D}', '\u{6587}'),
        ("Han Extension A", '\u{3400}', '\u{3401}'),
        ("Han Extension B", '\u{20000}', '\u{20001}'),
        ("Hiragana", '\u{3042}', '\u{3044}'),
        ("Katakana", '\u{30A2}', '\u{30A4}'),
        ("Hangul Syllables", '\u{AC00}', '\u{B098}'),
        ("Hangul Jamo", '\u{1100}', '\u{1101}'),
        ("Hangul Compatibility Jamo", '\u{3131}', '\u{3132}'),
    ];

    for (name, first, second) in cases {
        let input: String = [first, second].into_iter().collect();
        let expected: String = [first, ' ', second].into_iter().collect();
        assert_eq!(legacy_v1::normalize_cjk(&input), expected, "{name}");
    }
}

#[test]
fn range_boundary_edges_are_classified_cjk() {
    let edges = [
        ("Han core lower", '\u{4E00}'),
        ("Han core upper", '\u{9FFF}'),
        ("Han Extension A lower", '\u{3400}'),
        ("Han Extension A upper", '\u{4DBF}'),
        ("Han Extension B lower", '\u{20000}'),
        ("Han Extension B upper", '\u{2A6DF}'),
        ("Hiragana lower", '\u{3040}'),
        ("Hiragana upper", '\u{309F}'),
        ("Katakana lower", '\u{30A0}'),
        ("Katakana upper", '\u{30FF}'),
        ("Hangul Syllables lower", '\u{AC00}'),
        ("Hangul Syllables upper", '\u{D7A3}'),
        ("Hangul Jamo lower", '\u{1100}'),
        ("Hangul Jamo upper", '\u{11FF}'),
        ("Hangul Compatibility Jamo lower", '\u{3130}'),
        ("Hangul Compatibility Jamo upper", '\u{318F}'),
    ];

    for (name, c) in edges {
        assert!(legacy_v1::is_cjk(c), "{name}");
    }
}

#[test]
fn range_boundary_neighbours_are_not_classified_cjk() {
    let neighbours = [
        ("below Han core", '\u{4DFF}'),
        ("above Han core", '\u{A000}'),
        ("below Han Extension A", '\u{33FF}'),
        ("above Han Extension A", '\u{4DC0}'),
        ("below Han Extension B", '\u{1FFFF}'),
        ("above Han Extension B", '\u{2A6E0}'),
        ("below Hiragana", '\u{303F}'),
        ("above Katakana", '\u{3100}'),
        ("below Hangul Syllables", '\u{ABFF}'),
        ("above Hangul Syllables", '\u{D7A4}'),
        ("below Hangul Jamo", '\u{10FF}'),
        ("above Hangul Jamo", '\u{1200}'),
        ("below Hangul Compatibility Jamo", '\u{312F}'),
        ("above Hangul Compatibility Jamo", '\u{3190}'),
    ];

    for (name, c) in neighbours {
        assert!(!legacy_v1::is_cjk(c), "{name}");
    }
}

#[test]
fn already_spaced_hangul_is_unchanged() {
    let input = "프 로 젝 트";
    let result = legacy_v1::normalize_cjk(input);
    assert_eq!(result, input);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn denormalize_cjk_collapses_only_cjk_bounded_single_spaces() {
    assert_eq!(legacy_v1::denormalize_cjk("프 로 젝 트"), "프로젝트");

    for input in ["abc 한", "한 abc", "가  나", " 가나", "가나 "] {
        assert_eq!(legacy_v1::denormalize_cjk(input), input);
    }
}

#[test]
fn denormalize_cjk_is_byte_identical_on_ascii() {
    assert_eq!(
        legacy_v1::denormalize_cjk("subject:project 한 글 unrelated"),
        "subject:project 한글 unrelated"
    );

    let input = "subject:project with several internal single spaces";
    let result = legacy_v1::denormalize_cjk(input);
    assert_eq!(result, input);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn decomposed_nfd_jamo_stays_idempotent() {
    let nfd = "\u{1100}\u{1161}";
    let once = legacy_v1::normalize_cjk(nfd);
    assert_eq!(once, "\u{1100} \u{1161}");
    assert_eq!(legacy_v1::normalize_cjk(&once), once);
}

#[test]
fn fts5_operator_syntax_is_byte_identical_and_borrowed() {
    for input in FTS5_OPERATOR_SYNTAX_CORPUS {
        let result = legacy_v1::normalize_cjk(input);
        assert_eq!(result, *input, "{input:?}");
        assert!(matches!(result, Cow::Borrowed(_)), "{input:?}");
    }
}

#[test]
fn cjk02_sole_definition_seam() {
    assert_eq!(legacy_v1::normalize_cjk(""), "");
    let _ = legacy_v1::normalize_cjk("placeholder");
}

/// SHA-256 of the frozen v1 reference code, excluding its leading `//!`
/// description. Editing that code moves the baseline these contracts compare
/// against, so this digest changes only together with a deliberate v1 update.
/// CRLF line endings are normalized to LF first, so a `core.autocrlf` checkout
/// hashes the same bytes. Reproduce with:
/// `sed 's/\r$//' tests/support/legacy_v1.rs | awk 'f || (!/^\/\/!/ && !/^$/) { f = 1; print }' | sha256sum`
const LEGACY_V1_CODE_SHA256: &str =
    "64832501fd6301575debe20f8f808bd1ec973092a08c2bb706e43d2851f7b5c5";

/// Returns the source after the leading `//!` and blank lines.
fn legacy_v1_code_body(source: &str) -> &str {
    let mut rest = source;
    while let Some(line_end) = rest.find('\n') {
        let line = &rest[..line_end];
        if !(line.is_empty() || line.starts_with("//!")) {
            break;
        }
        rest = &rest[line_end + 1..];
    }
    rest
}

/// Minimal FIPS 180-4 SHA-256, kept std-only for this integrity check.
fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut message = bytes.to_vec();
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for block in message.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (word, chunk) in schedule.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for index in 16..64 {
            let w15 = schedule[index - 15];
            let w2 = schedule[index - 2];
            let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
            let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (round_constant, word) in K.iter().zip(schedule.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(*round_constant)
                .wrapping_add(*word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (value, delta) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(delta);
        }
    }

    let mut hex = String::with_capacity(64);
    for value in state {
        for byte in value.to_be_bytes() {
            hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
            hex.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
        }
    }
    hex
}

#[test]
fn sha256_helper_matches_fips_180_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn frozen_v1_reference_code_is_unchanged() {
    let source = include_str!("support/legacy_v1.rs").replace("\r\n", "\n");
    let body = legacy_v1_code_body(&source);
    assert!(body.starts_with("use std::borrow::Cow;"), "{body:.40}");
    assert_eq!(sha256_hex(body.as_bytes()), LEGACY_V1_CODE_SHA256);
}

proptest! {
    #[test]
    fn idempotent_over_arbitrary_strings(input in any::<String>()) {
        let once = legacy_v1::normalize_cjk(&input);
        let twice = legacy_v1::normalize_cjk(&once);
        prop_assert_eq!(&twice, &once);
        prop_assert!(matches!(twice, Cow::Borrowed(_)));
    }

    #[test]
    fn preserves_non_space_character_order(input in any::<String>()) {
        let result = legacy_v1::normalize_cjk(&input);
        let input_chars: Vec<char> = input.chars().filter(|&c| c != ' ').collect();
        let output_chars: Vec<char> = result.chars().filter(|&c| c != ' ').collect();
        prop_assert_eq!(input_chars, output_chars);
    }

    #[test]
    fn never_panics_on_arbitrary_input(input in any::<String>()) {
        let _ = legacy_v1::normalize_cjk(&input);
    }

    #[test]
    fn extracted_legacy_matches_literal_reference(input in any_rust_string()) {
        prop_assert_eq!(legacy_v1::normalize_cjk(&input), legacy_reference(&input));
    }
}
