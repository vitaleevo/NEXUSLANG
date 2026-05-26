#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
REPORT_PATH="${NEXUS_RELEASE_STRICT_REPORT:-$DIST_DIR/release-strict-preflight-report.txt}"
SIGNING_KEY="${NEXUS_RELEASE_SIGNING_KEY:-}"
MIN_SUCCESSFUL_RUNS="${NEXUS_RELEASE_REQUIRED_SUCCESSFUL_CI_RUNS:-1}"
PREFLIGHT_ONLY=0

usage() {
    cat <<'EOF'
Usage: scripts/release-dry-run-strict.sh [--preflight-only]

Runs the NexusLang release dry-run in strict public-release mode.

Required environment:
  NEXUS_RELEASE_SIGNING_KEY=<fingerprint-or-key-id>

Optional environment:
  NEXUS_GITHUB_REPOSITORY=owner/repo
  NEXUS_RELEASE_REQUIRED_SUCCESSFUL_CI_RUNS=1

Strict mode requires:
  - a Git worktree with a clean checked-out branch;
  - an origin remote on GitHub;
  - NEXUS_GITHUB_REPOSITORY=owner/repo when the slug cannot be inferred;
  - gh authenticated against GitHub;
  - the current HEAD pushed to origin/<branch>;
  - at least one successful GitHub Actions run for the current HEAD;
  - an explicit non-dry-run GPG secret key.
EOF
}

while [ "${1:-}" != "" ]; do
    case "$1" in
        --preflight-only)
            PREFLIGHT_ONLY=1
            shift
            ;;
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

run() {
    echo ""
    echo "==> $*"
    "$@"
}

report_line() {
    echo "$*" | tee -a "$REPORT_PATH" >/dev/null
}

fail() {
    local code="$1"
    local message="$2"

    report_line "strict_status=failed:$code"
    echo "ERROR: $message" >&2
    echo "Strict preflight report: $REPORT_PATH" >&2
    exit 1
}

require_command() {
    local name="$1"

    command -v "$name" >/dev/null 2>&1 || fail "missing-$name" "$name is required."
}

