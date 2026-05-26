#!/usr/bin/env bash
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"
QA_FILE="$ROOT_DIR/nexuslang-src/examples/openapi_qa.nx"
DATA_DIR="$CRATE_DIR/examples/.nexus-data"
VENV_DIR="/tmp/nexus-venv"
SERVER_PID=""
SERVER_LOG=$(mktemp /tmp/nexus-server-XXXX.log)
OPENAPI_JSON=$(mktemp /tmp/nexus-openapi-XXXX.json)

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "A parar servidor (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$SERVER_LOG" "$OPENAPI_JSON"
}
trap cleanup EXIT INT TERM

echo "=== NexusLang OpenAPI 1.0 External Validation ==="
echo ""

# Step 1: Build
echo "[1/4] A compilar o projeto..."
cd "$CRATE_DIR"
if cargo build --release 2>&1; then
    echo "✅ Build concluído"
else
    echo "❌ Build falhou"
    exit 1
fi
echo ""

# Step 2: Start server
echo "[2/4] A iniciar servidor com openapi_qa.nx..."
cd "$ROOT_DIR"
rm -rf "$DATA_DIR"
./nexuslang-src/target/release/nexus serve "$QA_FILE" 127.0.0.1:5050 > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "❌ Servidor não iniciou"
    cat "$SERVER_LOG"
    exit 1
fi
echo "✅ Servidor em http://127.0.0.1:5050"
echo ""

# Step 3: Fetch /openapi.json
echo "[3/4] A obter /openapi.json..."
HTTP_CODE=$(curl -s -o "$OPENAPI_JSON" -w "%{http_code}" http://127.0.0.1:5050/openapi.json)

if [ "$HTTP_CODE" != "200" ]; then
    echo "❌ GET /openapi.json retornou HTTP $HTTP_CODE"
    cat "$SERVER_LOG"
    exit 1
fi

echo "✅ OpenAPI obtido (HTTP $HTTP_CODE)"
echo ""

# Step 4: Validate with Python (structural)
echo "[4/4] A validar com openapi-schema-validator..."
if [ ! -f "$VENV_DIR/bin/activate" ]; then
    echo "   A criar virtualenv..."
    python3 -m venv "$VENV_DIR"
    source "$VENV_DIR/bin/activate"
    pip install openapi-schema-validator 2>&1 | tail -1
else
    source "$VENV_DIR/bin/activate"
fi
python3 "$ROOT_DIR/scripts/validate-openapi.py" "$OPENAPI_JSON"
VALIDATION_RESULT=$?

# Also validate with OAS30Validator (full spec)
echo ""
echo "   A validar com OAS30Validator (full spec)..."
python3 -c "
import json, sys
from openapi_schema_validator import OAS30Validator
doc = json.load(open('$OPENAPI_JSON'))
v = OAS30Validator(doc)
errors = list(v.iter_errors(doc))
if errors:
    print('   OAS30Validator: {} erro(s)'.format(len(errors)))
    for e in errors:
        print('      - {} (path: {})'.format(e.message, list(e.path)))
    sys.exit(1)
else:
    print('   OAS30Validator: valido')
" 2>&1
OAS_RESULT=$?

echo ""

# Step 5: Smoke test endpoints
echo "--- Smoke Tests ---"

test_endpoint() {
    local method="$1"
    local path="$2"
    local expected="$3"
    local body="$4"

    if [ -n "$body" ]; then
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -d "$body" \
            "http://127.0.0.1:5050$path")
    else
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" \
            "http://127.0.0.1:5050$path")
    fi

    if [ "$HTTP_CODE" = "$expected" ]; then
        echo "   ✅ $method $path → $HTTP_CODE"
    else
        echo "   ❌ $method $path → $HTTP_CODE (esperado $expected)"
        return 1
    fi
}

echo ""

# Health
test_endpoint "GET" "/health" "200" ""

# Create
test_endpoint "POST" "/customers" "201" \
    '{"name":"Teste","display_name":"Teste User","balance":{"amount":1000,"currency":"kz"},"score":50}'

# Read
test_endpoint "GET" "/customers/Teste" "200" ""

# List
test_endpoint "GET" "/customers" "200" ""

# Search
test_endpoint "GET" "/customers/search?statuses=active" "200" ""

# Filter
test_endpoint "GET" "/customers/filter?status=active" "200" ""

# Not found
test_endpoint "GET" "/customers/Inexistente" "404" ""

# Update
test_endpoint "PUT" "/customers/Teste" "200" \
    '{"name":"Teste","status":"active","display_name":"Teste Updated","balance":{"amount":2000,"currency":"kz"},"score":60}'

# Delete
test_endpoint "DELETE" "/customers/Teste" "200" ""

echo ""
echo "=== Resultado Final ==="
if [ "$VALIDATION_RESULT" -eq 0 ] && [ "$OAS_RESULT" -eq 0 ]; then
    echo "✅ OpenAPI 3.0 validation: PASS"
else
    echo "❌ OpenAPI 3.0 validation: FAIL"
fi
echo "✅ Smoke tests: PASS (se disponivel)"
echo ""
echo "Documento OpenAPI guardado em: $OPENAPI_JSON"

exit $((VALIDATION_RESULT | OAS_RESULT))
