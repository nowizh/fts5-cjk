use crate::normalize_cjk;

/// Formats one complete raw value as a normalized, double-quoted phrase.
///
/// Embedded double quotes are doubled after the active transform is applied.
/// This helper only constructs a string; real FTS5 execution is outside its
/// contract.
#[must_use]
pub fn quote_fts5_phrase(raw: &str) -> String {
    let normalized = normalize_cjk(raw);
    let escaped = normalized.replace('"', "\"\"");
    let mut phrase = String::with_capacity(escaped.len() + 2);
    phrase.push('"');
    phrase.push_str(&escaped);
    phrase.push('"');
    phrase
}

/// Formats whitespace-separated raw terms as independently quoted phrases.
///
/// Splitting happens before normalization so one raw CJK term remains one
/// phrase. The returned phrases retain source order and are joined by one ASCII
/// space. Empty or whitespace-only input returns an empty string, which a
/// consumer must reject before passing it to `MATCH`.
#[must_use]
pub fn literal_query(raw: &str) -> String {
    raw.split_whitespace()
        .map(quote_fts5_phrase)
        .collect::<Vec<_>>()
        .join(" ")
}
