/// Complete compile-time identity of the active transform contract.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransformProfile {
    /// Byte-exact identity of the complete profile.
    pub id: &'static str,
    /// Version of the transform algorithm.
    pub algorithm_version: u32,
    /// Whether NFC normalization is enabled in this build.
    pub nfc: bool,
    /// SQL UDF name reserved for this profile.
    pub udf_name: &'static str,
    /// Byte-exact FTS5 tokenizer specification recommended by this profile.
    pub recommended_tokenizer: &'static str,
}

/// Version of the transform algorithm.
pub const TRANSFORM_VERSION: u32 = 2;

const RECOMMENDED_TOKENIZER: &str = "unicode61 remove_diacritics 2";

#[cfg(not(feature = "nfc"))]
const PROFILE_ID: &str = "fts5-cjk/v2;unicode61=6.1-default;nfc=off";
#[cfg(feature = "nfc")]
const PROFILE_ID: &str = "fts5-cjk/v2;unicode61=6.1-default;nfc=on";

/// SQL UDF name for the active compile-time profile.
#[cfg(not(feature = "nfc"))]
pub const UDF_NAME: &str = "fts5_cjk_v2";
/// SQL UDF name for the active compile-time profile.
#[cfg(feature = "nfc")]
pub const UDF_NAME: &str = "fts5_cjk_v2_nfc";

#[cfg(not(feature = "nfc"))]
const NFC: bool = false;
#[cfg(feature = "nfc")]
const NFC: bool = true;

// The nfc profile identity covers the NFC tables. Cargo.toml bounds the
// dependency below 0.1.26; this assertion also fails builds where a patch,
// fork or relaxed bound supplies other tables.
#[cfg(feature = "nfc")]
const _: () = {
    let (major, minor, patch) = unicode_normalization::UNICODE_VERSION;
    assert!(
        major == 17 && minor == 0 && patch == 0,
        "fts5-cjk nfc profile requires Unicode 17.0.0 normalization tables; other tables need a new transform profile and a full reindex"
    );
};

/// Complete transform profile compiled into this crate.
pub const ACTIVE_PROFILE: TransformProfile = TransformProfile {
    id: PROFILE_ID,
    algorithm_version: TRANSFORM_VERSION,
    nfc: NFC,
    udf_name: UDF_NAME,
    recommended_tokenizer: RECOMMENDED_TOKENIZER,
};
