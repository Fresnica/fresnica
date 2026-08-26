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
  'deriveMnemonicSigner',
  'reprotect',
  'reveal',
  'prepareEd25519Signing',
  'applyEd25519Signature',
  'canUseSystemAuth',
  'hasSystemAuthDomain',
  'initializeSystemAuth',
  'registerSignerSystemAuth',
  'hasSignerSystemAuth',
  'removeSignerSystemAuth',
  'removeSystemAuthDomain',
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
  assert.equal((source.match(/@unknown default:/g) ?? []).length, 2, 'Swift 6 error mapping must tolerate future Native SDK enum cases');
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
  assert.match(build, /! -path '\*macos\*'/, 'iOS device lookup must exclude the macOS Native SDK slice');
  assert.match(build, /Headers\/Private/, 'React Native build must include private CocoaPods header roots when present');
  assert.match(build, /React\/RCTBridgeModule\.h/, 'React Native build must discover the actual RCTBridgeModule header');
  assert.match(build, /REACT_BRIDGE_ROOT/, 'React Native build must derive the include root from a CocoaPods bridge header');
  assert.match(build, /React\.xcframework/, 'modern prebuilt React Native must use the CocoaPods-installed React framework');
  assert.match(build, /REACT_DEVICE_FRAMEWORK/, 'Apple device build must select the installed React framework slice');
  assert.match(build, /REACT_SIMULATOR_FRAMEWORK/, 'Apple simulator build must select the installed React framework slice');
  assert.match(build, /-F/, 'prebuilt React framework must be supplied as a framework search path');
  assert.doesNotMatch(build, /node_modules\/react-native|pod-header-shim\.rb|react-source-headers/, 'Apple adapter must not reconstruct CocoaPods header namespaces from React Native source');
});


test('Apple real-consumer validator reuses Native SDK and checks adapter slices', async () => {
  const validate = await readFile(path.join(ADAPTER_DIR, 'apple/validate-consumer.sh'), 'utf8');
  assert.match(validate, /validate-apple-local\.sh/);
  assert.match(validate, /fresnica-adapter\.mjs/);
  assert.match(validate, /adapter-manifest\.mjs/);
  assert.match(validate, /ios\/Pods/);
  assert.doesNotMatch(validate, /Pods\/Headers\/Public/, 'consumer validator must not assume React headers are public');
  assert.match(validate, /x86_64/);
  assert.match(validate, /arm64/);
});
