#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
REPORT_PATH="$DIST_DIR/release-dry-run-report.txt"
SIGNATURE_MODE="${NEXUS_RELEASE_SIGNATURE_MODE:-auto}"
REQUIRE_REMOTE_CI="${NEXUS_RELEASE_REQUIRE_REMOTE_CI:-0}"
DRY_RUN_SIGNING_USER="NexusLang Release Dry Run <dry-run@nexuslang.local>"

run() {
    echo ""
    echo "==> $*"
    "$@"
}

report_line() {
    echo "$*" | tee -a "$REPORT_PATH" >/dev/null
}

default_archive_path() {
    local matches=("$DIST_DIR"/nexuslang-v*-local-release.tar.gz)

    if [ -e "${matches[0]}" ]; then
        printf '%s\n' "${matches[@]}" | sort -V | tail -n 1
        return
    fi

    echo "$DIST_DIR/nexuslang-local-release.tar.gz"
}

has_secret_keys() {
    gpg --list-secret-keys --with-colons 2>/dev/null | grep -q "^sec"
}

latest_archive_sha256() {
    local archive_path="$1"
    cut -d " " -f 1 "$archive_path.sha256"
}

normalize_github_repo() {
    local value="$1"

    value="${value#https://github.com/}"
    value="${value#http://github.com/}"
    value="${value#git@github.com:}"
    value="${value#ssh://git@github.com/}"
    value="${value%.git}"

    case "$value" in
        */*) printf '%s\n' "$value" ;;
        *) return 1 ;;
    esac
}

sign_with_existing_key() {
    local archive_path="$1"

    run "$ROOT_DIR/scripts/sign-release-artifacts.sh" "$archive_path"
    echo "signed-existing-key"
}

sign_with_ephemeral_key() {
    local archive_path="$1"
    local work_dir
    local fingerprint
    local public_key_path

    work_dir="$(mktemp -d /tmp/nexus-release-dry-run-gpg.XXXXXX)"
    chmod 700 "$work_dir"

    GNUPGHOME="$work_dir" gpg --batch --pinentry-mode loopback --passphrase "" \
        --quick-generate-key "$DRY_RUN_SIGNING_USER" ed25519 sign 1d >/dev/null 2>&1

    fingerprint="$(
        GNUPGHOME="$work_dir" gpg --with-colons --list-secret-keys "$DRY_RUN_SIGNING_USER" \
            | awk -F ":" "/^fpr:/ { print \$10; exit }"
    )"

    [ -n "$fingerprint" ] || {
        echo "Could not create dry-run GPG key." >&2
        rm -rf "$work_dir"
        exit 1
    }

    run env GNUPGHOME="$work_dir" NEXUS_RELEASE_SIGNING_KEY="$fingerprint" \
        "$ROOT_DIR/scripts/sign-release-artifacts.sh" "$archive_path"

    public_key_path="$archive_path.dry-run-public-key.asc"
    GNUPGHOME="$work_dir" gpg --armor --export "$fingerprint" > "$public_key_path"
    rm -rf "$work_dir"

    echo "signed-ephemeral-dry-run"
}

observe_remote_ci() {
    local repo
    local remote_url
    local head_sha

    if ! git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        echo "not-observed:no-git-repository"
        return
    fi

    if ! command -v gh >/dev/null 2>&1; then
        echo "not-observed:gh-missing"
        return
    fi

    if ! gh auth status >/dev/null 2>&1; then
        echo "not-observed:gh-not-authenticated"
        return
    fi

    repo="${NEXUS_GITHUB_REPOSITORY:-}"

    if [ -z "$repo" ]; then
        remote_url="$(git -C "$ROOT_DIR" remote get-url origin 2>/dev/null || true)"
        repo="$(normalize_github_repo "$remote_url" || true)"
    else
        repo="$(normalize_github_repo "$repo" || true)"
    fi

    if [ -z "$repo" ]; then
        echo "not-observed:no-github-remote"
        return
    fi

    head_sha="$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || true)"

    if gh -R "$repo" run list --commit "$head_sha" --limit 5 \
        --json databaseId,status,conclusion,headSha,workflowName,createdAt,url \
        > "$DIST_DIR/github-actions-runs.txt" 2>"$DIST_DIR/github-actions-runs.err"; then
        if grep -q "\"databaseId\"" "$DIST_DIR/github-actions-runs.txt"; then
            echo "observed:github-actions-runs-for-head"
        else
            echo "not-observed:no-runs-for-head"
        fi
    else
        echo "not-observed:gh-run-list-failed"
    fi
}

mkdir -p "$DIST_DIR"
: > "$REPORT_PATH"

echo "=== NexusLang Final Release Dry Run ==="
report_line "release_dry_run_started_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

run bash -n "$ROOT_DIR/scripts/package-release.sh"
run bash -n "$ROOT_DIR/scripts/validate-release-package.sh"
run bash -n "$ROOT_DIR/scripts/validate-release-second-env.sh"
run bash -n "$ROOT_DIR/scripts/sign-release-artifacts.sh"

run env NEXUS_RUN_CLIPPY=1 "$ROOT_DIR/scripts/quality-gate.sh"
run "$ROOT_DIR/scripts/package-release.sh"

ARCHIVE_PATH="$(default_archive_path)"
ARCHIVE_NAME="$(basename "$ARCHIVE_PATH")"

run "$ROOT_DIR/scripts/validate-release-package.sh" "$ARCHIVE_PATH"
run "$ROOT_DIR/scripts/validate-release-second-env.sh" "$ARCHIVE_PATH"

case "$SIGNATURE_MODE" in
    auto)
        if has_secret_keys; then
            signing_status="$(sign_with_existing_key "$ARCHIVE_PATH" | tail -n 1)"
        else
            signing_status="$(sign_with_ephemeral_key "$ARCHIVE_PATH" | tail -n 1)"
        fi
        ;;
    existing)
        signing_status="$(sign_with_existing_key "$ARCHIVE_PATH" | tail -n 1)"
        ;;
    ephemeral)
        signing_status="$(sign_with_ephemeral_key "$ARCHIVE_PATH" | tail -n 1)"
        ;;
    skip)
        signing_status="skipped"
        ;;
    *)
        echo "Unknown NEXUS_RELEASE_SIGNATURE_MODE: $SIGNATURE_MODE" >&2
        exit 1
        ;;
esac

ci_status="$(observe_remote_ci)"

if [ "$REQUIRE_REMOTE_CI" = "1" ] && [ "${ci_status%%:*}" != "observed" ]; then
    echo "Remote CI observation required but unavailable: $ci_status" >&2
    exit 1
fi

report_line "archive=$ARCHIVE_NAME"
report_line "sha256=$(latest_archive_sha256 "$ARCHIVE_PATH")"
report_line "signing_status=$signing_status"
report_line "remote_ci_status=$ci_status"
report_line "second_environment=docker:${NEXUS_RELEASE_SECOND_ENV_IMAGE:-ruby:3.3-bookworm}"
report_line "release_dry_run_finished_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

echo ""
echo "Release dry-run report:"
cat "$REPORT_PATH"
