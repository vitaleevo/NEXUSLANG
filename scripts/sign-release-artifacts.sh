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

usage() {
    cat <<'EOF'
Usage: scripts/sign-release-artifacts.sh [archive.tar.gz]

Signs a NexusLang release archive and its .sha256 file using GPG detached ASCII
signatures. Set NEXUS_RELEASE_SIGNING_KEY to select a key.
EOF
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    exit 0
fi

ARCHIVE_PATH="${1:-$(default_archive_path)}"
CHECKSUM_PATH="$ARCHIVE_PATH.sha256"
GPG_BIN="${GPG_BIN:-gpg}"
SIGNING_KEY="${NEXUS_RELEASE_SIGNING_KEY:-}"

run() {
    echo ""
    echo "==> $*"
    "$@"
}

sign_file() {
    local file_path="$1"
    local signature_path="$file_path.asc"

    rm -f "$signature_path"

    if [ -n "$SIGNING_KEY" ]; then
        run "$GPG_BIN" --batch --yes --armor --detach-sign \
            --local-user "$SIGNING_KEY" \
            --output "$signature_path" \
            "$file_path"
    else
        run "$GPG_BIN" --batch --yes --armor --detach-sign \
            --output "$signature_path" \
            "$file_path"
    fi

    run "$GPG_BIN" --verify "$signature_path" "$file_path"
}

[ -f "$ARCHIVE_PATH" ] || {
    echo "Archive not found: $ARCHIVE_PATH" >&2
    exit 1
}

[ -f "$CHECKSUM_PATH" ] || {
    echo "Checksum not found: $CHECKSUM_PATH" >&2
    exit 1
}

command -v "$GPG_BIN" >/dev/null 2>&1 || {
    echo "GPG binary not found: $GPG_BIN" >&2
    exit 1
}

echo "=== NexusLang Release Artifact Signing ==="
(
    cd "$(dirname "$ARCHIVE_PATH")"
    run sha256sum -c "$(basename "$CHECKSUM_PATH")"
)
sign_file "$ARCHIVE_PATH"
sign_file "$CHECKSUM_PATH"

echo ""
echo "Signatures ready:"
echo "  $ARCHIVE_PATH.asc"
echo "  $CHECKSUM_PATH.asc"
