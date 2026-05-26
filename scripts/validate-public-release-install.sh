#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

REPOSITORY="${NEXUS_PUBLIC_RELEASE_REPOSITORY:-vitaleevo/NEXUSLANG}"
TAG="${NEXUS_PUBLIC_RELEASE_TAG:-v0.1.1}"
VERSION="${TAG#v}"
ARCHIVE_NAME="${NEXUS_PUBLIC_RELEASE_ARCHIVE:-nexuslang-$TAG-local-release.tar.gz}"
CHECKSUM_NAME="$ARCHIVE_NAME.sha256"
ARCHIVE_SIG_NAME="$ARCHIVE_NAME.asc"
CHECKSUM_SIG_NAME="$CHECKSUM_NAME.asc"
PUBLIC_KEY_NAME="${NEXUS_PUBLIC_RELEASE_PUBLIC_KEY:-nexuslang-release-public-key.asc}"
FINGERPRINT_NAME="${NEXUS_PUBLIC_RELEASE_FINGERPRINT_FILE:-nexuslang-release-signing-key.fingerprint}"
EXPECTED_FINGERPRINT="${NEXUS_PUBLIC_RELEASE_SIGNING_FINGERPRINT:-3237F7CC5CE2514FC9671BB93CB6808B55385273}"
BASE_URL="${NEXUS_PUBLIC_RELEASE_BASE_URL:-https://github.com/$REPOSITORY/releases/download/$TAG}"
REPORT_PATH="${NEXUS_PUBLIC_RELEASE_REPORT:-$DIST_DIR/public-release-install-validation-report.txt}"
PORT="${NEXUS_PUBLIC_RELEASE_VALIDATE_PORT:-8093}"
KEEP_WORK_DIR="${NEXUS_PUBLIC_RELEASE_KEEP_WORK_DIR:-0}"

WORK_DIR=""
DOWNLOAD_DIR=""
GNUPG_HOME=""
SERVER_PID=""

usage() {
    cat <<'EOF'
Usage: scripts/validate-public-release-install.sh

Downloads the published NexusLang GitHub Release assets into a clean temporary
directory, verifies checksum and detached GPG signatures with an isolated GPG
home, extracts the package, and runs the packaged smoke test.

Optional environment:
  NEXUS_PUBLIC_RELEASE_REPOSITORY=owner/repo
  NEXUS_PUBLIC_RELEASE_TAG=v0.1.1
  NEXUS_PUBLIC_RELEASE_SIGNING_FINGERPRINT=<fingerprint>
  NEXUS_PUBLIC_RELEASE_VALIDATE_PORT=8093
  NEXUS_PUBLIC_RELEASE_KEEP_WORK_DIR=1
  NEXUS_PUBLIC_RELEASE_REPORT=dist/public-release-install-validation-report.txt

The default tag follows the latest published release. After publishing a new
release, set NEXUS_PUBLIC_RELEASE_TAG explicitly, for example:

  NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh
EOF
}

while [ "${1:-}" != "" ]; do
    case "$1" in
        --help | -h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

mkdir -p "$DIST_DIR"
: > "$REPORT_PATH"

report_line() {
    echo "$*" | tee -a "$REPORT_PATH" >/dev/null
}

fail() {
    local code="$1"
    local message="$2"

    report_line "public_install_validation_status=failed:$code"
    echo "ERROR: $message" >&2
    echo "Public install validation report: $REPORT_PATH" >&2
    exit 1
}

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi

    if [ "$KEEP_WORK_DIR" != "1" ] && [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then
        case "$WORK_DIR" in
            /tmp/nexus-public-release-install.*) rm -rf "$WORK_DIR" ;;
        esac
    fi
}
trap cleanup EXIT

run() {
    echo ""
    echo "==> $*"
    "$@"
}

require_command() {
    local name="$1"

    command -v "$name" >/dev/null 2>&1 || fail "missing-$name" "$name is required."
}

download_asset() {
    local name="$1"
    local url="$BASE_URL/$name"

    run curl -fsSL --retry 3 --retry-delay 2 -o "$DOWNLOAD_DIR/$name" "$url"
}

assert_file() {
    [ -f "$1" ] || fail "missing-file" "Missing file: $1"
}

assert_executable() {
    [ -x "$1" ] || fail "missing-executable" "Missing executable: $1"
}

