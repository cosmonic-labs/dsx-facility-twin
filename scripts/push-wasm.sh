#!/usr/bin/env bash
# Push dist/*.wasm to the kind registry as OCI Wasm artifacts (application/wasm layer).
# Usage: push-wasm.sh <tag>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TAG="${1:?tag}"; REG="${REG:-localhost:5001}"
cd "$ROOT/dist"
printf '{"architecture":"wasm","os":"wasip2","created":"%s"}' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > config.json
for f in *.wasm; do
  name="${f%.wasm}"; name="${name//_/-}"
  docker run --rm --network host -v "$PWD":/w -w /w ghcr.io/oras-project/oras:v1.2.3 push --plain-http \
    "$REG/dsx-twin/$name:$TAG" --config config.json:application/vnd.wasm.config.v0+json "$f:application/wasm" >/dev/null
  echo "pushed $REG/dsx-twin/$name:$TAG"
done
