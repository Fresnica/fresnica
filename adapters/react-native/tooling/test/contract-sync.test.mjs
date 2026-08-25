import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const TEST_DIR = path.dirname(fileURLToPath(import.meta.url));
const ADAPTER_DIR = path.resolve(TEST_DIR, '../..');
const REPO_ROOT = path.resolve(ADAPTER_DIR, '../..');

test('adapter contract stays pinned to the Native SDK binding/version', async () => {
  const [contract, adapterPackage, nativeCargo, nativeLib] = await Promise.all([
    readFile(path.join(ADAPTER_DIR, 'adapter-contract.json'), 'utf8').then(JSON.parse),
    readFile(path.join(ADAPTER_DIR, 'package.json'), 'utf8').then(JSON.parse),
    readFile(path.join(REPO_ROOT, 'bindings/native/Cargo.toml'), 'utf8'),
    readFile(path.join(REPO_ROOT, 'bindings/native/src/lib.rs'), 'utf8'),
  ]);

  const nativeVersion = nativeCargo.match(/^version = "([^"]+)"/m)?.[1];
  const bindingVersion = nativeLib.match(/pub const NATIVE_BINDING_API_VERSION: u64 = (\d+);/)?.[1];

  assert.equal(contract.fresnicaNativeSdkVersion, nativeVersion);
  assert.equal(String(contract.nativeBindingApiVersion), bindingVersion);
  assert.equal(contract.adapterSourceVersion, adapterPackage.version);
});
