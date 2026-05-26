#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
REPORT_PATH="$DIST_DIR/github-release-connect-report.txt"
REPOSITORY="${NEXUS_GITHUB_REPOSITORY:-vitaleevo/nexuslang}"
CREATE_REPO=0
PUSH_MAIN=0
VISIBILITY="public"

usage() {
    cat <<'EOF'
Usage: scripts/connect-github-release.sh [--repo owner/name] [--create] [--private] [--push]

Connects the local NexusLang Git repository to the GitHub release repository.

Defaults:
  --repo vitaleevo/nexuslang

Options:
  --create   Create the repository with gh if it does not exist.
  --private  Create the repository as private when used with --create.
  --push     Push the current main branch after configuring origin.

This script requires gh authentication. Run gh auth login first.
EOF
}

while [ "${1:-}" != "" ]; do
    case "$1" in
        --repo)
            [ -n "${2:-}" ] || {
                echo "--repo requires owner/name" >&2
                exit 1
            }
            REPOSITORY="$2"
            shift 2
            ;;
        --create)
            CREATE_REPO=1
            shift
            ;;
        --private)
            VISIBILITY="private"
            shift
            ;;
        --push)
            PUSH_MAIN=1
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

report_line() {
    echo "$*" | tee -a "$REPORT_PATH" >/dev/null
}

fail() {
    local code="$1"
    local message="$2"

    report_line "connect_status=failed:$code"
    echo "ERROR: $message" >&2
    echo "Connection report: $REPORT_PATH" >&2
    exit 1
}

run() {
    echo ""
    echo "==> $*"
    "$@"
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

report_line "connect_started_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

command -v git >/dev/null 2>&1 || fail "git-missing" "git is required."
command -v gh >/dev/null 2>&1 || fail "gh-missing" "gh is required."

git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
    fail "no-git-repository" "$ROOT_DIR is not a Git worktree."
}

git -C "$ROOT_DIR" rev-parse --verify HEAD >/dev/null 2>&1 || {
    fail "no-commits" "Create an initial commit before connecting GitHub."
}

BRANCH_NAME="$(git -C "$ROOT_DIR" rev-parse --abbrev-ref HEAD)"
[ "$BRANCH_NAME" = "main" ] || {
    fail "not-main" "Checkout main before connecting the release repository."
}

if [ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=normal)" ]; then
    fail "dirty-worktree" "Commit or stash local changes before connecting GitHub."
fi

REPOSITORY="$(normalize_github_repo "$REPOSITORY" || true)"
[ -n "$REPOSITORY" ] || fail "invalid-repository" "Repository must be owner/name."

gh auth status -h github.com >/dev/null 2>&1 || {
    fail "gh-not-authenticated" "Run gh auth login before connecting GitHub."
}

if gh repo view "$REPOSITORY" --json nameWithOwner,url,visibility \
    > "$DIST_DIR/github-release-repo-view.json" 2>"$DIST_DIR/github-release-repo-view.err"; then
    repo_status="exists"
else
    if [ "$CREATE_REPO" != "1" ]; then
        fail "repo-not-found" "Repository $REPOSITORY was not found. Re-run with --create or choose an existing repo."
    fi

    visibility_flag="--public"
    [ "$VISIBILITY" = "private" ] && visibility_flag="--private"
    run gh repo create "$REPOSITORY" "$visibility_flag" --confirm
    repo_status="created"
fi

REMOTE_URL="https://github.com/$REPOSITORY.git"

if git -C "$ROOT_DIR" remote get-url origin >/dev/null 2>&1; then
    run git -C "$ROOT_DIR" remote set-url origin "$REMOTE_URL"
else
    run git -C "$ROOT_DIR" remote add origin "$REMOTE_URL"
fi

if [ "$PUSH_MAIN" = "1" ]; then
    run git -C "$ROOT_DIR" push -u origin main
    push_status="pushed"
else
    push_status="skipped"
fi

report_line "repository=$REPOSITORY"
report_line "repository_status=$repo_status"
report_line "origin=$REMOTE_URL"
report_line "push_status=$push_status"
report_line "connect_status=passed"
report_line "connect_finished_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

echo ""
echo "GitHub release connection report:"
cat "$REPORT_PATH"