validate_tar_paths() {
    tar -tzf "$DOWNLOAD_DIR/$ARCHIVE_NAME" | while IFS= read -r entry; do
        case "$entry" in
            /* | *../*) fail "unsafe-archive-path" "Unsafe archive path: $entry" ;;
        esac
    done
}

top_level_package_name() {
    tar -tzf "$DOWNLOAD_DIR/$ARCHIVE_NAME" | awk -F '/' '
        NF && $1 != "" && found == 0 {
            print $1
            found = 1
        }
        END {
            exit found ? 0 : 1
        }
    '
}

manifest_value() {
    local package_dir="$1"
    local name="$2"

    sed -n "s/^$name=//p" "$package_dir/PACKAGE_MANIFEST.txt" | head -n 1
}

assert_no_generated_storage() {
    if find "$1" -name ".nexus-data" -print -quit | grep -q .; then
        fail "generated-storage-in-package" "Package must not contain generated .nexus-data storage."
    fi
}

echo "=== NexusLang Public Release Install Validation ==="

require_command curl
require_command tar
require_command sha256sum
require_command gpg
require_command awk
require_command sed
require_command grep
require_command wc
require_command python3
require_command node

WORK_DIR="$(mktemp -d /tmp/nexus-public-release-install.XXXXXX)"
DOWNLOAD_DIR="$WORK_DIR/downloads"
GNUPG_HOME="$WORK_DIR/gnupg"
mkdir -p "$DOWNLOAD_DIR" "$GNUPG_HOME"
chmod 700 "$GNUPG_HOME"

report_line "public_install_validation_started_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
report_line "repository=$REPOSITORY"
report_line "tag=$TAG"
report_line "release_url=https://github.com/$REPOSITORY/releases/tag/$TAG"
report_line "base_download_url=$BASE_URL"
report_line "work_dir=$WORK_DIR"

download_asset "$ARCHIVE_NAME"
download_asset "$CHECKSUM_NAME"
download_asset "$ARCHIVE_SIG_NAME"
download_asset "$CHECKSUM_SIG_NAME"
download_asset "$PUBLIC_KEY_NAME"
download_asset "$FINGERPRINT_NAME"

assert_file "$DOWNLOAD_DIR/$ARCHIVE_NAME"
assert_file "$DOWNLOAD_DIR/$CHECKSUM_NAME"
assert_file "$DOWNLOAD_DIR/$ARCHIVE_SIG_NAME"
assert_file "$DOWNLOAD_DIR/$CHECKSUM_SIG_NAME"
assert_file "$DOWNLOAD_DIR/$PUBLIC_KEY_NAME"
assert_file "$DOWNLOAD_DIR/$FINGERPRINT_NAME"

downloaded_fingerprint="$(tr -d '[:space:]' < "$DOWNLOAD_DIR/$FINGERPRINT_NAME")"
[ "$downloaded_fingerprint" = "$EXPECTED_FINGERPRINT" ] || {
    fail "fingerprint-mismatch" "Expected signing fingerprint $EXPECTED_FINGERPRINT but release published $downloaded_fingerprint."
}

run gpg --homedir "$GNUPG_HOME" --batch --import "$DOWNLOAD_DIR/$PUBLIC_KEY_NAME"

imported_fingerprint="$(
    gpg --homedir "$GNUPG_HOME" --batch --with-colons --fingerprint "$EXPECTED_FINGERPRINT" \
        | awk -F ':' '/^fpr:/ { print $10; exit }'
)"

[ "$imported_fingerprint" = "$EXPECTED_FINGERPRINT" ] || {
    fail "public-key-fingerprint-mismatch" "Imported public key fingerprint did not match $EXPECTED_FINGERPRINT."
}

run gpg --homedir "$GNUPG_HOME" --batch --verify "$DOWNLOAD_DIR/$ARCHIVE_SIG_NAME" "$DOWNLOAD_DIR/$ARCHIVE_NAME"
run gpg --homedir "$GNUPG_HOME" --batch --verify "$DOWNLOAD_DIR/$CHECKSUM_SIG_NAME" "$DOWNLOAD_DIR/$CHECKSUM_NAME"

(
    cd "$DOWNLOAD_DIR"
    run sha256sum -c "$CHECKSUM_NAME"
)

run validate_tar_paths

PACKAGE_NAME="$(top_level_package_name)" || fail "package-name-unavailable" "Could not read top-level package name."

case "$PACKAGE_NAME" in
    "nexuslang-$TAG-local-release") ;;
    *) fail "unexpected-package-name" "Unexpected top-level package directory: $PACKAGE_NAME" ;;
esac

run tar -xzf "$DOWNLOAD_DIR/$ARCHIVE_NAME" -C "$WORK_DIR"

PACKAGE_DIR="$WORK_DIR/$PACKAGE_NAME"
[ -d "$PACKAGE_DIR" ] || fail "package-dir-missing" "Package directory missing after extraction."

assert_executable "$PACKAGE_DIR/bin/nexus"
assert_file "$PACKAGE_DIR/PACKAGE_MANIFEST.txt"
assert_file "$PACKAGE_DIR/README.md"
assert_file "$PACKAGE_DIR/docs/README.md"
assert_file "$PACKAGE_DIR/docs/RELEASE_NOTES.md"
assert_file "$PACKAGE_DIR/docs/COMPATIBILITY.md"
assert_file "$PACKAGE_DIR/docs/SIGNING.md"
assert_file "$PACKAGE_DIR/nexuslang-playground.html"
assert_file "$PACKAGE_DIR/nexuslang-playground.js"
assert_file "$PACKAGE_DIR/nexuslang-src/web/nexuslang_playground.wasm"
assert_file "$PACKAGE_DIR/examples/erp_basico.nx"
assert_executable "$PACKAGE_DIR/scripts/smoke-package.sh"
assert_no_generated_storage "$PACKAGE_DIR"

manifest_package="$(manifest_value "$PACKAGE_DIR" package)"
manifest_archive="$(manifest_value "$PACKAGE_DIR" archive)"
manifest_checksum="$(manifest_value "$PACKAGE_DIR" checksum)"
manifest_version="$(manifest_value "$PACKAGE_DIR" package_version)"
manifest_wasm_bytes="$(manifest_value "$PACKAGE_DIR" wasm_bytes)"
actual_wasm_bytes="$(wc -c < "$PACKAGE_DIR/nexuslang-src/web/nexuslang_playground.wasm" | tr -d '[:space:]')"

[ "$manifest_package" = "$PACKAGE_NAME" ] || {
    fail "manifest-package-mismatch" "manifest package=$manifest_package but extracted package=$PACKAGE_NAME"
}
[ "$manifest_archive" = "$ARCHIVE_NAME" ] || {
    fail "manifest-archive-mismatch" "manifest archive=$manifest_archive but expected $ARCHIVE_NAME"
}
[ "$manifest_checksum" = "$CHECKSUM_NAME" ] || {
    fail "manifest-checksum-mismatch" "manifest checksum=$manifest_checksum but expected $CHECKSUM_NAME"
}
[ "$manifest_version" = "$VERSION" ] || {
    fail "manifest-version-mismatch" "manifest package_version=$manifest_version but expected $VERSION"
}
[ "$manifest_wasm_bytes" = "$actual_wasm_bytes" ] || {
    fail "manifest-wasm-size-mismatch" "manifest wasm_bytes=$manifest_wasm_bytes but actual=$actual_wasm_bytes"
}

run "$PACKAGE_DIR/scripts/smoke-package.sh"

echo ""
echo "==> HTTP asset smoke on 127.0.0.1:$PORT"
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$PACKAGE_DIR" \
    >"$WORK_DIR/http.log" 2>&1 &
SERVER_PID="$!"
sleep 1

curl -fsS "http://127.0.0.1:$PORT/nexuslang-playground.html" >/dev/null
curl -fsS "http://127.0.0.1:$PORT/nexuslang-playground.js" >/dev/null
curl -fsS "http://127.0.0.1:$PORT/nexuslang-src/web/nexuslang_playground.wasm" >/dev/null

archive_sha256="$(cut -d ' ' -f 1 "$DOWNLOAD_DIR/$CHECKSUM_NAME")"
archive_bytes="$(wc -c < "$DOWNLOAD_DIR/$ARCHIVE_NAME" | tr -d '[:space:]')"

report_line "archive=$ARCHIVE_NAME"
report_line "archive_bytes=$archive_bytes"
report_line "archive_sha256=$archive_sha256"
report_line "package=$PACKAGE_NAME"
report_line "package_version=$manifest_version"
report_line "wasm_bytes=$actual_wasm_bytes"
report_line "signing_key_fingerprint=$EXPECTED_FINGERPRINT"
report_line "public_install_validation_status=passed"
report_line "public_install_validation_finished_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

echo ""
echo "Public release install validation passed."
echo "Report: $REPORT_PATH"
if [ "$KEEP_WORK_DIR" = "1" ]; then
    echo "Work dir kept at: $WORK_DIR"
fi
