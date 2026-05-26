# NexusLang 0.1.0 Release Notes

Release type: local/internal release candidate.

This release packages the current NexusLang compiler/runtime, examples,
playground assets, release documentation, and validation scripts into a
versioned local archive with a SHA-256 checksum.

## Highlights

- ERP-first language core with models, workflows, invoices, money values, and
  typed functions.
- CLI commands for `run`, `check`, `fmt`, `lint`, `repl`, `new`, `tokens`,
  `ast`, and `serve`.
- HTTP runtime for declared routes, including path params, typed query params,
  CRUD-style model operations, filters, pagination, and error responses.
- JSON storage and SQLite storage backend coverage for critical CRUD/filter
  parity.
- OpenAPI 3.0 document generation for the supported route/runtime subset.
- Rust-powered browser playground compiled to WebAssembly.
- Local release package:
  - `nexuslang-v0.1.0-local-release.tar.gz`
  - `nexuslang-v0.1.0-local-release.tar.gz.sha256`
- Clean package validation with checksum verification before extraction.
- Formal version/tag policy in `VERSIONING.md`.
- Compatibility contract in `COMPATIBILITY.md`.
- GPG signing path in `SIGNING.md` and `scripts/sign-release-artifacts.sh`.
- Strict public-release preflight and dry-run path in `GITHUB_RELEASE.md` and
  `scripts/release-dry-run-strict.sh`.
- CI workflow configured to run the quality gate, build the local package,
  validate it, and upload the archive/checksum as workflow artifacts.

## Validation Summary

The current local release process validates:

- Rust formatting
- `cargo check --all-targets` with warnings denied
- Clippy with warnings denied
- Rust unit and integration tests
- CLI smoke tests
- OpenAPI external validation
- Playground JavaScript syntax
- WASM rebuild
- Package smoke in a clean temporary directory
- SHA-256 checksum before package extraction

Latest recorded release state:

- Project score: 100/100 for the 0.1.0 release scope after the strict
  public-release dry-run phase
- GitHub repository: `https://github.com/vitaleevo/NEXUSLANG`
- GitHub Actions: observed successful `NexusLang Quality Gate` for the pushed
  release commit
- Signing key fingerprint: `3237F7CC5CE2514FC9671BB93CB6808B55385273`
- WASM size: 347437 bytes
- Rust tests: 9 internal + 145 core/integration tests
- CLI smoke: 18 passed, 0 failed
- OpenAPI validation: PASS

## Supported Subset

NexusLang 0.1.0 focuses on a practical ERP subset:

- scalar values: string, int, float, bool, money, date, nil
- arrays and optional values
- typed functions and return checking
- model declarations with defaults and basic constraints
- model instances and field access
- executable workflows
- structured invoices
- route declarations with HTTP methods and typed params
- model create, find, update, delete, list, filters, ordering, and pagination
- generated OpenAPI for the supported route shapes

## Known Limitations

- A signed release dry-run passed, but the final GitHub Release and public
  `v0.1.0` tag still need to be published.
- The packaged binary is local-platform oriented and has not been validated as
  a cross-platform installer.
- Remote GitHub Actions should still be observed after a real push or PR.
- JSON storage is suitable for local/dev scenarios; storage compatibility is
  not yet frozen as a long-term public contract.
- SQLite has parity coverage for critical flows, but database migrations and
  long-term schema compatibility are not yet formalized.
- `index` model fields are declarative metadata today; they do not create
  physical indexes yet.
- OpenAPI output is validated for the NexusLang-supported HTTP subset, not for
  arbitrary custom OpenAPI extensions or every possible route shape.
- Playground loading is validated locally; it is not yet deployed as a hosted
  public web product.
- Strict release dry-run refuses ephemeral keys and requires observed GitHub
  Actions for the current commit; this gate has passed with the maintained
  NexusLang release key.

## Upgrade Notes

There is no previous public package format to migrate from. If a local package
exists from an earlier phase, rebuild it:

```bash
./scripts/package-release.sh
./scripts/validate-release-package.sh
```

## Next Release Focus

- Publish the `v0.1.0` source tag and GitHub Release.
- Attach signed artifacts, checksums, signatures, and the public release key to
  the GitHub Release.
- Public install guide polish and expanded examples.
- Clear storage compatibility policy for JSON and SQLite.
