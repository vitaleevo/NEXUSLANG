#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/nexuslang-src"
OUT_DIR="$CRATE_DIR/web"

rustup target add wasm32-unknown-unknown
cargo build \
  --manifest-path "$CRATE_DIR/Cargo.toml" \
  --release \
  --target wasm32-unknown-unknown \
  --lib

mkdir -p "$OUT_DIR"
cp \
  "$CRATE_DIR/target/wasm32-unknown-unknown/release/nexuslang.wasm" \
  "$OUT_DIR/nexuslang_playground.wasm"

WASM_SIZE="$(wc -c < "$OUT_DIR/nexuslang_playground.wasm" | tr -d '[:space:]')"
echo "WASM atualizado em $OUT_DIR/nexuslang_playground.wasm ($WASM_SIZE bytes)"
