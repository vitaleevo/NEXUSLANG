# NexusLang Storage Backup And Restore

This guide is the operational companion to the `0.1.x` storage compatibility
policy in `COMPATIBILITY.md`.

## Scope

The public `nexus serve <file.nx>` command uses JSON storage by default. It
creates `.nexus-data` next to the served `.nx` file, with one lower-case model
file per model:

```text
.nexus-data/inventoryitem.json
```

SQLite is covered by JSON/SQLite behavior parity tests and by the compatibility
policy, but `0.1.x` does not yet expose a stable CLI flag for choosing SQLite
from `nexus serve`. If an integration uses the SQLite backend directly, treat
the database file and its companion `-wal` and `-shm` files as user data.

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

The model uses storage features that matter for `0.1.x` compatibility:

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
`0.1.x`. The public promise is behavior parity for the supported route subset.

## Supported Schema Evolution

Safe additive changes for `0.1.x`:

- add an optional field;
- add a field with a static default;
- add validation that still allows older records to be read.

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
```

The script serves `storage_backup_restore_inventory.nx`, creates two records,
backs up `.nexus-data`, mutates live storage, restores the backup, and verifies
that the restored item is readable again.
