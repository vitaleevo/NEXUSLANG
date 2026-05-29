# NexusLang Compatibility Contract

This document records what NexusLang treats as stable, release-candidate, or
experimental for the current public `0.2.x` line.

## Compatibility Levels

- Stable: changes require a documented version bump and release note.
- Release candidate: intended to stay coherent, but may still change before
  `1.0.0` with clear notes.
- Experimental: useful, tested locally, but not a long-term public contract.
- Internal: implementation detail with no compatibility promise.

## Language Syntax

Level: release candidate.

The syntax baseline is documented in `nexuslang-src/SYNTAX_1_0.md`. The current
release candidate supports:

- functions with typed params and returns;
- `let` and `const`;
- `if`, `while`, and `for`;
- arrays and optional values;
- `model`, `workflow`, `route`, `invoice`, and `money`;
- model instances and field access;
- typed route params and query params.

Before `1.0.0`, syntax changes may still happen, but breaking changes must be
listed in `RELEASE_NOTES.md`.

## CLI

Level: release candidate.

The public CLI commands for `0.2.x` include:

```text
nexus run [file.nx]
nexus check [file.nx]
nexus fmt <file.nx> [--write]
nexus lint <file.nx>
nexus docs [file.nx] [--output docs.md]
nexus test [file-or-directory]
nexus serve [file.nx] [addr] [--storage json|sqlite]
nexus storage-plan [file.nx] [--storage sqlite] [--apply]
nexus install
nexus add <package> [--path <dir>|--registry <pkg@version>]
nexus update
nexus repl
nexus new <project>
nexus tokens <file.nx>
nexus ast <file.nx>
nexus --help
```

Patch releases should not remove or rename these commands.

## HTTP Runtime

Level: release candidate.

The runtime contract covers the route shapes validated by tests and OpenAPI QA:

- health endpoint;
- declared route matching;
- path params with safe percent-decoding;
- typed/defaulted/optional query params;
- model create, find, update, delete, lists, filters, ordering, and pagination;
- JSON responses with `400`, `404`, and `409` errors where supported.

Unsupported route expressions remain outside the compatibility contract.

## OpenAPI

Level: release candidate.

OpenAPI generation is stable for the supported NexusLang HTTP subset and is
validated by internal tests plus external OpenAPI 3.0 validation. `x-nexus-*`
extensions are Nexus-specific and should be treated as optional metadata by
external tools.

## Storage

### JSON Storage

Level: release candidate for the `0.2.x` record contract; experimental for
long-term operations.

JSON storage is appropriate for local development, examples, demos, and smoke
tests. The default `nexus serve <file.nx>` runtime stores one JSON array per
model under `.nexus-data` next to the served source file:

```text
.nexus-data/<model-name-lowercase>.json
```

For `0.2.x`, NexusLang keeps this minimal record contract:

- each model file is a JSON array;
- each array item is a JSON object representing one model record;
- money fields are stored as objects with `amount` and `currency`;
- optional fields may be absent in older data and are read as `null`;
- fields with static defaults may be absent in older data and are read with
  their declared default value;
- generated `.nexus-data` directories are local runtime data and are excluded
  from release packages.

NexusLang does not promise automatic rewrites of existing JSON files in
`0.2.x`. Compatibility for additive fields is applied when records are read.

### SQLite Storage

Level: release candidate for JSON/SQLite behavior parity; experimental for the
physical database schema.

SQLite has parity coverage for critical CRUD and filter flows. The current
SQLite backend stores each model in a lower-case table with an internal
autoincrement `id` and a `data TEXT NOT NULL` JSON payload. This physical
layout is still not a public ORM contract, but NexusLang now exposes a
conservative migration plan so users can inspect and apply the supported DDL
before serving production-like data.

The public `0.2.x` SQLite promise is behavioral:

- CRUD, filters, ordering, pagination, unique checks, defaults, optionals, and
  min/max validation should match JSON storage for the supported route subset;
- additive optional/defaulted fields should read older records the same way as
  JSON storage;
- SQLite database files remain user data and should be backed up before
  upgrading NexusLang.

SQLite creates internal indexes for `unique` fields and physical indexes for
declared `index` fields through the migration plan. Index names remain
implementation details and should not be used by application code.

SQLite also creates an internal `nexus_schema_migrations` ledger when the
migration plan is applied. The ledger records NexusLang-managed DDL actions for
auditability and idempotence; it is not an application model or public SQL API.

### SQLite Migration Plan MVP

Level: release candidate for conservative dry-run/apply behavior;
experimental for the long-term physical schema.

Use the migration plan before running SQLite against important data:

```bash
nexus storage-plan path/to/app.nx --storage sqlite
nexus storage-plan path/to/app.nx --storage sqlite --apply
```

