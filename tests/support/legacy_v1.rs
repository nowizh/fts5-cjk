//! Test-only specification of fts5-cjk transform v1.
//!
//! Transform v1 inserts a boundary only between two adjacent supported CJK
//! scalars (CJK-CJK-only boundary). This frozen reference is used by contract
//! tests to pin how v2 differs from v1. It is not part of the
//! production module graph.

use std::borrow::Cow;

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

pub fn normalize_cjk(s: &str) -> Cow<'_, str> {
    let mut needs_change = false;
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev
            && is_cjk(p)
            && is_cjk(c)
        {
            needs_change = true;
            break;
        }
        prev = Some(c);
    }
    if !needs_change {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev
            && is_cjk(p)
            && is_cjk(c)
        {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    Cow::Owned(out)
}

pub fn denormalize_cjk(s: &str) -> Cow<'_, str> {
    let mut needs_change = false;
    {
        let mut it = s.chars().peekable();
        let mut prev: Option<char> = None;
        while let Some(c) = it.next() {
            if c == ' '
                && let Some(p) = prev
                && p != ' '
                && is_cjk(p)
                && let Some(&n) = it.peek()
                && n != ' '
                && is_cjk(n)
            {
                needs_change = true;
                break;
            }
            prev = Some(c);
        }
    }
    if !needs_change {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    let mut prev: Option<char> = None;
    while let Some(c) = it.next() {
        if c == ' '
            && let Some(p) = prev
            && p != ' '
            && is_cjk(p)
            && let Some(&n) = it.peek()
            && n != ' '
            && is_cjk(n)
        {
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    Cow::Owned(out)
}
