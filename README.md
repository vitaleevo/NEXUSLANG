# NexusLang

NexusLang is an ERP-first programming language for business workflows,
models, invoices, routes, and small runtime services. The current stable source
line is `0.2.0`. The latest stable GitHub Release is `v0.2.0`; the previous
public pre-release remains `v0.2.0-rc.2` for RC comparison.

## What is included

- Rust CLI: `nexus`
- Rust-powered browser playground through WebAssembly
- ERP primitives: `model`, `workflow`, `route`, `auth`, `invoice`, `money`
- Typed semantic checker with line/column diagnostics
- HTTP runtime for declared routes
- JSON and SQLite storage backends
- OpenAPI generation and validation for the supported HTTP subset
- Central `ModelStaticOperation` descriptor contract for checker, runtime
  storage, and OpenAPI behavior
- Native auth MVP with Argon2id password hashing, opaque sessions, bearer
  tokens, and route guards
- Local package manager MVP with `nexus.toml`, `nexus.lock`, `nexus install`,
  `nexus add`, `nexus update`, path dependencies, and an initial registry
  declaration contract
- Examples for ERP, runtime services, models, and OpenAPI QA

## Install From The GitHub Release

The published `v0.2.0` release is available at:

```text
https://github.com/vitaleevo/NEXUSLANG/releases/tag/v0.2.0
```

Download the archive, checksum, signatures, public key, and fingerprint:

```bash
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-v0.2.0-local-release.tar.gz
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-v0.2.0-local-release.tar.gz.sha256
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-v0.2.0-local-release.tar.gz.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-v0.2.0-local-release.tar.gz.sha256.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-release-public-key.asc
curl -LO https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0/nexuslang-release-signing-key.fingerprint
```

Verify the signing fingerprint, signatures, and checksum before extracting:

```bash
test "$(tr -d '[:space:]' < nexuslang-release-signing-key.fingerprint)" = "3237F7CC5CE2514FC9671BB93CB6808B55385273"
gpg --import nexuslang-release-public-key.asc
gpg --verify nexuslang-v0.2.0-local-release.tar.gz.asc nexuslang-v0.2.0-local-release.tar.gz
gpg --verify nexuslang-v0.2.0-local-release.tar.gz.sha256.asc nexuslang-v0.2.0-local-release.tar.gz.sha256
sha256sum -c nexuslang-v0.2.0-local-release.tar.gz.sha256
```

Extract and enter the package:

```bash
tar -xzf nexuslang-v0.2.0-local-release.tar.gz
cd nexuslang-v0.2.0-local-release
```

Run the packaged smoke test:

```bash
./scripts/smoke-package.sh
```

From a source checkout, maintainers can validate the public installation path
end to end from a clean temporary directory:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh
```

## Build Or Validate A Local Package

Build the stable package locally from this branch:

```text
nexuslang-v0.2.0-local-release.tar.gz
nexuslang-v0.2.0-local-release.tar.gz.sha256
```

The last public RC artifacts remain available for comparison:

- https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0-rc.2/nexuslang-v0.2.0-rc.2-local-release.tar.gz
- https://github.com/vitaleevo/NEXUSLANG/releases/download/v0.2.0-rc.2/nexuslang-v0.2.0-rc.2-local-release.tar.gz.sha256

Verify the archive before extracting it:

```bash
sha256sum -c nexuslang-v0.2.0-local-release.tar.gz.sha256
```

For signed public artifacts, also verify the detached GPG signatures described
in `SIGNING.md`.

Maintainers can validate the public stable install path end to end with
`scripts/validate-public-release-install.sh`:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh
```

Extract and enter the package:

