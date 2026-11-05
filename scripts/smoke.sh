#!/usr/bin/env bash
# Smoke: testes + demos rápidas (host only — sem QEMU).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo test =="
cargo test --locked --quiet

echo "== syscalls =="
cargo run --locked --quiet -- syscalls | sed -n '1,5p'

echo "== demo =="
cargo run --locked --quiet --release -- demo | tail -15

echo "== smoke OK =="
