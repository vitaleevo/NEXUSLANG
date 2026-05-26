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
ARCHIVE_NAME="$(basename "$ARCHIVE_PATH")"
CHECKSUM_NAME="$ARCHIVE_NAME.sha256"
PACKAGE_NAME="${ARCHIVE_NAME%.tar.gz}"
IMAGE="${NEXUS_RELEASE_SECOND_ENV_IMAGE:-ruby:3.3-bookworm}"

[ -f "$ARCHIVE_PATH" ] || {
    echo "Archive not found: $ARCHIVE_PATH" >&2
    exit 1
}

[ -f "$ARCHIVE_PATH.sha256" ] || {
    echo "Checksum not found: $ARCHIVE_PATH.sha256" >&2
    exit 1
}

command -v docker >/dev/null 2>&1 || {
    echo "Docker is required for second-environment validation." >&2
    exit 1
}

echo "=== NexusLang Second Environment Validation ==="
echo "Image: $IMAGE"
echo "Archive: $ARCHIVE_NAME"

docker run --rm \
    -e ARCHIVE_NAME="$ARCHIVE_NAME" \
    -e CHECKSUM_NAME="$CHECKSUM_NAME" \
    -e PACKAGE_NAME="$PACKAGE_NAME" \
    -v "$(dirname "$ARCHIVE_PATH"):/release:ro" \
    "$IMAGE" \
    bash -lc '
        set -euo pipefail

        mkdir -p /tmp/nexus-second-env
        cd /tmp/nexus-second-env

        cp "/release/$ARCHIVE_NAME" .
        cp "/release/$CHECKSUM_NAME" .

        sha256sum -c "$CHECKSUM_NAME"
        tar -xzf "$ARCHIVE_NAME"
        cd "$PACKAGE_NAME"

        test -x bin/nexus
        test -f PACKAGE_MANIFEST.txt
        test -f docs/README.md
        test -f docs/RELEASE_NOTES.md
        test -f docs/VERSIONING.md
        test -f docs/COMPATIBILITY.md
        test -f docs/STORAGE_BACKUP_RESTORE.md
        test -f docs/SIGNING.md
        test -x scripts/validate-storage-compatibility-policy.sh
        test -x scripts/smoke-storage-backup-restore.sh
        test -f nexuslang-playground.html
        test -f nexuslang-playground.js
        test -s nexuslang-src/web/nexuslang_playground.wasm

        bin/nexus --help >/tmp/nexus-help.out
        bin/nexus check examples/erp_basico.nx >/tmp/nexus-check.out
        bin/nexus check examples/storage_backup_restore_inventory.nx >/tmp/nexus-storage-check.out
        bin/nexus run examples/erp_basico.nx >/tmp/nexus-run.out
        NEXUS_STORAGE_SMOKE_PORT=8094 scripts/smoke-storage-backup-restore.sh

        python3 -m http.server 8093 --bind 127.0.0.1 >/tmp/nexus-http.out 2>&1 &
        server_pid="$!"
        trap "kill \"$server_pid\" 2>/dev/null || true" EXIT
        sleep 1

        curl -fsS http://127.0.0.1:8093/nexuslang-playground.html >/dev/null
        curl -fsS http://127.0.0.1:8093/nexuslang-playground.js >/dev/null
        curl -fsS http://127.0.0.1:8093/nexuslang-src/web/nexuslang_playground.wasm >/dev/null

        echo "Second environment validation passed."
    '
