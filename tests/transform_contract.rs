use std::borrow::Cow;

use fts5_cjk::{ACTIVE_PROFILE, denormalize_cjk, normalize_cjk};

#[test]
fn normalize_cjk_boundaries_are_exact() {
    let cases = [
        ("headline", "AI개발중", "AI 개 발 중"),
        ("letter before CJK", "A개", "A 개"),
        ("letter after CJK", "개A", "개 A"),
        ("digit before CJK", "1개", "1 개"),
        ("digit after CJK", "개1", "개 1"),
        ("Han core", "中文", "中 文"),
        ("Han Extension A", "㐀㐁", "㐀 㐁"),
        ("Han Extension B", "𠀀𠀁", "𠀀 𠀁"),
        ("Hiragana", "あい", "あ い"),
        ("Katakana", "アイ", "ア イ"),
        ("Hangul syllables", "가나", "가 나"),
        ("Hangul Jamo", "ᄀᄁ", "ᄀ ᄁ"),
        ("Hangul compatibility Jamo", "ㄱㄲ", "ㄱ ㄲ"),
    ];

    for (name, input, expected) in cases {
        assert_eq!(normalize_cjk(input), expected, "{name} boundary drifted");
    }
}

#[test]
fn normalize_cjk_preserves_existing_separator_boundaries() {
    let cases = [
        ("empty", ""),
        ("ASCII", "subject:project"),
        ("single scalar", "개"),
        ("already normalized", "AI 개 발 중"),
        ("space", "개 발"),
        ("punctuation", "개,발"),
        ("underscore", "개_발"),
        ("U+0345 combining-mark separator", "개\u{0345}발"),
        ("U+3099 combining voiced mark", "개\u{3099}발"),
        ("U+309A combining semi-voiced mark", "개\u{309A}발"),
        ("U+309B voiced mark", "개\u{309B}발"),
        ("U+309C semi-voiced mark", "개\u{309C}발"),
        ("U+30A0 katakana-hiragana double hyphen", "개\u{30A0}발"),
        ("U+30FB katakana middle dot", "개\u{30FB}발"),
        ("NUL", "개\0발"),
        ("U+FFFE", "개\u{FFFE}발"),
        ("U+FFFF", "개\u{FFFF}발"),
    ];

    for (name, input) in cases {
        let result = normalize_cjk(input);
        assert_eq!(result, input, "{name} boundary must stay byte-identical");
        assert!(
            matches!(result, Cow::Borrowed(_)),
            "{name} boundary must take the borrowed fast path"
        );
    }
}

#[test]
fn normalize_cjk_splits_removed_diacritic_continuations() {
    let result = normalize_cjk("개\u{0301}발");
    assert_eq!(result, "개 \u{0301} 발");
    assert!(matches!(result, Cow::Owned(_)));

    let separator = normalize_cjk("개\u{0345}발");
    assert_eq!(separator, "개\u{0345}발");
    assert!(matches!(separator, Cow::Borrowed(_)));
}

#[test]
fn normalize_cjk_cow_variants_are_exact() {
    let changed = normalize_cjk("AI개발중");
    assert_eq!(changed, "AI 개 발 중");
    assert!(matches!(changed, Cow::Owned(_)));

    let unchanged = normalize_cjk("AI 개 발 중");
    assert_eq!(unchanged, "AI 개 발 중");
    assert!(matches!(unchanged, Cow::Borrowed(_)));
}

#[test]
fn denormalize_cjk_collapses_only_single_cjk_spaces() {
    let cases = [
        ("empty", "", ""),
        ("CJK run", "프 로 젝 트", "프로젝트"),
        ("mixed left boundary", "AI 개", "AI 개"),
        ("mixed right boundary", "개 AI", "개 AI"),
        ("mixed run", "AI 개 발 중", "AI 개발중"),
        ("leading", " 가나", " 가나"),
        ("trailing", "가나 ", "가나 "),
        ("double space", "가  나", "가  나"),
        ("triple space", "가   나", "가   나"),
        ("tab", "가\t나", "가\t나"),
        ("newline", "가\n나", "가\n나"),
        ("NUL", "가\0나", "가\0나"),
        ("astral CJK", "𠀀 𠀁", "𠀀𠀁"),
    ];

    for (name, input, expected) in cases {
        assert_eq!(
            denormalize_cjk(input),
            expected,
            "{name} denormalization drifted"
        );
    }
}

#[test]
fn denormalize_cjk_cow_variants_are_exact() {
    let changed = denormalize_cjk("프 로 젝 트");
    assert_eq!(changed, "프로젝트");
    assert!(matches!(changed, Cow::Owned(_)));

    for input in ["", "AI 개", "개 AI", "가  나", "가\t나"] {
        let unchanged = denormalize_cjk(input);
        assert_eq!(unchanged, input);
        assert!(matches!(unchanged, Cow::Borrowed(_)));
    }
}

#[test]
fn nfc_precedes_spacing() {
    let profile = ACTIVE_PROFILE;
    let decomposed_hangul = normalize_cjk("\u{1100}\u{1161}");
    let composition_only = normalize_cjk("e\u{0301}");
    let composition_and_spacing = normalize_cjk("e\u{0301}개");
    let already_nfc = normalize_cjk("é");

    #[cfg(not(feature = "nfc"))]
    {
        assert!(!profile.nfc);
        assert_eq!(decomposed_hangul, "\u{1100} \u{1161}");
        assert!(matches!(decomposed_hangul, Cow::Owned(_)));
        assert_eq!(composition_only, "e\u{0301}");
        assert!(matches!(composition_only, Cow::Borrowed(_)));
        assert_eq!(composition_and_spacing, "e\u{0301} 개");
        assert!(matches!(composition_and_spacing, Cow::Owned(_)));
        assert_eq!(already_nfc, "é");
        assert!(matches!(already_nfc, Cow::Borrowed(_)));
    }

    #[cfg(feature = "nfc")]
    {
        assert!(profile.nfc);
        assert_eq!(decomposed_hangul, "가");
        assert!(matches!(decomposed_hangul, Cow::Owned(_)));
        assert_eq!(composition_only, "é");
        assert!(matches!(composition_only, Cow::Owned(_)));
        assert_eq!(composition_and_spacing, "é 개");
        assert!(matches!(composition_and_spacing, Cow::Owned(_)));
        assert_eq!(already_nfc, "é");
        assert!(matches!(already_nfc, Cow::Borrowed(_)));
    }
}
