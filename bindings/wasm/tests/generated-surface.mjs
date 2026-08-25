import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

const outDir = resolve(process.argv[2] ?? new URL('../build/web', import.meta.url).pathname);
const declarationsPath = resolve(outDir, 'fresnica_sdk.d.ts');
const javascriptPath = resolve(outDir, 'fresnica_sdk.js');
const wasmPath = resolve(outDir, 'fresnica_sdk_bg.wasm');
const packagePath = resolve(outDir, 'package.json');

const declarations = readFileSync(declarationsPath, 'utf8');
const pkg = JSON.parse(readFileSync(packagePath, 'utf8'));

assert.ok(statSync(javascriptPath).size > 0, 'generated JavaScript must not be empty');
assert.ok(statSync(wasmPath).size > 0, 'generated WebAssembly must not be empty');
assert.match(declarations, /export class FresnicaWasmSdk\b/);

for (const exported of [
  'version',
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
  assert.match(
    declarations,
    new RegExp(`\\b${exported}\\s*\\(`),
    `${exported} must be present in the generated TypeScript surface`,
  );
}

for (const forbidden of [
  'deriveUnlockKey',
  'validateUnlockKey',
  'signTransactionXdr',
]) {
  const pattern = forbidden === 'signTransactionXdr'
    ? /\bsignTransactionXdr\s*\(/
    : new RegExp(`\\b${forbidden}\\s*\\(`);
  assert.equal(
    pattern.test(declarations),
    false,
    `${forbidden} must not be present in the generated Web surface`,
  );
}

assert.equal(pkg.name, '@fresnica/wasm-sdk');
assert.equal(pkg.type, 'module');
assert.equal(pkg.module, './fresnica_sdk.js');
assert.equal(pkg.types, './fresnica_sdk.d.ts');
assert.deepEqual(pkg.files, [
  'fresnica_sdk.js',
  'fresnica_sdk_bg.wasm',
  'fresnica_sdk.d.ts',
]);

console.log('Fresnica generated WASM surface: OK');
