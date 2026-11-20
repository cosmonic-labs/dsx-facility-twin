#!/usr/bin/env bash
# Compile every component in components/ to wasm32-wasip2 inside the builder image.
# Output: dist/*.wasm. Cargo registry and target dir live in docker volumes (fast rebuilds).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/dist"
docker run --rm -v "$ROOT":/src -v twin-cargo:/usr/local/cargo/registry -v twin-target:/target \
  -e CARGO_TARGET_DIR=/target -w /src/components twin-builder:local \
  bash -c 'cargo build --release "$@" && cp /target/wasm32-wasip2/release/*.wasm /src/dist/' -- "$@"
ls -la "$ROOT/dist"
