# NexusLang Local Release

This file records the local release flow for the current NexusLang workspace.
It is intentionally small: the source of truth is still the automated gate.

## Current readiness

- Local stable target: `0.2.0`.
- Stable release status: `v0.2.0` published and public install validation
  passed.
- Previous public RC status: `v0.2.0-rc.2` published as a pre-release and public
  install validation passed.
- Stable `0.2.0` decision on 2026-05-28: promote only through this controlled
  branch and the gates in `meta/STABLE_0_2_0_DECISION.md`.
- Language/core: 78/100
- Playground: 84/100
- OpenAPI/runtime: 66/100
- Engineering/release quality: 100/100
- Real production readiness for the 0.1.1 release scope: public release
  published and post-release install validated
- Overall project score after the latest completed release phase: 100/100
- Public GitHub Release v0.2.0: published and public install validated
- Public GitHub Release v0.1.1: previous stable, published and post-release
  install validated
- Public GitHub pre-release v0.2.0-rc.2: published and public install validated
- Public GitHub pre-release v0.2.0-rc.1: published and public install validated
- Previous public GitHub Release v0.1.0: published and post-release install
  validated

## Required gate

Run the full local gate before preparing an artifact:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
```

The gate checks formatting, `cargo check` with warnings denied, optional
Clippy, Rust tests, the storage compatibility policy, playground JavaScript
syntax, CLI smoke tests, storage backup/restore smoke, and OpenAPI validation.

## Playground/WASM

Rebuild the browser artifact from the repository root:

```bash
./scripts/build-playground-wasm.sh
```

Serve the repository root to test the playground in a browser:

```bash
python3 -m http.server 8091 --bind 127.0.0.1
```

Then open:

```text
http://127.0.0.1:8091/nexuslang-playground.html
```

The status should report `WASM pronto`, the default example should run, and
diagnostics should still show stage/message/line/column for invalid input.

## Local package

Build a reproducible local artifact with:

```bash
./scripts/package-release.sh
```

The script rebuilds the CLI and playground WASM, copies the browser files,
examples, docs, validation scripts, and release binary into `dist/`, then
creates:

```text
dist/nexuslang-v<version>-local-release.tar.gz
dist/nexuslang-v<version>-local-release.tar.gz.sha256
```

The archive version comes from `nexuslang-src/Cargo.toml`. The `.sha256` file
is generated next to the archive and is validated before extraction.

For public releases, sign the archive and checksum after validation:

```bash
./scripts/sign-release-artifacts.sh
```

For a final local dry-run, including a Docker-based second-environment
validation and dry-run signing when no maintained key is available:

```bash
./scripts/release-dry-run.sh
```

For a strict public-release dry-run, first configure the real GitHub repository,
authenticate `gh`, push the current commit, and export a maintained GPG signing
key:

```bash
NEXUS_RELEASE_SIGNING_KEY="<fingerprint-or-key-id>" ./scripts/release-dry-run-strict.sh
```

Validate the artifact in a clean temporary directory:

```bash
./scripts/validate-release-package.sh
```

This extracts the archive under `/tmp`, checks the expected files, rejects
generated `.nexus-data` storage, runs the packaged CLI smoke, validates
playground JavaScript syntax, compares the manifest WASM size, and serves the
clean package long enough to fetch the HTML, JS, and WASM assets over HTTP.

## Public release install validation

Validate the already published GitHub Release from a clean temporary directory:

```bash
./scripts/validate-public-release-install.sh
```

This downloads the selected release assets from GitHub, verifies the published
fingerprint, imports the public key into an isolated `GNUPGHOME`, verifies the
detached signatures, checks the SHA-256 checksum, extracts the package under
`/tmp`, runs the packaged smoke test, and fetches playground HTML/JS/WASM over
local HTTP.

Validate `v0.1.1` explicitly when checking the published release:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh
```

Validate the `v0.2.0-rc.2` public pre-release explicitly when checking the RC:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0-rc.2 ./scripts/validate-public-release-install.sh
```

After publishing `v0.2.0`, validate the stable public release explicitly:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh
```

## Release 1.0 checklist

### Local release candidate