normalize_github_repo() {
    local value="$1"

    value="${value#https://github.com/}"
    value="${value#http://github.com/}"
    value="${value#git@github.com:}"
    value="${value#ssh://git@github.com/}"
    value="${value%.git}"

    case "$value" in
        */*)
            printf '%s\n' "$value"
            ;;
        *)
            return 1
            ;;
    esac
}

report_line "strict_preflight_started_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

require_command git
require_command gh
require_command gpg
require_command python3

git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
    fail "no-git-repository" "$ROOT_DIR is not a Git worktree."
}

GIT_ROOT="$(git -C "$ROOT_DIR" rev-parse --show-toplevel)"

git -C "$ROOT_DIR" rev-parse --verify HEAD >/dev/null 2>&1 || {
    fail "no-commits" "Create and push an initial commit before strict release validation."
}

HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
BRANCH_NAME="$(git -C "$ROOT_DIR" rev-parse --abbrev-ref HEAD)"

[ "$BRANCH_NAME" != "HEAD" ] || {
    fail "detached-head" "A checked-out branch is required for strict release validation."
}

if [ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=normal)" ]; then
    fail "dirty-worktree" "Commit or stash local changes before strict release validation."
fi

REMOTE_URL="$(git -C "$ROOT_DIR" remote get-url origin 2>/dev/null || true)"
[ -n "$REMOTE_URL" ] || {
    fail "no-origin-remote" "Set origin to the GitHub repository before strict release validation."
}

GITHUB_REPO="${NEXUS_GITHUB_REPOSITORY:-}"

if [ -z "$GITHUB_REPO" ] && [ -n "$REMOTE_URL" ]; then
    GITHUB_REPO="$(normalize_github_repo "$REMOTE_URL" || true)"
fi

[ -n "$GITHUB_REPO" ] || {
    fail "no-github-remote" "Set origin to a GitHub remote or export NEXUS_GITHUB_REPOSITORY=owner/repo."
}

GITHUB_REPO="$(normalize_github_repo "$GITHUB_REPO" || true)"
[ -n "$GITHUB_REPO" ] || {
    fail "invalid-github-repo" "GitHub repository must be owner/repo or a github.com URL."
}

gh auth status -h github.com >/dev/null 2>&1 || {
    fail "gh-not-authenticated" "Run gh auth login before strict release validation."
}

GH_USER="$(gh api user --jq .login 2>/dev/null || true)"
[ -n "$GH_USER" ] || fail "gh-user-unavailable" "Could not read the authenticated GitHub user."

gh repo view "$GITHUB_REPO" --json nameWithOwner,url,defaultBranchRef \
    > "$DIST_DIR/github-repo-view.json" 2>"$DIST_DIR/github-repo-view.err" || {
        fail "github-repo-unavailable" "Could not access GitHub repository $GITHUB_REPO."
    }

REMOTE_HEAD="$(git -C "$ROOT_DIR" ls-remote --heads origin "$BRANCH_NAME" | awk '{ print $1; exit }')"
[ "$REMOTE_HEAD" = "$HEAD_SHA" ] || {
    fail "head-not-pushed" "Current HEAD is not pushed to origin/$BRANCH_NAME."
}

[ -n "$SIGNING_KEY" ] || {
    fail "missing-signing-key" "Set NEXUS_RELEASE_SIGNING_KEY to a maintained release key fingerprint."
}

KEY_INFO="$(gpg --with-colons --list-secret-keys "$SIGNING_KEY" 2>/dev/null || true)"
echo "$KEY_INFO" | grep -q "^sec" || {
    fail "gpg-key-not-found" "Could not find a secret GPG key for NEXUS_RELEASE_SIGNING_KEY."
}

if echo "$KEY_INFO" | grep -qi "dry-run@nexuslang.local"; then
    fail "dry-run-key-not-allowed" "Dry-run GPG keys are not accepted in strict release mode."
fi

SIGNING_FINGERPRINT="$(echo "$KEY_INFO" | awk -F ":" '/^fpr:/ { print $10; exit }')"
[ -n "$SIGNING_FINGERPRINT" ] || {
    fail "gpg-fingerprint-unavailable" "Could not read the signing key fingerprint."
}

RUNS_JSON="$DIST_DIR/github-actions-runs-strict.json"
gh run list -R "$GITHUB_REPO" --commit "$HEAD_SHA" --limit 20 \
    --json databaseId,status,conclusion,headSha,workflowName,createdAt,url \
    > "$RUNS_JSON" 2>"$DIST_DIR/github-actions-runs-strict.err" || {
        fail "github-actions-unavailable" "Could not list GitHub Actions runs for $HEAD_SHA."
    }

if ! SUCCESSFUL_RUNS="$(
    python3 - "$RUNS_JSON" "$MIN_SUCCESSFUL_RUNS" <<'PY'
import json
import sys

path = sys.argv[1]
minimum = int(sys.argv[2])

with open(path, "r", encoding="utf-8") as handle:
    runs = json.load(handle)

successes = [
    run for run in runs
    if run.get("status") == "completed" and run.get("conclusion") == "success"
]

print(len(successes))
raise SystemExit(0 if len(successes) >= minimum else 1)
PY
)"; then
    fail "no-successful-ci-for-head" "No successful GitHub Actions run was found for $HEAD_SHA."
fi

report_line "git_root=$GIT_ROOT"
report_line "git_branch=$BRANCH_NAME"
report_line "git_head=$HEAD_SHA"
report_line "github_repository=$GITHUB_REPO"
report_line "github_user=$GH_USER"
report_line "signing_key_fingerprint=$SIGNING_FINGERPRINT"
report_line "successful_ci_runs_for_head=$SUCCESSFUL_RUNS"
report_line "strict_preflight_status=passed"

if [ "$PREFLIGHT_ONLY" = "1" ]; then
    report_line "strict_preflight_finished_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo ""
    echo "Strict release preflight passed."
    echo "Report: $REPORT_PATH"
    exit 0
fi

if ! run env \
    NEXUS_GITHUB_REPOSITORY="$GITHUB_REPO" \
    NEXUS_RELEASE_SIGNATURE_MODE=existing \
    NEXUS_RELEASE_REQUIRE_REMOTE_CI=1 \
    NEXUS_RELEASE_SIGNING_KEY="$SIGNING_KEY" \
    "$ROOT_DIR/scripts/release-dry-run.sh"; then
    fail "release-dry-run-failed" "Strict release dry-run failed after preflight."
fi

report_line "strict_status=passed"
report_line "strict_preflight_finished_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

echo ""
echo "Strict release dry-run passed."
echo "Report: $REPORT_PATH"
