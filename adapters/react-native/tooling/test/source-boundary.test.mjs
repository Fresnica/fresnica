import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const TEST_DIR = path.dirname(fileURLToPath(import.meta.url));
const ADAPTER_DIR = path.resolve(TEST_DIR, '../..');

const REQUIRED_METHODS = [
  'parseAccount',
  'protectSecret',
  'protectMnemonic',
  'generateMnemonic',
  'reprotect',
  'reveal',
  'prepareEd25519Signing',
  'applyEd25519Signature',
  'canEnrollSystemAuth',
  'hasSystemAuth',
  'removeSystemAuth',
  'enrollSystemAuth',
  'signWithSystemAuth',
  'signWithPasscode',
];

const FORBIDDEN_FRAMEWORK_METHODS = [
  'deriveUnlockKey',
  'validateUnlockKey',
  'signTransactionXdr',
];

test('Android adapter targets Native SDK and preserves the reviewed JS surface', async () => {
  const source = await readFile(
    path.join(ADAPTER_DIR, 'android/src/main/kotlin/com/fresnica/sdk/reactnative/FresnicaCoreModule.kt'),
    'utf8',
  );
  assert.match(source, /import com\.fresnica\.sdk\.FresnicaSdkApi/);
  assert.doesNotMatch(source, /MobileCoreApi|com\.fresnica\.core/);
  assert.match(source, /const val NAME = "FresnicaCore"/);
  for (const method of REQUIRED_METHODS) {
    assert.match(source, new RegExp(`fun ${method}\\(`), `missing Android framework method ${method}`);
  }
  for (const method of FORBIDDEN_FRAMEWORK_METHODS) {
    assert.doesNotMatch(source, new RegExp(`fun ${method}\\(`), `forbidden Android framework method ${method}`);
  }
});

test('Apple adapter targets Native SDK and preserves the reviewed bridge surface', async () => {
  const [source, shim] = await Promise.all([
    readFile(path.join(ADAPTER_DIR, 'apple/FresnicaCoreModule.swift'), 'utf8'),
    readFile(path.join(ADAPTER_DIR, 'apple/FresnicaCoreModule.m'), 'utf8'),
  ]);
  assert.match(source, /import FresnicaSDK/);
  assert.match(source, /private let core: FresnicaSdkApiProtocol/);
  assert.doesNotMatch(source, /MobileCoreApi|MobileCoreError/);
  assert.doesNotMatch(source, /class FresnicaSignerAuthorization|class FresnicaWalletUnlockKeyStore/);
  assert.match(shim, /RCT_EXTERN_MODULE\(FresnicaCoreModule/);
  for (const method of REQUIRED_METHODS) {
    assert.match(source, new RegExp(`@objc\\(${method}`), `missing Apple framework method ${method}`);
    assert.match(shim, new RegExp(`RCT_EXTERN_METHOD\\(${method}`), `missing Apple shim method ${method}`);
  }
  for (const method of FORBIDDEN_FRAMEWORK_METHODS) {
    assert.doesNotMatch(source, new RegExp(`@objc\\(${method}`), `forbidden Apple framework method ${method}`);
    assert.doesNotMatch(shim, new RegExp(`RCT_EXTERN_METHOD\\(${method}`), `forbidden Apple shim method ${method}`);
  }
});


test('Apple binary build compiles framework glue without absorbing Native SDK source', async () => {
  const build = await readFile(path.join(ADAPTER_DIR, 'apple/build.sh'), 'utf8');
  assert.match(build, /FresnicaCoreModule\.swift/);
  assert.match(build, /FresnicaCoreModule\.m/);
  assert.match(build, /FresnicaSDK\.xcframework/);
  assert.doesNotMatch(build, /FresnicaSignerAuthorization\.swift|FresnicaWalletUnlockKeyStore\.swift/);
  assert.match(build, /libFresnicaRNAdapter\.a/);
  assert.match(build, /-create-xcframework/);
});
