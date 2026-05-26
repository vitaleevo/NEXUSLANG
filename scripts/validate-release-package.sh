#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
default_archive_path() {
    local matches=("$ROOT_DIR"/dist/nexuslang-v*-local-release.tar.gz)

    if [ -e "${matches[0]}" ]; then
        printf '%s\n' "${matches[@]}" | sort -V | tail -n 1
        return
    fi

    echo "$ROOT_DIR/dist/nexuslang-local-release.tar.gz"
}

ARCHIVE_PATH="${1:-$(default_archive_path)}"
PORT="${NEXUS_RELEASE_VALIDATE_PORT:-8092}"
WORK_DIR=""
SERVER_PID=""
PACKAGE_NAME=""

fail() {
    echo "ERROR: $*" >&2
    exit 1
}

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi

    if [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then
        case "$WORK_DIR" in
            /tmp/nexus-release-validate.*) rm -rf "$WORK_DIR" ;;
        esac
    fi
}

run() {
    echo ""
    echo "==> $*"
    "$@"
}

assert_file() {
    [ -f "$1" ] || fail "missing file: $1"
}

assert_executable() {
    [ -x "$1" ] || fail "missing executable: $1"
}

assert_no_generated_storage() {
    if find "$1" -name ".nexus-data" -print -quit | grep -q .; then
        fail "package must not contain generated .nexus-data storage"
    fi
}

validate_tar_paths() {
    tar -tzf "$ARCHIVE_PATH" | while IFS= read -r entry; do
        case "$entry" in
            /* | *../*) fail "unsafe archive path: $entry" ;;
        esac
    done
}

manifest_value() {
    local name="$1"
    sed -n "s/^$name=//p" "$PACKAGE_DIR/PACKAGE_MANIFEST.txt" | head -n 1
}

validate_archive_checksum() {
    local checksum_path="$ARCHIVE_PATH.sha256"
    local checksum_name

    [ -f "$checksum_path" ] || fail "checksum file not found: $checksum_path"
    checksum_name="$(basename "$checksum_path")"

    (
        cd "$(dirname "$ARCHIVE_PATH")"
        sha256sum -c "$checksum_name"
    )
}

trap cleanup EXIT

[ -f "$ARCHIVE_PATH" ] || fail "archive not found: $ARCHIVE_PATH"

echo "=== NexusLang Clean Release Package Validation ==="
run validate_tar_paths
run validate_archive_checksum

PACKAGE_NAME="$(
    tar -tzf "$ARCHIVE_PATH" | awk -F '/' '
        NF && $1 != "" && found == 0 {
            print $1
            found = 1
        }
        END {
            exit found ? 0 : 1
        }
    '
)"

case "$PACKAGE_NAME" in
    nexuslang-v*-local-release | nexuslang-local-release) ;;
    *) fail "unexpected top-level package directory: $PACKAGE_NAME" ;;
esac

WORK_DIR="$(mktemp -d /tmp/nexus-release-validate.XXXXXX)"
run tar -xzf "$ARCHIVE_PATH" -C "$WORK_DIR"

PACKAGE_DIR="$WORK_DIR/$PACKAGE_NAME"
[ -d "$PACKAGE_DIR" ] || fail "package directory missing after extraction"

assert_executable "$PACKAGE_DIR/bin/nexus"
assert_file "$PACKAGE_DIR/nexuslang-playground.html"
assert_file "$PACKAGE_DIR/nexuslang-playground.js"
assert_file "$PACKAGE_DIR/nexuslang-src/web/nexuslang_playground.wasm"
assert_file "$PACKAGE_DIR/README.md"
assert_file "$PACKAGE_DIR/PACKAGE_MANIFEST.txt"
assert_file "$PACKAGE_DIR/docs/README.md"
assert_file "$PACKAGE_DIR/docs/RELEASE_NOTES.md"
assert_file "$PACKAGE_DIR/docs/VERSIONING.md"
assert_file "$PACKAGE_DIR/docs/COMPATIBILITY.md"
assert_file "$PACKAGE_DIR/docs/SIGNING.md"
assert_file "$PACKAGE_DIR/docs/GITHUB_RELEASE.md"
assert_file "$PACKAGE_DIR/docs/RELEASE.md"
assert_file "$PACKAGE_DIR/examples/erp_basico.nx"
assert_executable "$PACKAGE_DIR/scripts/smoke-package.sh"
assert_executable "$PACKAGE_DIR/scripts/validate-release-second-env.sh"
assert_executable "$PACKAGE_DIR/scripts/sign-release-artifacts.sh"
assert_executable "$PACKAGE_DIR/scripts/release-dry-run-strict.sh"
assert_no_generated_storage "$PACKAGE_DIR"

manifest_package="$(manifest_value package)"
manifest_archive="$(manifest_value archive)"
manifest_checksum="$(manifest_value checksum)"
manifest_version="$(manifest_value package_version)"

[ "$manifest_package" = "$PACKAGE_NAME" ] || {
    fail "manifest package=$manifest_package but extracted package=$PACKAGE_NAME"
}
[ "$manifest_archive" = "$(basename "$ARCHIVE_PATH")" ] || {
    fail "manifest archive=$manifest_archive but archive=$(basename "$ARCHIVE_PATH")"
}
[ "$manifest_checksum" = "$(basename "$ARCHIVE_PATH").sha256" ] || {
    fail "manifest checksum=$manifest_checksum but expected $(basename "$ARCHIVE_PATH").sha256"
}
[ -n "$manifest_version" ] || fail "manifest missing package_version"

run "$PACKAGE_DIR/scripts/smoke-package.sh"

manifest_wasm_bytes="$(
    sed -n 's/^wasm_bytes=//p' "$PACKAGE_DIR/PACKAGE_MANIFEST.txt" | head -n 1
)"
actual_wasm_bytes="$(wc -c < "$PACKAGE_DIR/nexuslang-src/web/nexuslang_playground.wasm" | tr -d '[:space:]')"

[ -n "$manifest_wasm_bytes" ] || fail "manifest missing wasm_bytes"
[ "$manifest_wasm_bytes" = "$actual_wasm_bytes" ] || {
    fail "manifest wasm_bytes=$manifest_wasm_bytes but actual=$actual_wasm_bytes"
}

echo ""
echo "==> HTTP asset smoke on 127.0.0.1:$PORT"
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$PACKAGE_DIR" \
    >"$WORK_DIR/http.log" 2>&1 &
SERVER_PID="$!"
sleep 1

curl -fsS "http://127.0.0.1:$PORT/nexuslang-playground.html" >/dev/null
curl -fsS "http://127.0.0.1:$PORT/nexuslang-playground.js" >/dev/null
curl -fsS "http://127.0.0.1:$PORT/nexuslang-src/web/nexuslang_playground.wasm" >/dev/null

echo ""
echo "Clean package validation passed."
echo "Extracted package was validated in: $WORK_DIR"
