#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

forbidden='AccountIdentity|ProtectionRegistry|derive_verified_unlock_key|export_signing_material|generate_protected_mnemonic|protect_mnemonic_signing_material|protect_secret_signing_material|sign_protected_transaction_envelope'
if grep -RInE "$forbidden" clients/rust-cli/src clients/rust-client/src; then
  echo "Rust engineering clients must use fresnica-sdk for identity, wallet protection, Reveal/Export, and routine signing." >&2
  exit 1
fi

mapfile -t core_files < <(grep -RIl 'use fresnica_core' clients/rust-cli/src clients/rust-client/src | sort)
expected=(
  "clients/rust-client/src/transaction.rs"
  "clients/rust-client/src/wallet.rs"
)

if [[ "${core_files[*]:-}" != "${expected[*]}" ]]; then
  printf 'Unexpected direct fresnica_core imports in Rust engineering client layers:\n' >&2
  printf '  %s\n' "${core_files[@]:-<none>}" >&2
  exit 1
fi

grep -q '^fresnica-client = ' clients/rust-cli/Cargo.toml
grep -q '^fresnica-sdk = ' clients/rust-client/Cargo.toml

echo "Fresnica Rust client/CLI SDK boundary: OK"
