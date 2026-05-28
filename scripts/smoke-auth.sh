#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"
if [ -f "$CRATE_DIR/examples/auth_secure_crm.nx" ]; then
    AUTH_FILE="$CRATE_DIR/examples/auth_secure_crm.nx"
    DATA_DIR="$CRATE_DIR/examples/.nexus-data"
else
    AUTH_FILE="$ROOT_DIR/examples/auth_secure_crm.nx"
    DATA_DIR="$ROOT_DIR/examples/.nexus-data"
fi
SERVER_LOG=$(mktemp /tmp/nexus-auth-smoke-XXXX.log)
TMP_DIR=$(mktemp -d /tmp/nexus-auth-smoke-XXXX)
SERVER_PID=""
PASS=0
FAIL=0
BASE="http://127.0.0.1:5051"
LAST_HEADERS="$TMP_DIR/headers"
LAST_BODY="$TMP_DIR/body"

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$SERVER_LOG"
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

request_status() {
    local method="$1" path="$2" body="$3" expected="$4" desc="$5"
    shift 5
    local header_args=()
    for header in "$@"; do
        header_args+=("-H" "$header")
    done

    local code
    if [ -n "$body" ]; then
        code=$(curl -sS -D "$LAST_HEADERS" -o "$LAST_BODY" -w "%{http_code}" \
            -X "$method" -H "Content-Type: application/json" "${header_args[@]}" \
            -d "$body" "$BASE$path")
    else
        code=$(curl -sS -D "$LAST_HEADERS" -o "$LAST_BODY" -w "%{http_code}" \
            -X "$method" "${header_args[@]}" "$BASE$path")
    fi

    if [ "$code" = "$expected" ]; then
        echo "  PASS $desc ($method $path -> $code)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL $desc ($method $path -> $code, expected $expected)"
        cat "$LAST_BODY"
        echo ""
        FAIL=$((FAIL + 1))
    fi
}

json_field() {
    local field="$1"
    sed -n "s/.*\"$field\":\"\\([^\"]*\\)\".*/\\1/p" "$LAST_BODY" | head -1
}

session_cookie() {
    awk 'BEGIN { IGNORECASE = 1 } /^Set-Cookie:/ {
        sub(/\r$/, "")
        sub(/^Set-Cookie:[[:space:]]*/, "")
        split($0, parts, ";")
        print parts[1]
        exit
    }' "$LAST_HEADERS"
}

require_nonempty() {
    local value="$1" desc="$2"
    if [ -n "$value" ]; then
        echo "  PASS $desc present"
        PASS=$((PASS + 1))
    else
        echo "  FAIL $desc missing"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== NexusLang Native Auth HTTP Smoke ==="
echo ""

if [ -x "$ROOT_DIR/bin/nexus" ]; then
    BIN="$ROOT_DIR/bin/nexus"
else
    echo "Building release binary..."
    cd "$CRATE_DIR"
    cargo build --release 2>&1 | tail -1
    BIN="$CRATE_DIR/target/release/nexus"
fi

rm -rf "$DATA_DIR"

echo "Starting auth server..."
"$BIN" serve "$AUTH_FILE" 127.0.0.1:5051 > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 30); do
    if curl -sS "$BASE/__health" >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
done

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "FAIL server did not start"
    cat "$SERVER_LOG"
    exit 1
fi

request_status "GET" "/__health" "" "200" "health"

request_status "POST" "/auth/register" \
    '{"email":"ana.auth@example.com","name":"Ana Auth","role":"admin","password":"strong-password-123"}' \
    "201" "register admin"
COOKIE="$(session_cookie)"
CSRF="$(json_field csrf_token)"
require_nonempty "$COOKIE" "session cookie"
require_nonempty "$CSRF" "csrf token"

request_status "GET" "/me" "" "200" "cookie session reads current user" \
    "Cookie: $COOKIE"

request_status "POST" "/auth/logout" "" "403" "cookie logout without csrf is blocked" \
    "Cookie: $COOKIE"

request_status "POST" "/auth/logout" "" "200" "cookie logout with csrf" \
    "Cookie: $COOKIE" \
    "X-Nexus-CSRF-Token: $CSRF"

request_status "GET" "/me" "" "401" "cookie session is revoked" \
    "Cookie: $COOKIE"

request_status "POST" "/auth/login" \
    '{"email":"ana.auth@example.com","password":"strong-password-123"}' \
    "200" "login admin"
TOKEN="$(json_field token)"
require_nonempty "$TOKEN" "bearer token"

request_status "GET" "/admin/users" "" "200" "bearer token reaches admin route" \
    "Authorization: Bearer $TOKEN"

request_status "POST" "/auth/logout" "" "200" "bearer logout does not require csrf" \
    "Authorization: Bearer $TOKEN"

request_status "GET" "/me" "" "401" "bearer token is revoked" \
    "Authorization: Bearer $TOKEN"

for _ in $(seq 1 5); do
    request_status "POST" "/auth/login" \
        '{"email":"ana.auth@example.com","password":"wrong-password-123"}' \
        "401" "failed login attempt"
done

request_status "POST" "/auth/login" \
    '{"email":"ana.auth@example.com","password":"wrong-password-123"}' \
    "429" "login rate limit"

echo ""
echo "--- Result: $PASS passed, $FAIL failed ---"
[ "$FAIL" -eq 0 ] || exit 1
