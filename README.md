# NexusLang

NexusLang is an ERP-first programming language for business workflows,
models, invoices, routes, and small runtime services. The current release line
is `0.1.1` for evaluation, demos, and QA.

## What is included

- Rust CLI: `nexus`
- Rust-powered browser playground through WebAssembly
- ERP primitives: `model`, `workflow`, `route`, `invoice`, `money`
- Typed semantic checker with line/column diagnostics
- HTTP runtime for declared routes
- JSON and SQLite storage backends
- OpenAPI generation and validation for the supported HTTP subset
- Examples for ERP, runtime services, models, and OpenAPI QA

## Install From The GitHub Release

The published `v0.1.1` release is available at:

```text
https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.1.1
```

Download the archive, checksum, signatures, public key, and fingerprint:

```bash
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-v0.1.1-local-release.tar.gz
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-v0.1.1-local-release.tar.gz.sha256
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-v0.1.1-local-release.tar.gz.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-v0.1.1-local-release.tar.gz.sha256.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-release-public-key.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.1.1/nexuslang-release-signing-key.fingerprint
```

Verify the signing fingerprint, signatures, and checksum before extracting:

```bash
test "$(tr -d '[:space:]' < nexuslang-release-signing-key.fingerprint)" = "3237F7CC5CE2514FC9671BB93CB6808B55385273"
gpg --import nexuslang-release-public-key.asc
gpg --verify nexuslang-v0.1.1-local-release.tar.gz.asc nexuslang-v0.1.1-local-release.tar.gz
gpg --verify nexuslang-v0.1.1-local-release.tar.gz.sha256.asc nexuslang-v0.1.1-local-release.tar.gz.sha256
sha256sum -c nexuslang-v0.1.1-local-release.tar.gz.sha256
```

Extract and enter the package:

```bash
tar -xzf nexuslang-v0.1.1-local-release.tar.gz
cd nexuslang-v0.1.1-local-release
```

Run the packaged smoke test:

```bash
./scripts/smoke-package.sh
```

From a source checkout, maintainers can validate the public installation path
end to end from a clean temporary directory:

```bash
./scripts/validate-public-release-install.sh
```

## Build Or Validate A Local Package

Build or download the local package artifacts:

```text
nexuslang-v0.1.1-local-release.tar.gz
nexuslang-v0.1.1-local-release.tar.gz.sha256
```

Verify the archive before extracting it:

```bash
sha256sum -c nexuslang-v0.1.1-local-release.tar.gz.sha256
```

For signed public artifacts, also verify the detached GPG signatures described
in `SIGNING.md`.

Extract and enter the package:

```bash
tar -xzf nexuslang-v0.1.1-local-release.tar.gz
cd nexuslang-v0.1.1-local-release
```

Run the packaged smoke test:

```bash
./scripts/smoke-package.sh
```

## Quick Start

Show the CLI help:

```bash
./bin/nexus --help
```

Validate and run the basic ERP example:

```bash
./bin/nexus check examples/erp_basico.nx
./bin/nexus run examples/erp_basico.nx
```

Create a small file:

```nexus
model Customer {
    name: string
    balance: money
}

fn bonus(value: money) -> money {
    return value * 0.1
}

let balance = 100000 kz
print("Customer balance")
print(balance)
print("Bonus")
print(bonus(balance))
```

Then run it:

```bash
./bin/nexus run path/to/file.nx
```

## Playground

Serve the extracted package from its root:

```bash
python3 -m http.server 8091 --bind 127.0.0.1
```

Open:

```text
http://127.0.0.1:8091/nexuslang-playground.html
```

The page should report `WASM pronto`.

## HTTP Runtime

The CLI can serve route declarations:

```bash
./bin/nexus serve examples/openapi_qa.nx 127.0.0.1:5050
```

In another terminal:

```bash
curl http://127.0.0.1:5050/health
curl http://127.0.0.1:5050/openapi.json
```

Runtime storage may create `.nexus-data` next to the served example. That
directory is local generated data and is intentionally excluded from release
packages.

## Storage Backup And Restore

The packaged examples include a small inventory flow for storage operations:

```bash
./bin/nexus check examples/storage_backup_restore_inventory.nx
./scripts/smoke-storage-backup-restore.sh
```

The guide in `STORAGE_BACKUP_RESTORE.md` documents JSON `.nexus-data` backup,
SQLite database backup expectations, restore checks, and the safe `0.1.x`
schema evolution limits.

## Build From Source

Recommended local requirements:

- Rust stable with `rustfmt` and `clippy`
- Node.js 22 or newer
- Python 3.12 or newer for OpenAPI validation helpers

Run the full gate:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
```

Build and validate the local release package:

```bash
./scripts/package-release.sh
./scripts/validate-release-package.sh
```

Run the final local dry-run:

```bash
./scripts/release-dry-run.sh
```

Run the strict public-release dry-run only after connecting a real GitHub
repository, authenticating `gh`, pushing the current commit, and configuring a
maintained GPG key:

```bash
NEXUS_RELEASE_SIGNING_KEY="<fingerprint-or-key-id>" ./scripts/release-dry-run-strict.sh
```

Validate the `v0.1.1` public install path with:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh
```

## Current Limits

- The packaged binary is a local Linux/WSL-style artifact, not a cross-platform
  installer.
- The current source version is `0.1.1`; version/tag policy is documented in
  `VERSIONING.md`.
- Release artifacts have SHA-256 checksums and detached GPG signatures.
- Strict public-release validation is scripted in `GITHUB_RELEASE.md`, and the
  published install path is validated by
  `scripts/validate-public-release-install.sh`.
- JSON storage is the simplest supported backend; SQLite exists but storage
  compatibility and migration limits for `0.1.x` are documented in
  `COMPATIBILITY.md`.
- `index` model metadata is declarative and does not create physical indexes.
- The OpenAPI contract covers the supported NexusLang HTTP subset, not every
  possible OpenAPI feature.
- The playground is a learning/debugging surface, not a hosted production app.

See `RELEASE_NOTES.md`, `RELEASE.md`, `VERSIONING.md`, `COMPATIBILITY.md`,
`SIGNING.md`, and `GITHUB_RELEASE.md` for release status and known risks.
