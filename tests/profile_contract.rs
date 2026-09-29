use std::fmt::Debug;
use std::hash::Hash;

use fts5_cjk::{ACTIVE_PROFILE, TRANSFORM_VERSION, TransformProfile, UDF_NAME};

fn assert_profile_traits<T: Copy + Clone + Debug + Eq + PartialEq + Hash>() {}

#[test]
fn profile_is_public_complete_and_consistent() {
    assert_profile_traits::<TransformProfile>();

    let profile: TransformProfile = ACTIVE_PROFILE;
    assert!(!profile.id.is_empty());
    assert_eq!(profile.algorithm_version, TRANSFORM_VERSION);
    assert_eq!(profile.udf_name, UDF_NAME);
    assert!(!profile.udf_name.is_empty());
    assert_eq!(
        profile.recommended_tokenizer,
        "unicode61 remove_diacritics 2"
    );
}

#[cfg(not(feature = "nfc"))]
#[test]
fn default_profile_values_are_exact() {
    let profile = ACTIVE_PROFILE;

    assert_eq!(TRANSFORM_VERSION, 2);
    assert_eq!(UDF_NAME, "fts5_cjk_v2");
    assert_eq!(profile.id, "fts5-cjk/v2;unicode61=6.1-default;nfc=off");
    assert!(!profile.nfc);
}

#[cfg(feature = "nfc")]
#[test]
fn nfc_profile_values_are_exact() {
    let profile = ACTIVE_PROFILE;

    assert_eq!(TRANSFORM_VERSION, 2);
    assert_eq!(UDF_NAME, "fts5_cjk_v2_nfc");
    assert_eq!(profile.id, "fts5-cjk/v2;unicode61=6.1-default;nfc=on");
    assert!(profile.nfc);
}
