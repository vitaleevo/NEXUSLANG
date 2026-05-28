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

The `0.2.0-rc.1` candidate was prepared from PR:

```text
https://github.com/vitaleevo/NEXUSLANG/pull/1
```

For this RC line, the branch was pushed, GitHub Actions `NexusLang Quality Gate`
passed, and strict public-release dry-run passed with the maintained signing key
above. The signed annotated tag `v0.2.0-rc.1` exists, and the GitHub Release is
published as a public pre-release, not as the latest stable release.

The public pre-release install path passed with:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.1 ./scripts/validate-public-release-install.sh
```

Published RC assets include:

- `nexuslang-v0.2.0-rc.1-local-release.tar.gz`
- `nexuslang-v0.2.0-rc.1-local-release.tar.gz.sha256`
- `nexuslang-v0.2.0-rc.1-local-release.tar.gz.asc`
- `nexuslang-v0.2.0-rc.1-local-release.tar.gz.sha256.asc`
- `nexuslang-release-public-key.asc`
- `nexuslang-release-signing-key.fingerprint`

The validated public archive SHA-256, rechecked against
`dist/nexuslang-v0.2.0-rc.1-local-release.tar.gz` after publication, is:

```text
3d1f376e81aa855c69db3da70674811098169d3aaec8d19cbf50fc36bcbe91d5
```

Earlier local-only draft artifacts may have different checksums; the value
above is the checksum for the published `v0.2.0-rc.1` pre-release asset.

The post-merge `0.2.0-rc.2` candidate starts from the validated `main` line
after PR #1 and carries only the version/docs changes needed to publish the
post-feedback code as a new signed public pre-release. It remains a pre-release
and did not replace `v0.1.1` as the latest stable release.

The signed annotated tag `v0.2.0-rc.2` points to:

```text
5561a2484e7f5082b9d339f94b02ee5dd8d77be0
```

The `v0.2.0-rc.2` GitHub Release is published as a public pre-release:

```text
https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.2.0-rc.2
```

Published RC2 assets include:

- `nexuslang-v0.2.0-rc.2-local-release.tar.gz`
- `nexuslang-v0.2.0-rc.2-local-release.tar.gz.sha256`
- `nexuslang-v0.2.0-rc.2-local-release.tar.gz.asc`
- `nexuslang-v0.2.0-rc.2-local-release.tar.gz.sha256.asc`
- `nexuslang-release-public-key.asc`
- `nexuslang-release-signing-key.fingerprint`

The validated public archive SHA-256 is:

```text
8ed601c2751e86ca84c40cbbd0edec9b4f1266d3663299fd83e8b2b4912eea0b
```

Public RC2 validation command:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh
```

The stable `0.2.0` release is prepared from the post-hardening `main` line. It
bumped the source version from `0.2.0-rc.2` to `0.2.0`, keeps the RC2 feature
surface, and preserves the documented limits for registry dependencies,
SQLite physical schema, LSP editor features, and hosted playground.

Do not create or publish a replacement `v0.2.0` artifact unless the release
head has passed:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
./scripts/package-release.sh
./scripts/validate-release-package.sh
NEXUS_RELEASE_SIGNING_KEY=3237F7CC5CE2514FC9671BB93CB6808B55385273 ./scripts/release-dry-run-strict.sh
```

After publication, run:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh
```
