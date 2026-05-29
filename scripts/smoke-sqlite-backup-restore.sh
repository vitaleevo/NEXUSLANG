#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${NEXUS_SQLITE_STORAGE_SMOKE_PORT:-5052}"
BASE="http://127.0.0.1:$PORT"
WORK_DIR=""
BACKUP_DIR=""
SERVER_PID=""
SERVER_LOG=""

fail() {
    echo "ERROR: $*" >&2
    if [ -n "$SERVER_LOG" ] && [ -f "$SERVER_LOG" ]; then
        echo ""
        echo "Server log:" >&2
        cat "$SERVER_LOG" >&2
    fi
    exit 1
}

cleanup() {
    stop_server
    if [ -n "$WORK_DIR" ] && [ -d "$WORK_DIR" ]; then
        case "$WORK_DIR" in
            /tmp/nexus-sqlite-backup-restore.*) rm -rf "$WORK_DIR" ;;
        esac
    fi
}
trap cleanup EXIT INT TERM

run() {
    echo ""
    echo "==> $*"
    "$@"
}

stop_server() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        for _ in $(seq 1 50); do
            if ! kill -0 "$SERVER_PID" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done
        if kill -0 "$SERVER_PID" 2>/dev/null; then
            kill -9 "$SERVER_PID" 2>/dev/null || true
        fi
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    SERVER_PID=""
}

find_nexus_bin() {
    if [ -x "$ROOT_DIR/bin/nexus" ]; then
        printf '%s\n' "$ROOT_DIR/bin/nexus"
        return
    fi

    if [ -f "$ROOT_DIR/nexuslang-src/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
        run cargo build --manifest-path "$ROOT_DIR/nexuslang-src/Cargo.toml" --release >&2
        printf '%s\n' "$ROOT_DIR/nexuslang-src/target/release/nexus"
        return
    fi

    if [ -x "$ROOT_DIR/nexuslang-src/target/release/nexus" ]; then
        printf '%s\n' "$ROOT_DIR/nexuslang-src/target/release/nexus"
        return
    fi

    fail "could not find bin/nexus or build it with cargo"
}

find_example() {
    if [ -f "$ROOT_DIR/examples/storage_backup_restore_inventory.nx" ]; then
        printf '%s\n' "$ROOT_DIR/examples/storage_backup_restore_inventory.nx"
        return
    fi

    if [ -f "$ROOT_DIR/nexuslang-src/examples/storage_backup_restore_inventory.nx" ]; then
        printf '%s\n' "$ROOT_DIR/nexuslang-src/examples/storage_backup_restore_inventory.nx"
        return
    fi

    fail "storage_backup_restore_inventory.nx not found"
}

start_server() {
    local nexus_bin="$1"
    local app_file="$2"
    local retries="${NEXUS_SQLITE_STORAGE_SMOKE_START_RETRIES:-60}"
    local interval="${NEXUS_SQLITE_STORAGE_SMOKE_START_INTERVAL_SEC:-0.5}"

    SERVER_LOG="$WORK_DIR/server.log"
    if curl -fsS "$BASE/health" >/dev/null 2>&1; then
        fail "port $PORT already has a healthy service before smoke server starts"
    fi
    "$nexus_bin" serve "$app_file" "127.0.0.1:$PORT" --storage sqlite >"$SERVER_LOG" 2>&1 &
    SERVER_PID="$!"

    for _ in $(seq 1 "$retries"); do
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            fail "server exited before becoming ready"
        fi
        if curl -fsS "$BASE/health" >/dev/null 2>&1; then
            return
        fi
        sleep "$interval"
    done

    fail "server did not become ready on $BASE"
}

http_body() {
    local method="$1"
    local path="$2"
    local body="${3:-}"

    if [ -n "$body" ]; then
        curl -fsS -X "$method" -H "Content-Type: application/json" -d "$body" "$BASE$path"
    else
        curl -fsS -X "$method" "$BASE$path"
    fi
}

http_status() {
    local method="$1"
    local path="$2"
    local body="${3:-}"

    if [ -n "$body" ]; then
        curl -s -o /dev/null -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" -d "$body" "$BASE$path"
    else
        curl -s -o /dev/null -w "%{http_code}" -X "$method" "$BASE$path"
    fi
}

assert_contains() {
    local value="$1"
    local expected="$2"
    local context="$3"

    case "$value" in
        *"$expected"*) ;;
        *) fail "$context did not contain '$expected': $value" ;;
    esac
}

assert_status() {
    local method="$1"
    local path="$2"
    local body="$3"
    local expected="$4"
    local actual

    actual="$(http_status "$method" "$path" "$body")"
    [ "$actual" = "$expected" ] || {
        fail "$method $path returned $actual, expected $expected"
    }
}

