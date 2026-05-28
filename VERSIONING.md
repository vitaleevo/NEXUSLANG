# NexusLang Versioning And Tag Policy

This document defines how NexusLang versions, source tags, and local release
artifacts are named.

## Source Of Truth

The canonical project version is the `version` field in:

```text
nexuslang-src/Cargo.toml
```

Release package names are derived from that value:

```text
dist/nexuslang-v<version>-local-release.tar.gz
dist/nexuslang-v<version>-local-release.tar.gz.sha256
```

The package manifest records the same version as `package_version`.

## Tag Format

Public source tags should use:

```text
v<version>
```

Example:

```text
v0.2.0-rc.2
```

Tags should point to the commit that produced the release artifacts and passed
the full quality gate.

## Pre-1.0 Policy

NexusLang is currently `0.x`. Until `1.0.0`, compatibility is release-candidate
quality rather than fully stable.

- Patch releases, such as `0.1.1`, should be bug fixes, docs, tests, packaging,
  or narrow behavior fixes with no intentional breaking changes.
- Minor releases, such as `0.2.0`, may refine language/runtime contracts when
  needed, but must document breaking changes in `RELEASE_NOTES.md`.
- Release-candidate tags, such as `v0.2.0-rc.2`, may be used before a minor
  line is declared stable enough for a final public release.
- The package format should remain compatible within a minor line unless the
  release notes explicitly say otherwise.

## 1.0 And Later

After `1.0.0`, use semantic versioning for public contracts:

- `MAJOR`: breaking changes to stable language syntax, stable CLI behavior,
  stable runtime contracts, stable storage compatibility, or release package
  layout.
- `MINOR`: backwards-compatible features and additions.
- `PATCH`: backwards-compatible fixes, documentation, packaging, tests, and
  security fixes.

## Required Release Steps

For any release candidate:

1. Update `nexuslang-src/Cargo.toml` if the version changes.
2. Update `RELEASE_NOTES.md`.
3. Run `NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh`.
4. Run `./scripts/package-release.sh`.
5. Run `./scripts/validate-release-package.sh`.
6. For public artifacts, sign the archive and checksum using
   `./scripts/sign-release-artifacts.sh`.
7. For public release readiness, run
   `NEXUS_RELEASE_SIGNING_KEY=<key> ./scripts/release-dry-run-strict.sh`
   against the real GitHub repository.
8. Tag the source as `v<version>` only after the gate, artifact validation,
   maintained-key signing, and remote CI observation pass.

## Current Release

Current source version: `0.2.0-rc.2`

Latest stable GitHub Release: `v0.1.1`.

Current public pre-release RC: `v0.2.0-rc.2`.

Stable `0.2.0` decision: hold stable publication and run a short pre-stable
hardening cycle first. See `meta/STABLE_0_2_0_DECISION.md`.

Previous published GitHub Release: `v0.1.0`.

Run the public install validation against the release tag:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh
```

Validate the current public RC pre-release explicitly:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh
```