The dry-run opens the SQLite target and reports:

- missing internal migration ledger table;
- missing model tables;
- missing auth table when the program declares `auth`;
- missing unique indexes for `unique` fields;
- missing non-unique indexes for fields declared with `index`;
- blockers for an incompatible internal migration ledger table;
- blockers for legacy tables that do not match the current `id`/`data`
  payload layout;
- blockers for unique indexes that would fail because existing data contains
  duplicate values.

`--apply` refuses to run if blockers exist. Safe actions create missing tables
and indexes only, then insert deterministic records into
`nexus_schema_migrations`. They do not rewrite existing JSON payloads, rename
fields, drop columns, transform money/date representation, or perform a
semantic versioned migration.

### SQLite Migration History MVP

Level: release candidate for action idempotence; internal for the ledger schema.

For `0.2.x`, NexusLang records only the storage actions it knows how to apply:

- creation of the internal migration ledger;
- creation of model tables;
- creation of the auth table;
- creation of unique indexes;
- creation of non-unique indexes for `index` fields.

The ledger is append-only for applied NexusLang DDL actions and uses stable
action IDs to make reapplying the same plan idempotent. It is not a full
semantic migration system: NexusLang does not yet track rename/drop/data
transform history, rollback scripts, model-version graphs, or dependency-aware
schema solvers.

### Migration Policy For 0.2.x

NexusLang `0.2.x` supports only conservative, user-auditable storage evolution.

Supported additive model changes:

- add an optional field, for example `email: string?`;
- add a field with a static default, for example `status: string = "active"`;
- add or tighten runtime validation for new writes, when old records can still
  be read through optional/defaulted fields.

Breaking storage changes:

- rename a model or field;
- remove a field that existing code still reads;
- add a required field without a default;
- change a field type in existing data;
- change money/date representation;
- depend on physical SQL layout beyond the documented migration plan.

For breaking changes, users must export or back up data first, transform the
data explicitly, then run smoke tests against the new model declarations before
serving production-like data.

### Backup And Restore Expectations

Before upgrading between `0.2.x` releases:

- stop the NexusLang server process;
- copy the full `.nexus-data` directory for JSON storage;
- copy the SQLite database file together with any companion `-wal` and `-shm`
  files if present;
- validate the upgraded program with `nexus check`;
- run at least one create, find/list, update, delete, and filter route against
  a copied dataset before reusing original data.

The release package must not contain generated `.nexus-data` directories. That
is enforced by release-package validation.

The operational backup/restore guide is `STORAGE_BACKUP_RESTORE.md`. It is
included in release packages as `docs/STORAGE_BACKUP_RESTORE.md`.

### Storage Release Gate

Every `0.2.x` release candidate should run:

```bash
./scripts/validate-storage-compatibility-policy.sh
./scripts/smoke-storage-backup-restore.sh
./scripts/smoke-sqlite-backup-restore.sh
cd nexuslang-src
cargo test sqlite_migration_plan_records_history_and_is_idempotent
cargo test storage_schema_evolution_allows_additive_optional_and_defaulted_fields
cargo test sqlite_storage_matches_json_storage_for_crud_and_critical_filters
```

After publishing a GitHub Release, maintainers should also run:

```bash
./scripts/validate-public-release-install.sh
```

That post-release check downloads the public assets, verifies checksum and GPG
signatures, extracts the package in `/tmp`, and runs the packaged smoke test.

## Playground And WASM

Level: release candidate for the packaged user experience, internal for WASM
exports.

The package promises that `nexuslang-playground.html`,
`nexuslang-playground.js`, and
`nexuslang-src/web/nexuslang_playground.wasm` load together when served from
the package root. Raw WebAssembly exports are internal implementation details.

## Package Layout

Level: release candidate.

The release package should contain:

- `bin/nexus`
- `nexuslang-playground.html`
- `nexuslang-playground.js`
- `nexuslang-src/web/nexuslang_playground.wasm`
- `examples/*.nx`
- `docs/README.md`
- `docs/RELEASE.md`
- `docs/RELEASE_NOTES.md`
- `docs/VERSIONING.md`
- `docs/COMPATIBILITY.md`
- `docs/STORAGE_BACKUP_RESTORE.md`
- `docs/SIGNING.md`
- `examples/storage_backup_restore_inventory.nx`
- `PACKAGE_MANIFEST.txt`
- `scripts/smoke-package.sh`
- `scripts/smoke-storage-backup-restore.sh`
- `scripts/smoke-sqlite-backup-restore.sh`

## Breaking Change Rules

Before `1.0.0`, breaking changes are allowed only when they are intentional,
documented, and covered by tests or release validation. After `1.0.0`, breaking
changes require a major version bump.