copy_sqlite_store() {
    local from_dir="$1"
    local to_dir="$2"
    local from_db="$from_dir/nexus.db"
    local to_db="$to_dir/nexus.db"

    [ -f "$from_db" ] || fail "SQLite database not found: $from_db"
    mkdir -p "$to_dir"
    rm -f "$to_db" "$to_db-wal" "$to_db-shm"
    cp -a "$from_db" "$to_db"
    if [ -e "$from_db-wal" ]; then
        cp -a "$from_db-wal" "$to_db-wal"
    fi
    if [ -e "$from_db-shm" ]; then
        cp -a "$from_db-shm" "$to_db-shm"
    fi
}

echo "=== NexusLang SQLite Backup/Restore Smoke ==="

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v cp >/dev/null 2>&1 || fail "cp is required"
command -v rm >/dev/null 2>&1 || fail "rm is required"

WORK_DIR="$(mktemp -d /tmp/nexus-sqlite-backup-restore.XXXXXX)"
BACKUP_DIR="$WORK_DIR/backup"
DATA_DIR="$WORK_DIR/.nexus-data"
mkdir -p "$BACKUP_DIR"

NEXUS_BIN="$(find_nexus_bin)"
SOURCE_EXAMPLE="$(find_example)"
APP_FILE="$WORK_DIR/storage_backup_restore_inventory.nx"
cp "$SOURCE_EXAMPLE" "$APP_FILE"

run "$NEXUS_BIN" check "$APP_FILE"

echo ""
echo "==> SQLite storage-plan dry-run"
dry_run="$("$NEXUS_BIN" storage-plan "$APP_FILE" --storage sqlite)"
assert_contains "$dry_run" "Mode: dry-run" "SQLite dry-run"
assert_contains "$dry_run" "nexus_schema_migrations" "SQLite dry-run"
assert_contains "$dry_run" "Blockers: none" "SQLite dry-run"
test ! -e "$DATA_DIR/nexus.db" || fail "SQLite dry-run must not create nexus.db"

echo ""
echo "==> SQLite storage-plan apply"
applied="$("$NEXUS_BIN" storage-plan "$APP_FILE" --storage sqlite --apply)"
assert_contains "$applied" "Mode: applied" "SQLite apply"
assert_contains "$applied" "nexus_schema_migrations" "SQLite apply"
test -f "$DATA_DIR/nexus.db" || fail "SQLite apply did not create nexus.db"

post_apply="$("$NEXUS_BIN" storage-plan "$APP_FILE" --storage sqlite)"
assert_contains "$post_apply" "Actions: none" "SQLite post-apply plan"
assert_contains "$post_apply" "Blockers: none" "SQLite post-apply plan"

echo ""
echo "==> start SQLite server on $BASE"
start_server "$NEXUS_BIN" "$APP_FILE"

http_body "POST" "/items" \
    '{"sku":"SKU-001","name":"Notebook Pro","quantity":4,"unit_price":{"amount":750000,"currency":"kz"},"warehouse":"Luanda"}' >/dev/null
http_body "POST" "/items" \
    '{"sku":"SKU-002","name":"Mouse USB","status":"reserved","quantity":18,"unit_price":{"amount":9000,"currency":"kz"}}' >/dev/null

items_before="$(http_body "GET" "/items")"
assert_contains "$items_before" "SKU-001" "initial SQLite list"
assert_contains "$items_before" "SKU-002" "initial SQLite list"

stop_server
run copy_sqlite_store "$DATA_DIR" "$BACKUP_DIR/.nexus-data"

echo ""
echo "==> restart SQLite server for live mutation"
start_server "$NEXUS_BIN" "$APP_FILE"

assert_status "DELETE" "/items/SKU-001" "" "200"
assert_status "GET" "/items/SKU-001" "" "404"
http_body "POST" "/items" \
    '{"sku":"SKU-003","name":"Keyboard USB","quantity":2,"unit_price":{"amount":15000,"currency":"kz"}}' >/dev/null
assert_status "GET" "/items/SKU-003" "" "200"

stop_server
run copy_sqlite_store "$BACKUP_DIR/.nexus-data" "$DATA_DIR"

restored_plan="$("$NEXUS_BIN" storage-plan "$APP_FILE" --storage sqlite)"
assert_contains "$restored_plan" "Actions: none" "SQLite restored plan"
assert_contains "$restored_plan" "Blockers: none" "SQLite restored plan"

echo ""
echo "==> restart SQLite server after restore"
start_server "$NEXUS_BIN" "$APP_FILE"

restored="$(http_body "GET" "/items/SKU-001")"
assert_contains "$restored" "Notebook Pro" "restored SQLite item"
assert_contains "$restored" '"warehouse":"Luanda"' "restored SQLite item"
assert_status "GET" "/items/SKU-003" "" "404"

low_stock="$(http_body "GET" "/items/low-stock?max_qty=5")"
assert_contains "$low_stock" "SKU-001" "SQLite low-stock filter after restore"

page="$(http_body "GET" "/items/page?limit=1&offset=0")"
assert_contains "$page" '"total":2' "SQLite paged list after restore"

echo ""
echo "SQLite backup/restore smoke passed."
