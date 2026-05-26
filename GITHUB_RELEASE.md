# NexusLang GitHub Release Infrastructure

This guide describes the external setup required before NexusLang can call a
release fully public-ready instead of only locally validated.

## Required Setup

1. Put the workspace in a real Git repository.
2. Set `origin` to the public or private GitHub repository that will host the
   release.
3. Authenticate the GitHub CLI:

```bash
gh auth login
```

4. Configure a maintained GPG release key and keep the private key outside the
   repository.
5. Export the key fingerprint for release commands:

```bash
export NEXUS_RELEASE_SIGNING_KEY="<fingerprint-or-key-id>"
```

If the repository slug cannot be inferred from `origin`, set it explicitly.
The `origin` remote is still required because strict mode verifies that the
current commit was pushed:

```bash
export NEXUS_GITHUB_REPOSITORY="owner/repo"
```

## Strict Dry-Run

Run the fast external preflight first:

```bash
./scripts/release-dry-run-strict.sh --preflight-only
```

Then run the full strict dry-run:

```bash
./scripts/release-dry-run-strict.sh
```

Strict mode refuses to continue unless all of these are true:

- the repository has at least one commit;
- the worktree is clean and on a branch;
- the current commit is pushed to `origin/<branch>`;
- `gh` is authenticated and can access the GitHub repository;
- GitHub Actions has at least one successful run for the current commit;
- `NEXUS_RELEASE_SIGNING_KEY` points to a real secret GPG key;
- dry-run ephemeral signing is not used.

## Reports

Strict preflight writes:

```text
dist/release-strict-preflight-report.txt
dist/github-repo-view.json
dist/github-actions-runs-strict.json
```

The full dry-run still writes:

```text
dist/release-dry-run-report.txt
```

The project should only be marked as production/public release ready when the
strict dry-run passes with a maintained signing key and observed remote CI.
