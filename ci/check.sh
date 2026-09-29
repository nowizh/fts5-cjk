#!/usr/bin/env bash

set -euo pipefail

export LC_ALL=C

readonly EXPECTED_RUSTC="rustc 1.97.1 (8bab26f4f 2026-07-14)"
readonly EXPECTED_CARGO="cargo 1.97.1 (c980f4866 2026-06-30)"
readonly SQLITE_ARCHIVE_URL="https://www.sqlite.org/2026/sqlite-autoconf-3530200.tar.gz"
readonly SQLITE_ARCHIVE_SHA256="588ad51949419a56ebe81fe56193d510c559eb94c9a57748387860b5d3069316"
readonly SQLITE_SOURCE_SHA3_256="44fd61b9f93b4155105cb2d80c957ae6c64a8b5bd6ed51a4992f0dbd438e4e11"
readonly EXPECTED_SQLITE_VERSION="3.53.2"
readonly EXPECTED_SQLITE_SOURCE_ID="2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24"

SQLITE_BUILD_DIR=""
RESOLVED_SQLITE3=""

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

assert_equal() {
    local label=$1
    local expected=$2
    local actual=$3

    if [[ "$actual" != "$expected" ]]; then
        die "$label mismatch: expected '$expected', got '$actual'"
    fi
}

require_tools() {
    local tool

    for tool in basename cargo cc curl dirname git grep make mktemp openssl python3 rm rustc sed sha256sum tar wc; do
        command -v "$tool" >/dev/null 2>&1 || die "required host tool '$tool' is unavailable"
    done
}

cleanup() {
    if [[ -n "$SQLITE_BUILD_DIR" && "$SQLITE_BUILD_DIR" != "/" && -d "$SQLITE_BUILD_DIR" ]]; then
        rm -rf -- "$SQLITE_BUILD_DIR"
    fi
}

trap cleanup EXIT

