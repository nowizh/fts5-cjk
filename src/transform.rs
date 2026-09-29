use std::borrow::Cow;

use crate::classification::{is_cjk, needs_boundary};

/// Inserts ASCII-space boundaries required by the active CJK transform.
///
/// Adjacent supported CJK scalars are separated, as are mixed CJK/non-CJK
/// pairs that SQLite `unicode61` would otherwise join into one token. Existing
/// separator boundaries are preserved. The input is borrowed when its bytes do
/// not change.
#[must_use]
#[cfg(not(feature = "nfc"))]
pub fn normalize_cjk(input: &str) -> Cow<'_, str> {
    add_boundaries(input)
}

/// Composes NFC before inserting ASCII-space boundaries required by the active
/// CJK transform.
///
/// Adjacent supported CJK scalars are separated, as are mixed CJK/non-CJK
/// pairs that SQLite `unicode61` would otherwise join into one token. Existing
/// separator boundaries are preserved. The input is borrowed only when neither
/// NFC composition nor boundary insertion changes its bytes.
#[must_use]
#[cfg(feature = "nfc")]
pub fn normalize_cjk(input: &str) -> Cow<'_, str> {
    use unicode_normalization::{UnicodeNormalization, is_nfc};

    if is_nfc(input) {
        return add_boundaries(input);
    }

    let composed: String = input.nfc().collect();
    match add_boundaries(&composed) {
        Cow::Borrowed(_) => Cow::Owned(composed),
        Cow::Owned(spaced) => Cow::Owned(spaced),
    }
}

/// Collapses one ASCII space only when its immediate neighbours are supported
/// CJK scalars.
///
/// This helper is display-only and lossy. It preserves mixed CJK/token spaces,
/// leading and trailing spaces, runs of multiple spaces, and non-ASCII
/// whitespace, but it cannot distinguish an original single CJK-CJK space from
/// one inserted by [`normalize_cjk`]. It therefore does not restore original
/// whitespace.
#[must_use]
pub fn denormalize_cjk(input: &str) -> Cow<'_, str> {
    let mut chars = input.chars().peekable();
    let mut previous = None;
    let mut needs_change = false;
    while let Some(current) = chars.next() {
        if collapsible_cjk_space(previous, current, chars.peek().copied()) {
            needs_change = true;
            break;
        }
        previous = Some(current);
    }

    if !needs_change {
        return Cow::Borrowed(input);
    }

    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut previous = None;
    while let Some(current) = chars.next() {
        if collapsible_cjk_space(previous, current, chars.peek().copied()) {
            continue;
        }
        output.push(current);
        previous = Some(current);
    }

    Cow::Owned(output)
}

fn collapsible_cjk_space(previous: Option<char>, current: char, next: Option<char>) -> bool {
    current == ' ' && previous.is_some_and(is_cjk) && next.is_some_and(is_cjk)
}

fn add_boundaries(input: &str) -> Cow<'_, str> {
    if !input
        .chars()
        .zip(input.chars().skip(1))
        .any(|(left, right)| needs_boundary(left, right))
    {
        return Cow::Borrowed(input);
    }

    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    if let Some(first) = chars.next() {
        output.push(first);
        let mut left = first;
        for right in chars {
            if needs_boundary(left, right) {
                output.push(' ');
            }
            output.push(right);
            left = right;
        }
    }

    Cow::Owned(output)
}
