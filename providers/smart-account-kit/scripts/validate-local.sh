#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROVIDER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

command -v node >/dev/null 2>&1 || { echo "node is required" >&2; exit 1; }
command -v npm >/dev/null 2>&1 || { echo "npm is required" >&2; exit 1; }

test -d "$PROVIDER_DIR/node_modules/smart-account-kit" || {
  echo "run npm install in $PROVIDER_DIR first" >&2
  exit 1
}

npm test --prefix "$PROVIDER_DIR"
node --check "$PROVIDER_DIR/src/conformance-recorder.mjs"
node --check "$PROVIDER_DIR/src/conformance.mjs"
node --check "$PROVIDER_DIR/scripts/verify-fixture.mjs"
npm run --prefix "$PROVIDER_DIR" check:installed
npm run --prefix "$PROVIDER_DIR" testnet:build

printf 'Fresnica smart-account provider local validation: OK\n'
