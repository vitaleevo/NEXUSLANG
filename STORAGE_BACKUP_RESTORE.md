# NexusLang Storage Backup And Restore

This guide is the operational companion to the `0.2.x` storage compatibility
policy in `COMPATIBILITY.md`.

## Scope

The public `nexus serve <file.nx>` command uses JSON storage by default. It
creates `.nexus-data` next to the served `.nx` file, with one lower-case model
file per model:

```text
.nexus-data/inventoryitem.json
```

SQLite is covered by JSON/SQLite behavior parity tests and by the compatibility
policy. Use `--storage sqlite` to serve with SQLite and run the migration plan
before using important data:

```bash
nexus storage-plan path/to/app.nx --storage sqlite
nexus storage-plan path/to/app.nx --storage sqlite --apply
nexus serve path/to/app.nx 127.0.0.1:5050 --storage sqlite
```

Treat the database file and its companion `-wal` and `-shm` files as user data.
The internal `nexus_schema_migrations` table records NexusLang-managed DDL
actions applied by `storage-plan --apply`; keep it with the database during
backup and restore.

## Example

The operational example is:

```text
nexuslang-src/examples/storage_backup_restore_inventory.nx
```

It defines a small inventory API:

- `POST /items`
- `GET /items`
- `GET /items/page`
- `GET /items/by-status`
- `GET /items/low-stock`
- `GET /items/:sku`
- `PUT /items/:sku`
- `DELETE /items/:sku`

The model uses storage features that matter for `0.2.x` compatibility:

- `sku: string unique`
- `status: string = "active" index`
- `quantity: int min 0`
- `unit_price: money min 1 kz`
- `warehouse: string?`

## JSON Backup

Stop the NexusLang server before copying data.

```bash
cp -a path/to/project/.nexus-data path/to/backups/nexus-data-$(date -u +%Y%m%dT%H%M%SZ)
```

For a served file named `inventory.nx`, the data directory is next to that
file, not necessarily next to the current terminal directory.

## JSON Restore

Stop the server, replace the data directory, then start the server again.

```bash
rm -rf path/to/project/.nexus-data
cp -a path/to/backups/nexus-data-20260526T000000Z path/to/project/.nexus-data
nexus check path/to/project/inventory.nx
nexus serve path/to/project/inventory.nx 127.0.0.1:5050
```

After restore, check at least one create, find/list, update, delete, and filter
route against a copy of the data before using the restored data for real work.

## Logical Export/Import

Use logical export/import when you need a portable data archive, a JSON-to-SQLite
move, or an environment seed that does not depend on physical storage layout.
Stop the server before exporting or importing.

```bash
nexus storage-export path/to/project/inventory.nx --storage json --output data.json
nexus storage-import path/to/project/inventory.nx --storage sqlite --input data.json --replace
```

The archive format is `nexus.storage.export.v1`. It contains declared model
records and native auth data when the program declares `auth`; it does not
include SQLite internals such as `nexus_schema_migrations` or physical index
names.

`storage-import` is intentionally replace-only in this MVP. It validates the
archive against the current program before writing data, applies SQLite storage
setup through `storage-plan` internals, then replaces declared model records.
For SQLite, the replace operation runs in a transaction.

After import, validate the target:

```bash
nexus check path/to/project/inventory.nx
nexus storage-plan path/to/project/inventory.nx --storage sqlite
nexus storage-export path/to/project/inventory.nx --storage sqlite --output verify.json
```

Only start serving the imported data when the plan reports no blockers and the
exported verification archive contains the expected records.

## SQLite Backup

Stop the process that owns the database, then copy the database and any
write-ahead log companions if they exist.

```bash
cp -a path/to/nexus.db path/to/backups/nexus.db
cp -a path/to/nexus.db-wal path/to/backups/nexus.db-wal 2>/dev/null || true
cp -a path/to/nexus.db-shm path/to/backups/nexus.db-shm 2>/dev/null || true
```

If a future integration keeps SQLite open during backup, use SQLite's own
backup API or a consistent filesystem snapshot. A plain file copy is only safe
after the writer is stopped.

## SQLite Restore

Stop the process, restore the database and companion files, then validate with
the same route smoke used for JSON.

```bash
cp -a path/to/backups/nexus.db path/to/nexus.db
cp -a path/to/backups/nexus.db-wal path/to/nexus.db-wal 2>/dev/null || true
cp -a path/to/backups/nexus.db-shm path/to/nexus.db-shm 2>/dev/null || true
```

Do not depend on SQLite table names, internal index names, or raw SQL layout in
application code. The public promise is behavior parity for the supported route
subset plus the conservative `storage-plan` dry-run/apply contract documented
in `COMPATIBILITY.md`.

## SQLite Rollback And Migration History

`nexus storage-plan --storage sqlite --apply` creates the internal
`nexus_schema_migrations` ledger and records each safe DDL action with a stable
action ID. Running the same plan again should report `Actions: none` and leave
the ledger unchanged.

The ledger is for auditability and idempotence, not automatic rollback. To roll
back an applied SQLite storage change in `0.2.x`, stop the server and restore a
known-good `nexus.db` plus any matching `nexus.db-wal` and `nexus.db-shm` files
from backup. After restore, run:

```bash
nexus check path/to/project/inventory.nx
nexus storage-plan path/to/project/inventory.nx --storage sqlite
```

Only start serving the restored database when the plan reports no blockers.

## Supported Schema Evolution

Safe additive changes for `0.2.x`:

- add an optional field;
- add a field with a static default;
- add validation that still allows older records to be read;
- add `unique` or `index` metadata only after `nexus storage-plan --storage sqlite`
  reports no blockers for the copied dataset.

Breaking changes that need explicit data transformation:

- rename a model or field;
- remove a field still used by routes;
- add a required field without a default;
- change a field type;
- change money/date representation;
- rely on physical indexes for `index` fields.

## Verification

Run the operational smoke test from a source checkout or from an extracted
release package:

```bash
./scripts/smoke-storage-backup-restore.sh
./scripts/smoke-sqlite-backup-restore.sh
```

The JSON script serves `storage_backup_restore_inventory.nx`, creates two
records, backs up `.nexus-data`, mutates live storage, restores the backup, and
verifies that the restored item is readable again.

The SQLite script first validates `storage-plan` dry-run/apply behavior,
including the `nexus_schema_migrations` ledger and idempotent post-apply plan.
It then serves with `--storage sqlite`, creates records, backs up `nexus.db`
with any WAL/SHM companions after stopping the writer, mutates live storage,
restores the backup, and verifies route behavior plus a clean restored plan.
