# NexusLang Local Release

This file records the local release flow for the current NexusLang workspace.
It is intentionally small: the source of truth is still the automated gate.

## Current readiness

- Language/core: 78/100
- Playground: 84/100
- OpenAPI/runtime: 66/100
- Engineering/release quality: 100/100
- Real production readiness for the 0.1.0 release scope: 100/100
- Overall project score after the latest completed phase: 100/100

## Required gate

Run the full local gate before preparing an artifact:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
```

The gate checks formatting, `cargo check` with warnings denied, optional
Clippy, Rust tests, playground JavaScript syntax, CLI smoke tests, and OpenAPI
validation.

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

### Production/public release follow-up

- [ ] Create and publish the `v0.1.0` source tag.
- [ ] Create a GitHub Release with the signed archive, checksum, signatures,
  and public key.
