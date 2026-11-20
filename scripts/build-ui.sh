#!/usr/bin/env bash
# Build the React control panel into ui/dist (embedded into twin-api at component build time).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
docker run --rm -v "$ROOT/ui":/ui -v twin-npm:/root/.npm -w /ui twin-builder:local \
  bash -c 'npm install --no-audit --no-fund --loglevel=error && npx tsc --noEmit && npx vite build --logLevel warn'
ls -la "$ROOT/ui/dist/assets"
