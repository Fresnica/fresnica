#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

app_sources=(
  clients/rust-cli/src
  clients/rust-tui/src
  reference/rust-client/src
)

forbidden='AccountIdentity|ProtectionRegistry|derive_verified_unlock_key|export_signing_material|generate_protected_mnemonic|protect_mnemonic_signing_material|protect_secret_signing_material|sign_protected_transaction_envelope'
if grep -RInE "$forbidden" "${app_sources[@]}"; then
  echo "Rust application/reference layers must use fresnica-sdk for identity, wallet protection, Reveal/Export, and routine signing." >&2
  exit 1
fi

mapfile -t core_files < <(grep -RIl 'use fresnica_core' "${app_sources[@]}" | sort)
expected=(
  "reference/rust-client/src/ledger_authorization.rs"
  "reference/rust-client/src/transaction.rs"
  "reference/rust-client/src/wallet.rs"
)

if [[ "${core_files[*]:-}" != "${expected[*]}" ]]; then
  printf 'Unexpected direct fresnica_core imports in Rust application/reference layers:\n' >&2
  printf '  %s\n' "${core_files[@]:-<none>}" >&2
  exit 1
fi

grep -q '^fresnica-client = { path = "../../reference/rust-client" }$' clients/rust-cli/Cargo.toml
grep -q '^fresnica-client = { path = "../../reference/rust-client" }$' clients/rust-tui/Cargo.toml
grep -q '^fresnica-sdk = ' reference/rust-client/Cargo.toml

if grep -q '^fresnica-core = ' clients/rust-cli/Cargo.toml clients/rust-tui/Cargo.toml; then
  echo "Rust CLI/TUI must not depend directly on fresnica-core." >&2
  exit 1
fi

echo "Fresnica Rust application/reference SDK boundary: OK"
