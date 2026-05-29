#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPATIBILITY_PATH="$ROOT_DIR/COMPATIBILITY.md"
ROADMAP_PATH="$ROOT_DIR/nexuslang-src/ROADMAP.md"
TEST_PATH="$ROOT_DIR/nexuslang-src/tests/core.rs"
GUIDE_PATH="$ROOT_DIR/STORAGE_BACKUP_RESTORE.md"
EXAMPLE_PATH="$ROOT_DIR/nexuslang-src/examples/storage_backup_restore_inventory.nx"
SMOKE_PATH="$ROOT_DIR/scripts/smoke-storage-backup-restore.sh"
SQLITE_SMOKE_PATH="$ROOT_DIR/scripts/smoke-sqlite-backup-restore.sh"

fail() {
    echo "ERROR: $*" >&2
    exit 1
}

require_file() {
    [ -f "$1" ] || fail "missing file: $1"
}

require_text() {
    local path="$1"
    local text="$2"

    grep -Fq -- "$text" "$path" || fail "$path is missing required text: $text"
}

echo "=== NexusLang Storage Compatibility Policy Validation ==="

require_file "$COMPATIBILITY_PATH"
require_file "$ROADMAP_PATH"
require_file "$TEST_PATH"
require_file "$GUIDE_PATH"
require_file "$EXAMPLE_PATH"
require_file "$SMOKE_PATH"
require_file "$SQLITE_SMOKE_PATH"

require_text "$COMPATIBILITY_PATH" "## Storage"
require_text "$COMPATIBILITY_PATH" "### JSON Storage"
require_text "$COMPATIBILITY_PATH" "### SQLite Storage"
require_text "$COMPATIBILITY_PATH" "### SQLite Migration Plan MVP"
require_text "$COMPATIBILITY_PATH" "### Migration Policy For 0.2.x"
require_text "$COMPATIBILITY_PATH" "### Backup And Restore Expectations"
require_text "$COMPATIBILITY_PATH" "### Storage Release Gate"
require_text "$COMPATIBILITY_PATH" "optional fields may be absent in older data"
require_text "$COMPATIBILITY_PATH" "fields with static defaults may be absent in older data"
require_text "$COMPATIBILITY_PATH" "add a required field without a default"
require_text "$COMPATIBILITY_PATH" ".nexus-data"
require_text "$COMPATIBILITY_PATH" "-wal"
require_text "$COMPATIBILITY_PATH" "-shm"
require_text "$COMPATIBILITY_PATH" "validate-public-release-install.sh"
require_text "$COMPATIBILITY_PATH" "STORAGE_BACKUP_RESTORE.md"
require_text "$COMPATIBILITY_PATH" "nexus storage-plan path/to/app.nx --storage sqlite"
require_text "$COMPATIBILITY_PATH" "nexus_schema_migrations"

require_text "$ROADMAP_PATH" "Define the JSON/SQLite storage compatibility policy more concretely"
require_text "$ROADMAP_PATH" "storage_schema_evolution_allows_additive_optional_and_defaulted_fields"
require_text "$ROADMAP_PATH" "storage_backup_restore_inventory.nx"
require_text "$TEST_PATH" "fn storage_schema_evolution_allows_additive_optional_and_defaulted_fields()"
require_text "$TEST_PATH" "fn sqlite_storage_matches_json_storage_for_crud_and_critical_filters()"
require_text "$TEST_PATH" "fn sqlite_migration_plan_dry_run_and_apply_create_safe_schema()"
require_text "$GUIDE_PATH" "nexuslang-src/examples/storage_backup_restore_inventory.nx"
require_text "$GUIDE_PATH" "nexus storage-plan path/to/app.nx --storage sqlite"
require_text "$GUIDE_PATH" "./scripts/smoke-storage-backup-restore.sh"
require_text "$GUIDE_PATH" "./scripts/smoke-sqlite-backup-restore.sh"
require_text "$GUIDE_PATH" "nexus_schema_migrations"
require_text "$EXAMPLE_PATH" "model InventoryItem"
require_text "$EXAMPLE_PATH" "route DELETE /items/:sku"
require_text "$SMOKE_PATH" "Storage backup/restore smoke passed."
require_text "$SQLITE_SMOKE_PATH" "SQLite backup/restore smoke passed."

echo "Storage compatibility policy validation passed."