```bash
tar -xzf nexuslang-v0.2.0-local-release.tar.gz
cd nexuslang-v0.2.0-local-release
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

For tooling, validation diagnostics can also be emitted as JSON:

```bash
./bin/nexus check --json examples/erp_basico.nx
./bin/nexus run --json examples/erp_basico.nx
```

The JSON output uses the versioned diagnostics contract documented in
[`nexuslang-src/DIAGNOSTICS_JSON_CONTRACT.md`](nexuslang-src/DIAGNOSTICS_JSON_CONTRACT.md).
Diagnostics include optional `code`, `severity`, `labels`, `notes`, and
`suggestions` fields for tooling. The v1 code catalog is granular by lexer,
parser, module-loader, checker, and runtime error family, with high-impact
checker, loader, and runtime diagnostics already carrying richer metadata. Rust
tooling can also use the additive diagnostic report API to group diagnostics by
path/module, query reports by path, module, stage, severity, or group, and get
an in-memory summary plus flattened tooling items with group indexes. When a
`SourceDatabase` is available, Rust tooling can opt into source-line snippets
and highlight columns without changing JSON output. The public report,
summary, flattened view, and source-context surfaces are covered by a
pre-LSP contract matrix in the diagnostics contract doc. A compilable Rust
consumer example lives at
[`nexuslang-src/examples/diagnostic_report_tooling.rs`](nexuslang-src/examples/diagnostic_report_tooling.rs).
The first editor adapter now lives in
[`nexuslang-src/nexus-lsp`](nexuslang-src/nexus-lsp): it uses `tower-lsp` over
stdio, publishes diagnostics from the core diagnostic APIs, and offers initial
hover, completion, semantic tokens, document symbols, and same-document
go-to-definition. For clean entry
snapshots that match disk, `LspCore` now bridges to `SourceDatabase` and the
module-loader/checker report APIs to publish diagnostics for imported modules
as well as the opened file. Dirty unsaved snapshots fall back to
single-document diagnostics and clear previously published imported-module
diagnostics when the old graph is no longer current. Close events also publish
empty batches for the document's previous diagnostic group, while shared
modules stay visible if another open entry document still owns them. Its
transport adapter is thin:
`nexus-lsp/src/lib.rs` owns the testable `DocumentSnapshot`/`LspCore` logic and
`src/main.rs` only bridges LSP events to that core. It remains deliberately
separate from the compiler core and does not yet provide a persistent or
incremental source database. Go-to-definition now also has an opt-in
cross-file path for imports and aliases: clean disk-backed snapshots resolve
through `SourceDatabase` import edges and `ModuleGraph` exports to the target
module declaration, while dirty entry or imported snapshots keep the
same-document fallback. The adapter also provides full-document semantic tokens
with a small lexical legend for keywords, types, strings, numbers, identifiers,
and ERP symbols such as `model`, `route`, `auth`, `workflow`, `step`, and
`invoice`. Document symbols are document-local and AST-backed, covering
declarations plus ERP children such as model fields, workflow steps, and
route query params and invoice entries when the document parses.
CLI users can opt into that report with `nexus check
--json-report` or `nexus run --json-report`; those report modes can collect
multiple checker diagnostics from independent function, route, workflow, and
invoice declaration bodies, while regular `--json` keeps the first-error shape.

Generate Markdown documentation from checked ERP declarations:

```bash
./bin/nexus docs examples/openapi_qa.nx --output docs.md
```

`nexus docs` validates the entrypoint and its imports before documenting
models, functions, workflows, auth configs, routes, and invoices.

Run local `.nx` smoke tests or examples:

```bash
./bin/nexus test examples
```

Without an explicit path, `nexus test` looks for `tests/` first and then
`examples/`. Each discovered `.nx` file is validated and executed through the
same multi-module loader used by `nexus run`. Add an optional `.out` sidecar
next to a test file, for example `tests/smoke.nx` and `tests/smoke.out`, to
compare captured stdout exactly. Add an optional `.err` sidecar to make an
intentional diagnostic part of the passing contract; for example,
`tests/bad_input.nx` with `tests/bad_input.err` passes only when the diagnostic
text matches. Tests can also use native assertions:
`assert_true(condition)`, `assert_eq(actual, expected)`, `assert_ne(actual,
expected)`, and `assert_contains(container, item)` stop the case with a runtime
diagnostic when they fail; all accept an optional final message string for CI
context. Use `nexus test --update tests` to write or refresh `.out` files from
successful test output; failing programs never update their `.out` sidecars.
Use `nexus test --update-err tests` to write or refresh `.err` files from the
current diagnostic when a program fails; passing programs do not create `.err`.
Use `nexus test --name smoke tests` to run only discovered files whose path/name
contains `smoke`; the filter also composes with `--update` and
`--update-err`. Use
`nexus test --json tests` for a machine-readable report with summary counts,
per-case status, captured output, `.out` mismatches, expected diagnostics,
diagnostic mismatches, diagnostics, and updated sidecar paths. Mismatch reports
include a compact first divergent line in human output and JSON `first_diff`
metadata. Human output blocks are truncated after 20 lines to keep local/CI logs
readable; JSON keeps the full arrays for tooling. Use
`nexus test --list tests` to print the deterministic
discovered/filtered case list without executing programs or updating sidecars;
it also composes with `--json`. Use `nexus test --timeout 5s tests` to fail a
case that does not finish within the configured time. Use
`nexus test --isolate-data tests` to give each case its own temporary
`NEXUS_DATA_DIR`, keeping runtime storage away from the workspace
`.nexus-data`. Use `nexus test --jobs 4 tests` to run multiple cases at once;
reports keep the deterministic discovered order. Use
`nexus test --fail-fast tests` to stop after the first failing case; with
`--jobs`, execution stops after the first batch that contains a failure.

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

## Package Manager MVP

Create a project with a package manifest and lockfile:

```bash
nexus new acme_erp
cd acme_erp
```

The generated project includes:

```text
nexus.toml
nexus.lock
main.nx
examples/
```

Install or refresh local dependencies:

```bash
nexus install
```

Add a local dependency and update the lockfile:

```bash
nexus add crm_core
nexus update
```

Add a dependency from a local package directory:

```bash
nexus add billing-core --path ../billing_core
```

Declare a future registry dependency without downloading it yet:

```bash
nexus add audit_core --registry audit_core@0.1.0
```

The MVP stores dependencies in `nexus.toml`, writes deterministic package
entries to `nexus.lock`, creates a local `.nexus/packages/` cache, validates
manifest structure, and removes stale cache entries during install/update.
Remote registries, downloads, publishing, transitive dependencies, and version
solving are not implemented yet. See `PACKAGE_MANAGER.md` for the current
contract.

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

JSON is the default runtime storage driver. SQLite can be selected explicitly:

```bash
./bin/nexus serve examples/openapi_qa.nx 127.0.0.1:5050 --storage sqlite
```

In another terminal:

```bash
curl http://127.0.0.1:5050/health
curl http://127.0.0.1:5050/openapi.json
```

Runtime storage may create `.nexus-data` next to the served example. That
directory is local generated data and is intentionally excluded from release
packages. The JSON driver stores model files in that directory; the SQLite
driver stores `.nexus-data/nexus.db`.

## Model Operation Contract

The supported `Model::...` route operations are defined by a central Rust
descriptor table instead of repeated string lists in checker, router, and
OpenAPI code. See `MODEL_OPERATIONS.md` for the current contract, descriptor
fields, supported operation families, and extension checklist.

## Native Auth MVP

The post-`v0.1.1` source line includes the first secure backend auth slice:

- `auth Name { ... }` declarations bound to a `model`
- `route ... auth(Name)` and `auth(Name, role: "admin")` guards
- `Auth::register(Name)`, `Auth::login(Name)`, `Auth::logout()`, and
  `Auth::user()`
- Argon2id password hashing with per-password salt
- opaque server-side session cookies and revocable bearer tokens
- rate limiting for failed login/register attempts by auth identity
- CSRF tokens for cookie-backed `POST`/`PUT`/`DELETE` protected routes
- JSON and SQLite auth-store persistence through the storage backend
- OpenAPI `securitySchemes`, `401`, and `403` for protected routes

Example:

```bash
./bin/nexus check examples/auth_secure_crm.nx
./bin/nexus serve examples/auth_secure_crm.nx 127.0.0.1:5050
```

The default runtime stores auth metadata in `.nexus-data/.nexus-auth.json`.
SQLite-backed tests store the same auth metadata in the `nexus_auth` table.
Passwords, issued session/token secrets, and CSRF tokens are never stored in
clear text.

For browser cookie sessions, unsafe protected routes require the
`X-Nexus-CSRF-Token` header with the `csrf_token` returned by
`Auth::register()` or `Auth::login()`. Bearer token requests do not require the
CSRF header.

Production deployment expectations:

- run `nexus serve` behind an HTTPS terminator or reverse proxy;
- forward only trusted proxy headers from the edge;
- keep the `Secure` session cookie enabled;
- persist and back up the selected storage backend;
- treat the built-in server as the simple development/runtime server, not a TLS
  terminator.

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

Validate the `v0.2.0` public install path with:

```bash
NEXUS_PUBLIC_RELEASE_TAG=v0.2.0 ./scripts/validate-public-release-install.sh
```

## Current Limits

- The packaged binary is a local Linux/WSL-style artifact, not a cross-platform
  installer.
- The package manager is still an MVP; registry dependencies can be declared,
  but remote downloads, semantic version resolution, package publishing, and
  transitive dependencies are not implemented yet.
- The current source version is `0.2.0`; version/tag policy is documented in
  `VERSIONING.md`.
- Release artifacts have SHA-256 checksums and detached GPG signatures.
- Strict public-release validation is scripted in `GITHUB_RELEASE.md`, and the
  published install path is validated by
  `scripts/validate-public-release-install.sh`.
- JSON storage is the simplest supported backend; SQLite exists but storage
  compatibility and migration limits for `0.1.x` are documented in
  `COMPATIBILITY.md`.
- Native auth has rate limiting, CSRF tokens for cookie-backed unsafe methods,
  and JSON/SQLite auth-store parity. Secret rotation, password reset, MFA, and a
  dedicated production TLS server are still future hardening work.
- `index` model metadata is declarative and does not create physical indexes.
- The OpenAPI contract covers the supported NexusLang HTTP subset, not every
  possible OpenAPI feature.
- The playground is a learning/debugging surface, not a hosted production app.

See `RELEASE_NOTES.md`, `RELEASE.md`, `VERSIONING.md`, `COMPATIBILITY.md`,
`PACKAGE_MANAGER.md`, `SIGNING.md`, and `GITHUB_RELEASE.md` for release status
and known risks.
