use fts5_cjk::is_cjk;
#[cfg(not(feature = "nfc"))]
use fts5_cjk::{ACTIVE_PROFILE, TRANSFORM_VERSION, TransformProfile, UDF_NAME};

#[test]
fn range_boundary_edges_are_classified_cjk() {
    let edges: &[(&str, char)] = &[
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

    for (name, scalar) in edges {
        assert!(is_cjk(*scalar), "{name} edge {scalar:?} must classify CJK");
    }
}

#[test]
fn range_boundary_neighbour_semantics_are_exact() {
    let neighbours: &[(&str, char, bool)] = &[
        ("below Han core", '\u{4DFF}', false),
        ("above Han core", '\u{A000}', false),
        ("below Han Extension A", '\u{33FF}', false),
        ("above Han Extension A", '\u{4DC0}', false),
        ("below Han Extension B", '\u{1FFFF}', false),
        ("above Han Extension B", '\u{2A6E0}', false),
        ("below Hiragana", '\u{303F}', false),
        ("above Hiragana is Katakana lower", '\u{30A0}', true),
        ("below Katakana is Hiragana upper", '\u{309F}', true),
        ("above Katakana", '\u{3100}', false),
        ("below Hangul Syllables", '\u{ABFF}', false),
        ("above Hangul Syllables", '\u{D7A4}', false),
        ("below Hangul Jamo", '\u{10FF}', false),
        ("above Hangul Jamo", '\u{1200}', false),
        ("below Hangul Compatibility Jamo", '\u{312F}', false),
        ("above Hangul Compatibility Jamo", '\u{3190}', false),
    ];

    for (name, scalar, expected) in neighbours {
        assert_eq!(
            is_cjk(*scalar),
            *expected,
            "{name} neighbour {scalar:?} classification drifted"
        );
    }
}

#[test]
fn classification_is_total_and_deterministic_for_representative_scalars() {
    let scalars = ['\0', 'A', '\u{1100}', '\u{AC00}', '\u{20000}', '\u{10FFFF}'];

    for scalar in scalars {
        let expected = is_cjk(scalar);
        for _ in 0..32 {
            assert_eq!(
                is_cjk(scalar),
                expected,
                "{scalar:?} must classify deterministically"
            );
        }
    }
}

#[cfg(not(feature = "nfc"))]
#[test]
fn default_profile_is_visible_through_the_crate_root() {
    let profile: TransformProfile = ACTIVE_PROFILE;

    assert_eq!(TRANSFORM_VERSION, 2);
    assert_eq!(UDF_NAME, "fts5_cjk_v2");
    assert_eq!(profile.id, "fts5-cjk/v2;unicode61=6.1-default;nfc=off");
    assert_eq!(profile.algorithm_version, TRANSFORM_VERSION);
    assert!(!profile.nfc);
    assert_eq!(profile.udf_name, UDF_NAME);
    assert_eq!(
        profile.recommended_tokenizer,
        "unicode61 remove_diacritics 2"
    );
}