- [x] Rust formatting passes.
- [x] `cargo check --all-targets` passes with warnings denied.
- [x] Clippy passes with warnings denied.
- [x] Rust unit/integration tests pass.
- [x] CLI smoke tests pass.
- [x] OpenAPI external validation passes.
- [x] Playground JavaScript syntax check passes.
- [x] Playground WASM rebuild succeeds.
- [x] Local package builds.
- [x] Local package validates in a clean temporary directory.
- [x] Generated runtime storage is excluded from the package.
- [x] Local package filename includes the Cargo package version.
- [x] Local package has a SHA-256 checksum file.
- [x] CI builds and validates the local package.
- [x] Public install/getting-started guide exists.
- [x] Release notes document known limitations.
- [x] Version/tag policy is documented.
- [x] Language/runtime/storage compatibility contract is documented.
- [x] JSON/SQLite `0.1.x` storage migration policy is documented and gated.
- [x] Storage backup/restore guide and inventory smoke example exist.
- [x] Artifact signing path is documented and scripted.
- [x] Docker-based second-environment validation exists.
- [x] Final release dry-run script exists.
- [x] Final local dry-run passes with dry-run GPG signatures.
- [x] Strict public-release dry-run/preflight script exists.
- [x] Real GitHub repository is connected:
  `https://github.com/vitaleevo/NEXUSLANG`.
- [x] `gh` is authenticated for the release repository.
- [x] `main` is pushed to GitHub.
- [x] GitHub Actions passed for the pushed release commit.
- [x] Maintained GPG release key is configured:
  `3237F7CC5CE2514FC9671BB93CB6808B55385273`.
- [x] Strict release dry-run passed with maintained-key signing and remote CI
  observation.
- [x] `v0.2.0-rc.2` pre-release is published with signed archive, checksum,
  signatures, public key, and fingerprint.
- [x] Public GitHub pre-release install validation passes for `v0.2.0-rc.2`.
- [x] `v0.2.0-rc.1` pre-release was published with signed archive, checksum,
  signatures, public key, and fingerprint.
- [x] Public GitHub pre-release install validation passes for `v0.2.0-rc.1`.
- [x] Stable `0.2.0` decision is documented as hardening pre-stable, not
  immediate promotion.
- [x] GitHub Actions workflow is moved to Node 24 compatible first-party action
  refs pinned by commit SHA for pre-stable hardening.
- [x] Stable `0.2.0` source version is prepared on a dedicated release branch.
- [x] Stable `v0.2.0` tag and GitHub Release are published.
- [x] Public GitHub stable install validation passes for `v0.2.0`.

### Published release

- [x] `v0.2.0` source tag is the latest stable published release target.
- [x] GitHub Release `v0.2.0` is published with signed archive, checksum,
  signatures, public key, and fingerprint.
- [x] Public GitHub Release install validation passes for `v0.2.0`.
- [x] `v0.1.1` source tag is the previous stable published release target.
- [x] GitHub Release `v0.1.1` is published with signed archive, checksum,
  signatures, public key, and fingerprint.
- [x] Public GitHub Release install validation passes for `v0.1.1`.
- [x] `v0.1.0` source tag is the published release target.
- [x] GitHub Release `v0.1.0` is the publication target for the signed archive,
  checksum, signatures, and public key.
- [x] Public GitHub Release install validation downloads and verifies the
  published assets in a clean temporary directory.

### 0.1.1 release candidate

- [x] Source version bumped to `0.1.1`.
- [x] Local package target is `nexuslang-v0.1.1-local-release.tar.gz`.
- [x] Release notes describe the `0.1.1` patch scope.
- [x] Quality gate passes with Clippy for the current RC worktree.
- [x] Local package validates in a clean temporary directory.
- [x] Local release dry-run passes with Docker second-environment validation and
  maintained-key signing.
- [x] Strict public-release preflight attempted and documented as blocked by
  dirty local changes.
- [x] Current RC changes are committed and pushed.
- [x] GitHub Actions is observed for the pushed `0.1.1` commit.
- [x] Strict public-release dry-run passes for the pushed `0.1.1` commit.
- [x] `v0.1.1` tag and GitHub Release are published.
- [x] Public install validation passes with `NEXUS_PUBLIC_RELEASE_TAG=v0.1.1`.
