#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"
QA_FILE="$CRATE_DIR/examples/openapi_qa.nx"
DATA_DIR="$CRATE_DIR/examples/.nexus-data"
SERVER_LOG=$(mktemp /tmp/nexus-smoke-XXXX.log)
SERVER_PID=""
PASS=0
FAIL=0

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$SERVER_LOG"
}
trap cleanup EXIT INT TERM

BASE="http://127.0.0.1:5050"

test_ok() {
    local method="$1" path="$2" body="$3" desc="$4"
    local code
    if [ -n "$body" ]; then
        code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" -d "$body" "$BASE$path")
    else
        code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "$BASE$path")
    fi
    if [ "$code" = "200" ] || [ "$code" = "201" ]; then
        echo "  ✅ $desc ($method $path → $code)"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $desc ($method $path → $code, esperado 200/201)"
        FAIL=$((FAIL + 1))
    fi
}

test_status() {
    local method="$1" path="$2" body="$3" expected="$4" desc="$5"
    local code
    if [ -n "$body" ]; then
        code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" -d "$body" "$BASE$path")
    else
        code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "$BASE$path")
    fi
    if [ "$code" = "$expected" ]; then
        echo "  ✅ $desc ($method $path → $code)"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $desc ($method $path → $code, esperado $expected)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== NexusLang Smoke Tests ==="
echo ""

# Build & start
echo "A compilar..."
cd "$CRATE_DIR"
cargo build --release 2>&1 | tail -1
BIN="$CRATE_DIR/target/release/nexus"

rm -rf "$DATA_DIR"

echo "A iniciar servidor..."
"$BIN" serve "$QA_FILE" 127.0.0.1:5050 > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "❌ Servidor não iniciou"
    cat "$SERVER_LOG"
    exit 1
fi

# Tests
echo ""
echo "--- Health & OpenAPI ---"
test_ok "GET" "/health" "" "Health check"
test_ok "GET" "/openapi.json" "" "OpenAPI document"

echo ""
echo "--- CRUD ---"
test_ok "POST" "/customers" \
    '{"name":"Ana","display_name":"Ana Silva","balance":{"amount":1500,"currency":"kz"},"score":85}' \
    "Create customer"
test_ok "GET" "/customers/Ana" "" "Find customer"
test_ok "PUT" "/customers/Ana" \
    '{"name":"Ana","status":"active","display_name":"Ana Updated","balance":{"amount":2000,"currency":"kz"},"score":90}' \
    "Update customer"
test_ok "DELETE" "/customers/Ana" "" "Delete customer"

echo ""
echo "--- Listagens ---"
test_ok "GET" "/customers" "" "List all"
test_ok "GET" "/customers/list?limit=5&offset=0" "" "List paginated"
test_ok "GET" "/customers/page?limit=5&offset=0" "" "List paged with total"

echo ""
echo "--- Filtros ---"
test_ok "GET" "/customers/search?statuses=active" "" "Filter by where_in"
test_ok "GET" "/customers/filter?status=active&limit=10&offset=0" "" "Filter optional where"
test_ok "GET" "/customers/search-not?status=inactive" "" "Filter by where_not"
test_ok "GET" "/customers/search-compare?min_score=50" "" "Filter by compare"
test_ok "GET" "/customers/search-range?min_balance=100:kz&max_balance=5000:kz" "" "Filter by range"
test_ok "GET" "/customers/search-any?status=active&min_score=50" "" "Filter by OR"

echo ""
echo "--- Erros ---"
test_status "GET" "/customers/Inexistente" "" "404" "Find non-existent"
test_status "DELETE" "/customers/Inexistente" "" "404" "Delete non-existent"
test_status "PUT" "/customers/Inexistente" \
    '{"name":"Inexistente","status":"active","display_name":"XY","balance":{"amount":1000,"currency":"kz"},"score":50}' "404" "Update non-existent"

echo ""
echo "--- Resultado: $PASS passed, $FAIL failed ---"
[ "$FAIL" -eq 0 ] || exit 1