absolute_executable() {
    local candidate=$1
    local directory
    local resolved

    if [[ "$candidate" == */* ]]; then
        [[ -x "$candidate" ]] || die "FTS5 sqlite3 executable is not executable: $candidate"
        directory=$(cd -- "$(dirname -- "$candidate")" && pwd -P)
        resolved="$directory/$(basename -- "$candidate")"
    else
        resolved=$(command -v -- "$candidate") || die "FTS5 sqlite3 executable is unavailable: $candidate"
    fi

    [[ "$resolved" == /* ]] || die "resolved sqlite3 path is not absolute: $resolved"
    printf '%s\n' "$resolved"
}

provision_sqlite_3_53_2() {
    local archive
    local actual_archive_sha256
    local actual_source_sha3_256
    local source_dir
    local source_digest_line

    archive="$SQLITE_BUILD_DIR/sqlite-autoconf-3530200.tar.gz"

    printf 'Provisioning SQLite %s from %s\n' "$EXPECTED_SQLITE_VERSION" "$SQLITE_ARCHIVE_URL"
    curl --fail --show-error --silent --location --retry 3 \
        --output "$archive" "$SQLITE_ARCHIVE_URL"

    read -r actual_archive_sha256 _ < <(sha256sum "$archive")
    assert_equal "SQLite archive SHA-256" "$SQLITE_ARCHIVE_SHA256" "$actual_archive_sha256"

    tar -xzf "$archive" -C "$SQLITE_BUILD_DIR"
    source_dir="$SQLITE_BUILD_DIR/sqlite-autoconf-3530200"
    [[ -f "$source_dir/sqlite3.c" ]] || die "SQLite amalgamation is missing after extraction"

    source_digest_line=$(openssl dgst -sha3-256 "$source_dir/sqlite3.c")
    actual_source_sha3_256=${source_digest_line##*= }
    assert_equal "sqlite3.c SHA3-256" "$SQLITE_SOURCE_SHA3_256" "$actual_source_sha3_256"

    (
        cd "$source_dir"
        ./configure --disable-shared --disable-readline --enable-fts5
        make sqlite3
    )

    RESOLVED_SQLITE3="$source_dir/sqlite3"
}

resolve_sqlite3() {
    local candidate

    if [[ -n "${FTS5_CJK_SQLITE3:-}" ]]; then
        candidate=$FTS5_CJK_SQLITE3
    elif [[ "${CI:-}" == "true" ]]; then
        provision_sqlite_3_53_2
        candidate=$RESOLVED_SQLITE3
    else
        candidate=$(command -v sqlite3 2>/dev/null) || die \
            "sqlite3 is required; install it or set FTS5_CJK_SQLITE3 to an executable"
    fi

    RESOLVED_SQLITE3=$(absolute_executable "$candidate")
    export FTS5_CJK_SQLITE3=$RESOLVED_SQLITE3
}

check_toolchain_identity() {
    assert_equal "rustc identity" "$EXPECTED_RUSTC" "$(rustc --version)"
    assert_equal "Cargo identity" "$EXPECTED_CARGO" "$(cargo --version)"
}

check_sqlite_identity() {
    local capability
    local parameter_probe
    local probe_output
    local source_id
    local version

    if ! probe_output=$(printf '%s\n' \
        ".bail on" \
        ".parameter init" \
        ".parameter set @fts5_cjk_probe \"CAST(X'667473352D636A6B2D706172616D657465722D6F6B' AS TEXT)\"" \
        "SELECT 'PARAMETER|' || typeof(@fts5_cjk_probe) || '|' || @fts5_cjk_probe;" \
        "SELECT 'VERSION|' || sqlite_version();" \
        "SELECT 'SOURCE|' || sqlite_source_id();" \
        "SELECT 'FTS5|' || sqlite_compileoption_used('ENABLE_FTS5');" \
        "CREATE VIRTUAL TABLE fts5_cjk_ci_probe USING fts5(text, tokenize='unicode61 remove_diacritics 2');" \
        "SELECT 'TABLE|ok';" \
        | "$FTS5_CJK_SQLITE3" -batch -bail :memory: 2>&1); then
        die "sqlite3 CLI must support .parameter (SQLite CLI >= 3.28.0) and FTS5: $probe_output"
    fi

    parameter_probe=$(printf '%s\n' "$probe_output" | sed -n 's/^PARAMETER|//p')
    version=$(printf '%s\n' "$probe_output" | sed -n 's/^VERSION|//p')
    source_id=$(printf '%s\n' "$probe_output" | sed -n 's/^SOURCE|//p')
    capability=$(printf '%s\n' "$probe_output" | sed -n 's/^FTS5|//p')

    if [[ "$parameter_probe" != "text|fts5-cjk-parameter-ok" ]]; then
        die "sqlite3 CLI must support .parameter (SQLite CLI >= 3.28.0) with an exact TEXT round trip; got '$parameter_probe'"
    fi
    [[ -n "$version" ]] || die "sqlite3 identity probe omitted sqlite_version()"
    [[ -n "$source_id" ]] || die "sqlite3 identity probe omitted sqlite_source_id()"
    printf '%s\n' "$probe_output" | grep -Fx 'TABLE|ok' >/dev/null || die \
        "sqlite3 could not create the exact unicode61 FTS5 table"

    printf 'SQLite executable: %s\n' "$FTS5_CJK_SQLITE3"
    printf 'SQLite CLI .parameter TEXT round trip: ok\n'
    printf 'SQLite version: %s\n' "$version"
    printf 'SQLite source ID: %s\n' "$source_id"
    printf 'SQLite ENABLE_FTS5: %s\n' "$capability"

    if [[ "${CI:-}" == "true" ]]; then
        assert_equal "SQLite version" "$EXPECTED_SQLITE_VERSION" "$version"
        assert_equal "SQLite source ID" "$EXPECTED_SQLITE_SOURCE_ID" "$source_id"
        assert_equal "SQLite ENABLE_FTS5 capability" "1" "$capability"
    fi
}

check_manifest_contract() {
    local metadata_file

    metadata_file="$SQLITE_BUILD_DIR/cargo-metadata.json"
    cargo metadata --format-version 1 --locked --no-deps >"$metadata_file"
    python3 -E -c '
import json
import sys
import tomllib


def require(condition, label):
    # Explicit checks survive PYTHONOPTIMIZE, which strips assert statements.
    if not condition:
        sys.exit(f"error: manifest contract mismatch: {label}")


package = json.load(open(sys.argv[1], encoding="utf-8"))["packages"][0]
require(package["name"] == "fts5-cjk", "name")
require(package["version"] == "0.1.0", "version")
require(package["license"] == "MIT", "license")
require(package["publish"] is None, "publish")
require(package["readme"] == "README.md", "readme")
require(package["documentation"] == "https://docs.rs/fts5-cjk", "documentation")
require(package["repository"] == "https://github.com/nowizh/fts5-cjk", "repository")
require(
    package["keywords"] == ["sqlite", "fts5", "cjk", "korean", "full-text-search"],
    "keywords",
)
require(package["categories"] == ["text-processing", "database"], "categories")
require(
    package["metadata"]
    == {
        "docs": {
            "rs": {
                "all-features": False,
                "no-default-features": False,
                "features": [],
            },
        },
    },
    "package.metadata.docs.rs",
)
require(package["rust_version"] == "1.97.1", "rust-version")
with open("rust-toolchain.toml", "rb") as toolchain_file:
    toolchain = tomllib.load(toolchain_file)
require(
    toolchain["toolchain"]["channel"] == package["rust_version"],
    "rust-toolchain.toml channel",
)
require(
    package["features"]
    == {
        "default": [],
        "nfc": ["dep:unicode-normalization"],
    },
    "features",
)

dependencies = {
    (dependency["name"], dependency["kind"]): (
        dependency["optional"],
        dependency["uses_default_features"],
        dependency["req"],
    )
    for dependency in package["dependencies"]
}
require(
    dependencies
    == {
        ("unicode-normalization", None): (True, False, "^0.1.25"),
        ("proptest", "dev"): (False, False, "^1.11.0"),
    },
    "dependencies",
)
' "$metadata_file"
}

check_dependency_contract() {
    local all_tree
    local default_tree
    local default_tree_lines
    local nfc_tree

    default_tree=$(cargo tree --locked --no-default-features --edges normal --depth 1 --prefix none)
    default_tree_lines=$(printf '%s\n' "$default_tree" | sed '/^$/d' | wc -l)
    assert_equal "default selected normal dependency line count" "1" "$default_tree_lines"
    [[ "$default_tree" == fts5-cjk\ v0.1.0* ]] || die \
        "default selected normal dependency tree does not contain only the crate root"

    nfc_tree=$(cargo tree --locked --no-default-features --features nfc --edges normal --depth 1 --prefix none)
    printf '%s\n' "$nfc_tree" | grep -Fx 'unicode-normalization v0.1.25' >/dev/null || die \
        "nfc selected normal dependency tree omitted unicode-normalization v0.1.25"
    if grep -Eq '^(proptest|rusqlite|libsqlite3-sys) ' <<<"$nfc_tree"; then
        die "nfc selected normal dependency tree contains a forbidden dependency kind"
    fi

    all_tree=$(cargo tree --locked --all-features --edges all --prefix none)
    if grep -Eq '^(rusqlite|libsqlite3-sys) ' <<<"$all_tree"; then
        die "Cargo dependency graph contains a forbidden SQLite binding"
    fi
}

check_package_contents() {
    local actual
    local expected

    actual=$(cargo package --list --locked --allow-dirty | sed '/^\.cargo_vcs_info\.json$/d')
    expected=$(printf '%s\n' \
        CHANGELOG.md \
        Cargo.lock \
        Cargo.toml \
        Cargo.toml.orig \
        LICENSE \
        README.md \
        src/classification/oracle.rs \
        src/classification.rs \
        src/lib.rs \
        src/literal.rs \
        src/profile.rs \
        src/transform.rs \
        src/unicode61_data.rs \
        tests/classification_contract.rs \
        tests/fts5_contract.rs \
        tests/legacy_contract.rs \
        tests/literal_contract.rs \
        tests/profile_contract.rs \
        tests/support/legacy_v1.rs \
        tests/transform_contract.rs \
        tests/transform_properties.rs \
        tools/unicode61/LICENSE-UNICODE)
    assert_equal "packaged file list" "$expected" "$actual"
}

check_release_inputs() {
    git ls-files --error-unmatch Cargo.toml Cargo.lock rust-toolchain.toml LICENSE CHANGELOG.md >/dev/null
    check_manifest_contract
    check_dependency_contract
    check_package_contents
    cargo publish --dry-run --locked --allow-dirty --no-default-features
    cargo publish --dry-run --locked --allow-dirty --no-default-features --features nfc
}

run_test_gate() {
    require_tools
    check_toolchain_identity
    SQLITE_BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fts5-cjk-check.XXXXXX")
    resolve_sqlite3
    check_sqlite_identity

    python3 tools/unicode61/generate.py --check
    cargo fmt --all -- --check
    cargo test --locked --no-default-features
    cargo test --locked --no-default-features --features nfc
    RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked --no-default-features
    RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked --no-default-features --features nfc
    check_release_inputs
}

run_clippy_gate() {
    cargo clippy --all-targets --locked --no-default-features -- -D warnings
    cargo clippy --all-targets --locked --no-default-features --features nfc -- -D warnings
}

usage() {
    printf 'Usage: %s [all|test|clippy]\n' "$0" >&2
}

case "${1:-all}" in
    all)
        run_test_gate
        run_clippy_gate
        ;;
    test)
        run_test_gate
        ;;
    clippy)
        run_clippy_gate
        ;;
    *)
        usage
        exit 2
        ;;
esac
