#[path = "unicode61_data.rs"]
mod unicode61_data;

use unicode61_data::{UNICODE61_REMOVED_DIACRITICS, UNICODE61_TOKEN_RANGES};

/// Returns whether `c` belongs to the transform profile's fixed CJK policy.
///
/// The policy covers eight versioned Rust `char` ranges. It is deliberately
/// not an exhaustive or moving definition of Unicode CJK characters.
#[must_use]
pub fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'     // Han (CJK Unified Ideographs)
        | '\u{3400}'..='\u{4DBF}'   // Han Extension A
        | '\u{20000}'..='\u{2A6DF}' // Han Extension B
        | '\u{3040}'..='\u{309F}'   // Hiragana
        | '\u{30A0}'..='\u{30FF}'   // Katakana
        | '\u{AC00}'..='\u{D7A3}'   // Hangul Syllables
        | '\u{1100}'..='\u{11FF}'   // Hangul Jamo
        | '\u{3130}'..='\u{318F}'   // Hangul Compatibility Jamo
    )
}

fn is_unicode61_token_char(c: char) -> bool {
    let cp = c as u32;
    let index = UNICODE61_TOKEN_RANGES.partition_point(|&(_, end)| end < cp);
    UNICODE61_TOKEN_RANGES
        .get(index)
        .is_some_and(|&(start, end)| start <= cp && cp <= end)
}

fn is_unicode61_removed_diacritic(c: char) -> bool {
    UNICODE61_REMOVED_DIACRITICS
        .binary_search(&(c as u32))
        .is_ok()
}

fn is_unicode61_token_continuation(c: char) -> bool {
    is_unicode61_token_char(c) || is_unicode61_removed_diacritic(c)
}

pub(crate) fn needs_boundary(left: char, right: char) -> bool {
    (is_cjk(left) || is_cjk(right))
        && is_unicode61_token_continuation(left)
        && is_unicode61_token_continuation(right)
}

#[cfg(test)]
mod oracle;
