use std::borrow::Cow;

use fts5_cjk::{denormalize_cjk, is_cjk, normalize_cjk};
use proptest::prelude::*;

#[path = "../src/unicode61_data.rs"]
mod unicode61_data;

use unicode61_data::{UNICODE61_REMOVED_DIACRITICS, UNICODE61_TOKEN_RANGES};

#[cfg(feature = "nfc")]
use unicode_normalization::UnicodeNormalization;

fn any_rust_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::char::any(), 0..256)
        .prop_map(|chars| chars.into_iter().collect())
}

fn reference_is_unicode61_token_char(c: char) -> bool {
    let codepoint = c as u32;
    let index = UNICODE61_TOKEN_RANGES.partition_point(|&(_, end)| end < codepoint);
    UNICODE61_TOKEN_RANGES
        .get(index)
        .is_some_and(|&(start, end)| start <= codepoint && codepoint <= end)
}

fn reference_is_unicode61_removed_diacritic(c: char) -> bool {
    const MASK0: u32 = 0x0802_9FDF;
    const MASK1: u32 = 0x0003_61F8;

    let cp = c as u32;
    match cp {
        0x0300..=0x031F => MASK0 & (1 << (cp - 0x0300)) != 0,
        0x0320..=0x0331 => MASK1 & (1 << (cp - 0x0320)) != 0,
        _ => false,
    }
}

fn reference_is_unicode61_token_continuation(c: char) -> bool {
    reference_is_unicode61_token_char(c) || reference_is_unicode61_removed_diacritic(c)
}

#[test]
fn frozen_removed_diacritics_match_independent_reference_masks() {
    let expected: Vec<u32> = (0x0300..=0x0331)
        .filter(|&cp| {
            reference_is_unicode61_removed_diacritic(
                char::from_u32(cp).expect("reference range contains Rust scalars"),
            )
        })
        .collect();
    assert_eq!(expected.len(), 25);
    assert_eq!(UNICODE61_REMOVED_DIACRITICS, expected);
}

fn reference_needs_boundary(left: char, right: char) -> bool {
    (is_cjk(left) || is_cjk(right))
        && reference_is_unicode61_token_continuation(left)
        && reference_is_unicode61_token_continuation(right)
}

fn reference_normalize(input: &str) -> String {
    #[cfg(feature = "nfc")]
    let source: Cow<'_, str> = Cow::Owned(input.nfc().collect());
    #[cfg(not(feature = "nfc"))]
    let source: Cow<'_, str> = Cow::Borrowed(input);

    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars();
    if let Some(first) = chars.next() {
        output.push(first);
        let mut left = first;
        for right in chars {
            if reference_needs_boundary(left, right) {
                output.push(' ');
            }
            output.push(right);
            left = right;
        }
    }
    output
}

fn reference_denormalize(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(index, &current)| {
            let collapsible = current == ' '
                && index > 0
                && index + 1 < chars.len()
                && is_cjk(chars[index - 1])
                && is_cjk(chars[index + 1]);
            (!collapsible).then_some(current)
        })
        .collect()
}

proptest! {
    #[test]
    fn normalize_matches_reference(input in any_rust_string()) {
        let expected = reference_normalize(&input);
        let actual = normalize_cjk(&input);
        prop_assert_eq!(actual.as_bytes(), expected.as_bytes());
        prop_assert_eq!(matches!(actual, Cow::Borrowed(_)), expected == input);
    }

    #[test]
    fn normalize_is_idempotent_and_second_pass_borrowed(input in any_rust_string()) {
        let once = normalize_cjk(&input);
        let twice = normalize_cjk(&once);
        prop_assert_eq!(&twice, &once);
        prop_assert!(matches!(twice, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_preserves_order_and_never_panics(input in any_rust_string()) {
        let expected = reference_normalize(&input);
        let actual = normalize_cjk(&input);
        prop_assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    fn denormalize_is_narrow_and_never_panics(input in any_rust_string()) {
        let expected = reference_denormalize(&input);
        let actual = denormalize_cjk(&input);
        prop_assert_eq!(actual.as_bytes(), expected.as_bytes());
        prop_assert_eq!(matches!(actual, Cow::Borrowed(_)), expected == input);
    }
}
