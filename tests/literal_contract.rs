use fts5_cjk::{literal_query, quote_fts5_phrase};

#[test]
fn quote_fts5_phrase_always_emits_one_phrase() {
    let cases = [
        ("empty", "", "\"\""),
        ("ascii", "hello", "\"hello\""),
        ("cjk compound", "플랫폼", "\"플 랫 폼\""),
        ("astral cjk", "𠀀𠀁", "\"𠀀 𠀁\""),
        ("embedded control", "a\0b", "\"a\0b\""),
    ];

    for (name, raw, expected) in cases {
        assert_eq!(
            quote_fts5_phrase(raw),
            expected,
            "{name} must emit exactly one outer quote pair"
        );
    }
}

#[test]
fn quote_fts5_phrase_doubles_embedded_quotes() {
    let cases = [
        ("one quote", "say\"hi", "\"say\"\"hi\""),
        ("multiple quotes", "\"a\"\"b\"", "\"\"\"a\"\"\"\"b\"\"\""),
        ("operator-like text", "a OR \"b\"", "\"a OR \"\"b\"\"\""),
    ];

    for (name, raw, expected) in cases {
        let quoted = quote_fts5_phrase(raw);
        assert_eq!(quoted, expected, "{name} escaping drifted");
        assert!(!quoted.contains("\\\""), "{name} used a backslash escape");
    }
}

#[test]
fn quote_fts5_phrase_uses_active_transform() {
    assert_eq!(
        quote_fts5_phrase("AI개2\"끝"),
        "\"AI 개 2\"\"끝\"",
        "the whole raw value must be normalized before quote doubling and wrapping"
    );
}

#[test]
fn literal_helpers_reflect_diacritic_continuation_transform_shapes() {
    assert_eq!(quote_fts5_phrase("개\u{0301}발"), "\"개 \u{0301} 발\"");
    assert_eq!(literal_query("개\u{0301}발"), "\"개 \u{0301} 발\"");

    assert_eq!(quote_fts5_phrase("개\u{0345}발"), "\"개\u{0345}발\"");
    assert_eq!(literal_query("개\u{0345}발"), "\"개\u{0345}발\"");
}

#[cfg(not(feature = "nfc"))]
#[test]
fn quote_fts5_phrase_uses_default_transform_shape() {
    assert_eq!(quote_fts5_phrase("가\"끝"), "\"ᄀ ᅡ\"\"끝\"");
}

#[cfg(feature = "nfc")]
#[test]
fn quote_fts5_phrase_uses_nfc_transform_shape() {
    assert_eq!(quote_fts5_phrase("가\"끝"), "\"가\"\"끝\"");
}

#[test]
fn literal_query_splits_before_normalizing() {
    assert_eq!(literal_query("플랫폼"), "\"플 랫 폼\"");
    assert_eq!(
        literal_query("플랫폼 hello"),
        "\"플 랫 폼\" \"hello\"",
        "one raw CJK term must remain one complete phrase"
    );
}

#[test]
fn literal_query_preserves_order_and_implicit_and() {
    assert_eq!(
        literal_query("  AI개\u{2003}𠀀𠀁\n\"quoted\"  "),
        "\"AI 개\" \"𠀀 𠀁\" \"\"\"quoted\"\"\"",
        "raw terms must retain source order with one ASCII join space"
    );
    assert_eq!(
        literal_query("a\0개 tail"),
        "\"a\0개\" \"tail\"",
        "non-whitespace controls inside a term must remain encoded"
    );
}

#[test]
fn literal_query_empty_input_is_empty() {
    for raw in ["", "   ", "\t\n\r", "\u{00a0}\u{2003}\u{3000}"] {
        assert_eq!(
            literal_query(raw),
            String::new(),
            "empty or Unicode-whitespace-only input must stay empty"
        );
    }
}

#[cfg(not(feature = "nfc"))]
#[test]
fn literal_query_uses_default_transform_shape() {
    assert_eq!(literal_query("가 mixed개"), "\"ᄀ ᅡ\" \"mixed 개\"");
}

#[cfg(feature = "nfc")]
#[test]
fn literal_query_uses_nfc_transform_shape() {
    assert_eq!(literal_query("가 mixed개"), "\"가\" \"mixed 개\"");
}
