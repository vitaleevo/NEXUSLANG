# NexusLang 0.2.0-rc.1 Release Notes

Release type: local/public release-candidate preparation for the `0.2.0`
line.

This RC collects the post-`v0.1.1` development work into a traceable release
candidate. It expands tooling, package-manager, diagnostics, runtime, stdlib,
and release infrastructure surfaces, so it is intentionally tracked as a
minor-line RC rather than a `0.1.x` patch.

## 0.2.0-rc.1 Highlights

- Version target updated to `0.2.0-rc.1`.
- Initial separate `nexus-lsp` crate with diagnostics, hover, completion,
  go-to-definition for imports/aliases, semantic tokens, and document symbols.
- Multi-module diagnostics/report APIs and source database tooling contracts.
- Expanded module loader, HIR/checker internals, and import/package path
  handling.
- Local package manager MVP with manifests, lockfiles, path dependencies, and
  registry declaration contract.
- Initial stdlib modules for ERP/business, data, HTTP, security, reporting, and
  operational helpers.
- Runtime/auth/storage/OpenAPI hardening with JSON/SQLite parity checks,
  native auth smokes, and release validation updates.
- Release packaging scripts updated to include new docs, stdlib, smokes, and
  validation assets.

## 0.2.0-rc.1 Validation Plan

Before this RC can be tagged or published, run:

```bash
NEXUS_RUN_CLIPPY=1 ./scripts/quality-gate.sh
./scripts/package-release.sh
./scripts/validate-release-package.sh
```

For public release readiness, after the branch is committed, pushed, and CI is
green, run:

```bash
NEXUS_RELEASE_SIGNING_KEY=<fingerprint> ./scripts/release-dry-run-strict.sh
```

## 0.2.0-rc.1 Known Limits

- Registry dependencies are declarations only; there are still no remote
  downloads, package publishing command, semantic version solver, or
  transitive dependency solver.
- LSP features remain MVP-level and do not yet include workspace symbols,
  formatting, rename, code actions, or persistent source database indexing.
- The playground is still a local/package asset, not a hosted public web
  product.
- SQLite physical schema remains experimental; JSON/SQLite behavior is tested
  for supported flows, not a full production database migration system.
- This RC should not be published from a dirty worktree.

---

# NexusLang 0.1.1 Release Notes

Release type: public `0.1.1` patch release.

This release is a patch-line hardening release on top of the public `v0.1.0`
release. It focuses on public install validation, storage
compatibility, backup/restore operations, and release-package QA. It does not
intentionally change the NexusLang language syntax.

## Highlights

- Version bumped to `0.1.1` in `nexuslang-src/Cargo.toml`.
- CLI help now reads the version from Cargo package metadata.
- Public GitHub Release install validation script:
  `scripts/validate-public-release-install.sh`.
- JSON/SQLite `0.1.x` storage compatibility and migration policy in
  `COMPATIBILITY.md`.
- Storage policy gate:
  `scripts/validate-storage-compatibility-policy.sh`.
- Operational storage backup/restore guide:
  `STORAGE_BACKUP_RESTORE.md`.
- Inventory storage example:
  `examples/storage_backup_restore_inventory.nx`.
- Backup/restore smoke test:
  `scripts/smoke-storage-backup-restore.sh`.
- Release package validation now checks the storage guide, example, policy
  gate, and backup/restore smoke.
- Quality gate now includes the storage compatibility policy and backup/restore
  smoke.

## Validation Summary

The `0.1.1` release gate validates:

- Rust formatting
- `cargo check --all-targets` with warnings denied
- optional Clippy with warnings denied
- Rust unit and integration tests
- storage compatibility policy
- CLI smoke tests
- storage backup/restore smoke
- OpenAPI external validation
- playground JavaScript syntax
- WASM rebuild
- package smoke in a clean temporary directory
- SHA-256 checksum before package extraction

Recorded release artifact state:

- Version: `0.1.1`
- Release package target:
  - `nexuslang-v0.1.1-local-release.tar.gz`
  - `nexuslang-v0.1.1-local-release.tar.gz.sha256`
- The published `.sha256` asset is the source of truth for archive integrity.
- Quality gate with Clippy: passed.
- Local release dry-run: passed with Docker second-environment validation and
  maintained-key signing.
- Strict public-release dry-run is part of the publication gate and requires
  commit/push, GitHub Actions observation, and maintained-key signing.
- Public GitHub release tag: `v0.1.1`.
- Post-release public install validation:
  `NEXUS_PUBLIC_RELEASE_TAG=v0.1.1 ./scripts/validate-public-release-install.sh`.

## Supported Subset

NexusLang 0.1.1 keeps the same practical ERP subset as 0.1.0:

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

## Storage Notes

`0.1.1` documents the `0.1.x` storage policy more concretely:

- JSON storage keeps one array file per model under `.nexus-data`.
- Missing optional fields in older records read as `null`.
- Missing static-default fields in older records read with their declared
  defaults.
- SQLite is a behavioral parity backend for supported CRUD/filter flows, but
  its physical schema remains experimental.
- Renames, removals, required fields without defaults, type changes, and
  reliance on physical `index` behavior remain breaking storage changes.

## Known Limitations

- The packaged binary is still local Linux/WSL oriented and is not a
  cross-platform installer.
- SQLite does not yet have a stable public `nexus serve` selection flag.
- `index` model fields are declarative metadata today; they do not guarantee
  physical indexes.
- OpenAPI output is validated for the NexusLang-supported HTTP subset, not for
  arbitrary custom OpenAPI extensions or every possible route shape.
- Playground loading is validated locally; it is not yet deployed as a hosted
  public web product.
- Strict public-release dry-run requires a clean worktree, pushed HEAD,
  observed GitHub Actions, and the maintained signing key.

## Upgrade Notes From 0.1.0

This is a patch release. For users of the `0.1.0` package:

1. Back up `.nexus-data` before testing `0.1.1`.
2. Run `nexus check` on the served `.nx` files.
3. Run at least one create, find/list, update, delete, and filter route against
   a copied dataset.
4. Use `STORAGE_BACKUP_RESTORE.md` as the operational guide.

## Previous Public Release: 0.1.0

`v0.1.0` established the first public archive, checksum, signatures, public
signing key, release notes, strict release dry-run, and public install
validation path.

## Next Release Focus

- Keep `v0.1.1` honest by treating public install validation as the
  post-release gate for every published asset update.
- For `0.2.0`, choose a durable ERP vertical slice after storage/index/migration
  risks are explicit.
