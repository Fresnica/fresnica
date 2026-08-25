import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source = readFileSync(new URL('../src/lib.rs', import.meta.url), 'utf8');
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
const buildScript = readFileSync(new URL('../scripts/build-web.sh', import.meta.url), 'utf8');

for (const exported of [
  'parseAccount',
  'protectSecret',
  'protectMnemonic',
  'generateMnemonic',
  'reprotect',
  'signTransactionXdrWithPasscode',
  'reveal',
  'prepareEd25519Signing',
  'applyEd25519Signature',
]) {
  assert.match(source, new RegExp(`js_name = ${exported}\\b`));
}

for (const forbidden of [
  'js_name = deriveUnlockKey',
  'js_name = validateUnlockKey',
  'js_name = signTransactionXdr]',
]) {
  assert.equal(source.includes(forbidden), false, `${forbidden} must not be exported to Web`);
}

assert.match(
  source,
  /\.sign_transaction_xdr_with_passcode\(/,
  'routine Web signing must delegate to the SDK composite passcode operation',
);
assert.equal(
  source.includes('.derive_unlock_key('),
  false,
  'WASM glue must never materialize a raw unlock key',
);
assert.match(cargo, /getrandom = \{ version = "=0\.4\.3", features = \["wasm_js"\] \}/);
assert.match(cargo, /wasm-bindgen = "=0\.2\.127"/);
assert.match(buildScript, /WASM_BINDGEN_VERSION="0\.2\.127"/);

console.log('Fresnica WASM security boundary: OK');
