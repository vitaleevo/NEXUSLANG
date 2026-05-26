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

## Connect Origin

After `gh auth login`, connect the local repo to GitHub:

```bash
./scripts/connect-github-release.sh --repo vitaleevo/nexuslang --create --push
```

Use `--private` if the repository should start private:

```bash
./scripts/connect-github-release.sh --repo vitaleevo/nexuslang --create --private --push
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

## Current Status

The strict dry-run has passed for the NexusLang 0.1.0 release scope using:

```text
Repository: https://github.com/vitaleevo/NEXUSLANG
Release tag: v0.1.0
Signing key: 3237F7CC5CE2514FC9671BB93CB6808B55385273
```

The `v0.1.1` release was published from:

```text
c302f346e6ec2c17565daa3b1a69ff0e986533d5
```

The `0.1.1` release changes were committed, pushed, observed in GitHub Actions,
and validated with:

```bash
NEXUS_RELEASE_SIGNING_KEY="3237F7CC5CE2514FC9671BB93CB6808B55385273" ./scripts/release-dry-run-strict.sh
```

After the GitHub Release is published, validate the public install path:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh
```
