#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"

run() {
    echo ""
    echo "==> $*"
    "$@"
}

run_in_crate() {
    echo ""
    echo "==> (nexuslang-src) $*"
    (cd "$CRATE_DIR" && "$@")
}

echo "=== NexusLang Quality Gate ==="

run_in_crate cargo fmt --check

echo ""
echo "==> (nexuslang-src) cargo check --all-targets with -D warnings"
(
    cd "$CRATE_DIR"
    RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings" cargo check --all-targets
)

if [ "${NEXUS_RUN_CLIPPY:-0}" = "1" ]; then
    if cargo clippy --version >/dev/null 2>&1; then
        run_in_crate cargo clippy --all-targets -- -D warnings
    else
        echo "cargo clippy is required because NEXUS_RUN_CLIPPY=1, but it is not installed."
        echo "Install it with: rustup component add clippy"
        exit 1
    fi
fi

run_in_crate cargo test
run "$ROOT_DIR/scripts/validate-storage-compatibility-policy.sh"
run "$ROOT_DIR/scripts/validate-model-operation-contract-docs.sh"
run node --check "$ROOT_DIR/nexuslang-playground.js"
run "$ROOT_DIR/scripts/smoke-test.sh"
run "$ROOT_DIR/scripts/smoke-auth.sh"
run "$ROOT_DIR/scripts/smoke-storage-backup-restore.sh"
run "$ROOT_DIR/scripts/validate-openapi.sh"

echo ""
echo "Quality gate passed."
