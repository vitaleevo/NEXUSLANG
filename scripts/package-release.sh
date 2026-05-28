#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"
DIST_DIR="$ROOT_DIR/dist"
PACKAGE_VERSION="$(
    awk -F '=' '
        /^\[package\]/ { in_package = 1; next }
        /^\[/ { in_package = 0 }
        in_package && /^[[:space:]]*version[[:space:]]*=/ {
            gsub(/[[:space:]"]/, "", $2)
            print $2
            exit
        }
    ' "$CRATE_DIR/Cargo.toml"
)"

[ -n "$PACKAGE_VERSION" ] || {
    echo "Could not read package version from $CRATE_DIR/Cargo.toml" >&2
    exit 1
}

PACKAGE_NAME="nexuslang-v$PACKAGE_VERSION-local-release"
PACKAGE_DIR="$DIST_DIR/$PACKAGE_NAME"
ARCHIVE_NAME="$PACKAGE_NAME.tar.gz"
ARCHIVE_PATH="$DIST_DIR/$ARCHIVE_NAME"
CHECKSUM_NAME="$ARCHIVE_NAME.sha256"
CHECKSUM_PATH="$DIST_DIR/$CHECKSUM_NAME"

run() {
    echo ""
    echo "==> $*"
    "$@"
}

safe_clean() {
    mkdir -p "$DIST_DIR"
    case "$PACKAGE_DIR" in
        "$DIST_DIR"/*) ;;
        *)
            echo "Refusing to clean unexpected package path: $PACKAGE_DIR" >&2
            exit 1
            ;;
    esac

    rm -rf "$DIST_DIR"/nexuslang-v*-local-release "$DIST_DIR"/nexuslang-local-release
    rm -f "$DIST_DIR"/nexuslang-v*-local-release.tar.gz
    rm -f "$DIST_DIR"/nexuslang-v*-local-release.tar.gz.sha256
    rm -f "$DIST_DIR"/nexuslang-v*-local-release.tar.gz.asc
    rm -f "$DIST_DIR"/nexuslang-v*-local-release.tar.gz.sha256.asc
    rm -f "$DIST_DIR"/nexuslang-v*-local-release.tar.gz.dry-run-public-key.asc
    rm -f "$DIST_DIR"/nexuslang-local-release.tar.gz
    rm -f "$DIST_DIR"/nexuslang-local-release.tar.gz.sha256
    rm -f "$DIST_DIR"/nexuslang-local-release.tar.gz.asc
    rm -f "$DIST_DIR"/nexuslang-local-release.tar.gz.sha256.asc
    rm -f "$DIST_DIR"/nexuslang-local-release.tar.gz.dry-run-public-key.asc
}

copy_docs() {
    mkdir -p "$PACKAGE_DIR/docs"
    cp "$ROOT_DIR/README.md" "$PACKAGE_DIR/docs/README.md"
    cp "$ROOT_DIR/RELEASE_NOTES.md" "$PACKAGE_DIR/docs/RELEASE_NOTES.md"
    cp "$ROOT_DIR/VERSIONING.md" "$PACKAGE_DIR/docs/VERSIONING.md"
    cp "$ROOT_DIR/COMPATIBILITY.md" "$PACKAGE_DIR/docs/COMPATIBILITY.md"
    cp "$ROOT_DIR/PACKAGE_MANAGER.md" "$PACKAGE_DIR/docs/PACKAGE_MANAGER.md"
    cp "$ROOT_DIR/STORAGE_BACKUP_RESTORE.md" "$PACKAGE_DIR/docs/STORAGE_BACKUP_RESTORE.md"
    cp "$ROOT_DIR/SIGNING.md" "$PACKAGE_DIR/docs/SIGNING.md"
    cp "$ROOT_DIR/GITHUB_RELEASE.md" "$PACKAGE_DIR/docs/GITHUB_RELEASE.md"
    cp "$ROOT_DIR/RELEASE.md" "$PACKAGE_DIR/docs/RELEASE.md"
    cp "$ROOT_DIR/PLANO_NEXUSLANG.md" "$PACKAGE_DIR/docs/PLANO_NEXUSLANG.md"
    cp "$ROOT_DIR/MEMORIA_NEXUSLANG.md" "$PACKAGE_DIR/docs/MEMORIA_NEXUSLANG.md"
    cp "$CRATE_DIR/ROADMAP.md" "$PACKAGE_DIR/docs/ROADMAP.md"
    cp "$CRATE_DIR/SYNTAX_1_0.md" "$PACKAGE_DIR/docs/SYNTAX_1_0.md"
}

copy_runtime_assets() {
    mkdir -p "$PACKAGE_DIR/bin"
    mkdir -p "$PACKAGE_DIR/examples"
    mkdir -p "$PACKAGE_DIR/nexuslang-src"
    mkdir -p "$PACKAGE_DIR/scripts"
    mkdir -p "$PACKAGE_DIR/stdlib"

    cp "$CRATE_DIR/target/release/nexus" "$PACKAGE_DIR/bin/nexus"
    cp "$ROOT_DIR/nexuslang-playground.html" "$PACKAGE_DIR/nexuslang-playground.html"
    cp "$ROOT_DIR/nexuslang-playground.js" "$PACKAGE_DIR/nexuslang-playground.js"
    cp -R "$CRATE_DIR/web" "$PACKAGE_DIR/nexuslang-src/web"
    cp -R "$CRATE_DIR/stdlib/." "$PACKAGE_DIR/stdlib/"
    find "$CRATE_DIR/examples" -maxdepth 1 -type f -name "*.nx" -exec cp {} "$PACKAGE_DIR/examples/" \;
    cp "$ROOT_DIR/scripts/validate-release-second-env.sh" "$PACKAGE_DIR/scripts/validate-release-second-env.sh"
    cp "$ROOT_DIR/scripts/sign-release-artifacts.sh" "$PACKAGE_DIR/scripts/sign-release-artifacts.sh"
    cp "$ROOT_DIR/scripts/connect-github-release.sh" "$PACKAGE_DIR/scripts/connect-github-release.sh"
    cp "$ROOT_DIR/scripts/release-dry-run-strict.sh" "$PACKAGE_DIR/scripts/release-dry-run-strict.sh"
    cp "$ROOT_DIR/scripts/validate-public-release-install.sh" "$PACKAGE_DIR/scripts/validate-public-release-install.sh"
    cp "$ROOT_DIR/scripts/validate-storage-compatibility-policy.sh" "$PACKAGE_DIR/scripts/validate-storage-compatibility-policy.sh"
    cp "$ROOT_DIR/scripts/smoke-storage-backup-restore.sh" "$PACKAGE_DIR/scripts/smoke-storage-backup-restore.sh"
    cp "$ROOT_DIR/scripts/smoke-auth.sh" "$PACKAGE_DIR/scripts/smoke-auth.sh"
}

write_package_smoke() {
    cat > "$PACKAGE_DIR/scripts/smoke-package.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run() {
    echo ""
    echo "==> $*"
    "$@"
}

if find "$ROOT_DIR" -name ".nexus-data" -print -quit | grep -q .; then
    echo "Generated .nexus-data storage must not be present in the package." >&2
    exit 1
fi

run "$ROOT_DIR/bin/nexus" --help
package_manager_tmp="$(mktemp -d)"
trap 'rm -rf "$package_manager_tmp"' EXIT
run "$ROOT_DIR/bin/nexus" new "$package_manager_tmp/crm_core"
run "$ROOT_DIR/bin/nexus" new "$package_manager_tmp/package_manager_app"
(
    cd "$package_manager_tmp/package_manager_app"
    mkdir -p .nexus/packages/stale_core
    run "$ROOT_DIR/bin/nexus" add crm-core --path ../crm_core
    run "$ROOT_DIR/bin/nexus" add audit_core --registry audit_core@0.1.0
    run "$ROOT_DIR/bin/nexus" install
    run "$ROOT_DIR/bin/nexus" update
    test -f nexus.toml
    test -f nexus.lock
    test -f .nexus/packages/crm-core/PACKAGE.txt
    test -f .nexus/packages/audit_core/PACKAGE.txt
    test ! -e .nexus/packages/stale_core
)
run "$ROOT_DIR/bin/nexus" check "$ROOT_DIR/examples/erp_basico.nx"
run "$ROOT_DIR/bin/nexus" run "$ROOT_DIR/examples/erp_basico.nx"
cat > "$package_manager_tmp/std_math.nx" <<'NX'
import abs from "std/math"
import contains from "std/string"
import contains_int from "std/collections"
import is_email from "std/validation"
import is_iso_date from "std/date"
import is_positive_money from "std/money"
import number_is_even from "std/number"
import inventory_stock_status from "std/inventory"
import crm_is_active_status from "std/crm"
import invoice_line_total from "std/invoice"
import json_pair from "std/json"
import json_bool from "std/json"
import json_object_1 from "std/json"
import csv_row_2 from "std/csv"
import http_status_text from "std/http"
import crypto_sha256_hex from "std/crypto"
import time_seconds_between from "std/time"
import env_get_or from "std/env"
import log_info from "std/log"
import path_basename from "std/path"
import sales_line_total from "std/sales"
import tax_amount from "std/tax"
import discount_final_percent from "std/discount"
import payment_status from "std/payment"
import banking_is_debit from "std/banking"
import accounting_is_balanced from "std/accounting"
import ledger_side from "std/ledger"
import shipping_status from "std/shipping"
import warehouse_utilization_percent from "std/warehouse"
import procurement_status from "std/procurement"
import supplier_is_active_status from "std/supplier"
import customer_balance_status from "std/customer"
import project_progress_percent from "std/project"
import task_priority_label from "std/task"
import kpi_percent from "std/kpi"
import report_money_line from "std/report"
import pagination_offset from "std/pagination"
import security_is_https from "std/security"
import config_flag_enabled from "std/config"
import commerce_cart_total_3 from "std/commerce"
print(abs(-9))
print(contains("NexusLang", "Lang"))
print(contains_int([1, 2, 3], 2))
print(is_email("ana@example.com"))
print(is_iso_date("2026-05-27"))
print(is_positive_money(100 kz))
print(number_is_even(10))
print(inventory_stock_status(3, 5))
print(crm_is_active_status(" ACTIVE "))
print(invoice_line_total(2, 150 kz))
print(json_object_1(json_pair("ok", json_bool(true))))
print(csv_row_2("Ana", "Luanda, Angola"))
print(http_status_text(201))
print(crypto_sha256_hex("nexus"))
print(time_seconds_between(10, 16))
print(env_get_or("NEXUS_STDLIB_PACKAGE_MISSING", "fallback"))
print(log_info("ok"))
print(path_basename("/tmp/report.csv"))
print(sales_line_total(2, 100 kz))
print(tax_amount(1000 kz, 0.1))
print(discount_final_percent(1000 kz, 0.1))
print(payment_status(100 kz, 40 kz))
print(banking_is_debit(-10 kz))
print(accounting_is_balanced(100 kz, 100 kz))
print(ledger_side(-5 kz))
print(shipping_status(3, 5))
print(warehouse_utilization_percent(100, 25))
print(procurement_status(3, 5))
print(supplier_is_active_status(" ACTIVE "))
print(customer_balance_status(0 kz))
print(project_progress_percent(2, 4))
print(task_priority_label(1))
print(kpi_percent(3, 4))
print(report_money_line("total", 99 kz))
print(pagination_offset(3, 20))
print(security_is_https("https://nexus.local"))
print(config_flag_enabled(" YES "))
print(commerce_cart_total_3(10 kz, 20 kz, 30 kz))
NX
run "$ROOT_DIR/bin/nexus" check "$package_manager_tmp/std_math.nx"
stdlib_output="$("$ROOT_DIR/bin/nexus" run "$package_manager_tmp/std_math.nx")"
expected_stdlib_output="$(
    printf '%s\n' \
        9 \
        true \
        true \
        true \
        true \
        true \
        true \
        reorder \
        true \
        "300.00 KZ" \
        '{"ok":true}' \
        'Ana,"Luanda, Angola"' \
        Created \
        f5cfcb570b7edac2ed16e1a025d50155d6148de7397f4068790cdfc142300070 \
        6 \
        fallback \
        "[INFO] ok" \
        report.csv \
        "200.00 KZ" \
        "100.00 KZ" \
        "900.00 KZ" \
        partial \
        true \
        true \
        credit \
        partial \
        "25.00" \
        partial \
        true \
        settled \
        "50.00" \
        high \
        "75.00" \
        "total: 99.00 KZ" \
        40 \
        true \
        true \
        "60.00 KZ"
)"
[ "$stdlib_output" = "$expected_stdlib_output" ] || {
    echo "Unexpected stdlib smoke output:" >&2
    printf '%s\n' "$stdlib_output" >&2
    exit 1
}
run "$ROOT_DIR/bin/nexus" check "$ROOT_DIR/examples/auth_secure_crm.nx"
run "$ROOT_DIR/scripts/smoke-auth.sh"
run "$ROOT_DIR/bin/nexus" check "$ROOT_DIR/examples/storage_backup_restore_inventory.nx"
run "$ROOT_DIR/scripts/smoke-storage-backup-restore.sh"
run node --check "$ROOT_DIR/nexuslang-playground.js"
test -s "$ROOT_DIR/nexuslang-src/web/nexuslang_playground.wasm"

echo ""
echo "Package smoke passed."
EOF

    chmod +x "$PACKAGE_DIR/scripts/smoke-package.sh"
}

write_manifest() {
    local generated_at
    local wasm_bytes
    local archive_bytes

    generated_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    wasm_bytes="$(wc -c < "$CRATE_DIR/web/nexuslang_playground.wasm" | tr -d '[:space:]')"

    cat > "$PACKAGE_DIR/README.md" <<EOF
# NexusLang Local Release

Generated at: $generated_at
Version: $PACKAGE_VERSION

## Contents

- bin/nexus: release CLI binary
- nexuslang-playground.html and nexuslang-playground.js: browser playground
- nexuslang-src/web/nexuslang_playground.wasm: WebAssembly runtime artifact
- examples/: language/runtime examples
- stdlib/: installed NexusLang standard library modules
- docs/: public guide, release policy, compatibility, signing, roadmap, syntax, and memory
- scripts/: local validation helpers

## Quick smoke

\`\`\`bash
./bin/nexus --help
./bin/nexus check examples/erp_basico.nx
./scripts/smoke-package.sh
python3 -m http.server 8091 --bind 127.0.0.1
\`\`\`

Open http://127.0.0.1:8091/nexuslang-playground.html and confirm the page
reports WASM pronto.

Archive: $ARCHIVE_NAME
Checksum: $CHECKSUM_NAME
WASM bytes: $wasm_bytes

## More Docs

- docs/README.md: public install and getting-started guide
- docs/RELEASE_NOTES.md: release notes and known limitations
- docs/VERSIONING.md: version and tag policy
- docs/COMPATIBILITY.md: language/runtime/storage compatibility contract
- docs/PACKAGE_MANAGER.md: package manager manifest, lockfile, path, and registry contract
- docs/STORAGE_BACKUP_RESTORE.md: operational storage backup and restore guide
- docs/SIGNING.md: artifact signing path
- docs/GITHUB_RELEASE.md: GitHub, CI, and maintained-key release setup
- docs/RELEASE.md: release gate and checklist
- scripts/connect-github-release.sh: GitHub origin/create/push helper
- scripts/validate-release-second-env.sh: Docker-based second-environment check
- scripts/sign-release-artifacts.sh: GPG signing helper for publishers
- scripts/release-dry-run-strict.sh: strict public-release preflight and dry-run
- scripts/validate-public-release-install.sh: public GitHub Release install validation
- scripts/validate-storage-compatibility-policy.sh: storage compatibility policy gate
- scripts/smoke-storage-backup-restore.sh: storage backup/restore smoke test
EOF

    cat > "$PACKAGE_DIR/PACKAGE_MANIFEST.txt" <<EOF
package=$PACKAGE_NAME
package_version=$PACKAGE_VERSION
generated_at=$generated_at
wasm_bytes=$wasm_bytes
archive=$ARCHIVE_NAME
checksum=$CHECKSUM_NAME
cli_binary=bin/nexus
playground=nexuslang-playground.html
stdlib=stdlib
EOF

    (
        cd "$DIST_DIR"
        tar -czf "$ARCHIVE_NAME" "$PACKAGE_NAME"
        sha256sum "$ARCHIVE_NAME" > "$CHECKSUM_NAME"
    )

    archive_bytes="$(wc -c < "$ARCHIVE_PATH" | tr -d '[:space:]')"
    archive_sha256="$(cut -d ' ' -f 1 "$CHECKSUM_PATH")"
    echo ""
    echo "Package ready: $ARCHIVE_PATH ($archive_bytes bytes)"
    echo "Checksum: $CHECKSUM_PATH ($archive_sha256)"
    echo "WASM artifact: $CRATE_DIR/web/nexuslang_playground.wasm ($wasm_bytes bytes)"
}

echo "=== NexusLang Local Release Package ==="
run node --check "$ROOT_DIR/nexuslang-playground.js"
run "$ROOT_DIR/scripts/build-playground-wasm.sh"
run cargo build --manifest-path "$CRATE_DIR/Cargo.toml" --release

safe_clean
copy_docs
copy_runtime_assets
write_package_smoke
write_manifest
