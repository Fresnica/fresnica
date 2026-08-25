import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
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
  assert.match(build, /node_modules\/react-native/, 'React Native build must support source headers when CocoaPods does not expose a React header tree');
  assert.match(build, /REACT_NATIVE_ROOT/, 'React Native build must locate the installed source root when CocoaPods headers are absent');
  assert.match(build, /pod-header-shim\.rb/, 'React Native source fallback must follow evaluated CocoaPods podspec metadata');
  assert.match(build, /react-source-headers/, 'React Native source fallback must create isolated virtual header roots');
  assert.doesNotMatch(build, /RCTDeprecation_HEADER|link_flat_header_namespace/, 'React Native source fallback must not hardcode transitive header locations');
});


test('Apple podspec header shim follows CocoaPods namespaces instead of React Native source paths', async (t) => {
  const ruby = spawnSync('ruby', ['--version'], { encoding: 'utf8' });
  if (ruby.status !== 0) {
    t.skip('ruby is unavailable');
    return;
  }

  const temp = await mkdtemp(path.join(os.tmpdir(), 'fresnica-rn-podspec-'));
  const reactNative = path.join(temp, 'node_modules/react-native');
  const pods = path.join(temp, 'ios/Pods');
  const localSpecs = path.join(pods, 'Local Podspecs');
  const out = path.join(temp, 'headers');

  await mkdir(path.join(reactNative, 'React/Base'), { recursive: true });
  await mkdir(path.join(reactNative, 'ReactApple/Libraries/RCTFoundation/RCTDeprecation/Exported'), { recursive: true });
  await mkdir(path.join(reactNative, 'React/Fabric'), { recursive: true });
  await mkdir(localSpecs, { recursive: true });

  const bridge = path.join(reactNative, 'React/Base/RCTBridgeModule.h');
  const deprecation = path.join(reactNative, 'ReactApple/Libraries/RCTFoundation/RCTDeprecation/Exported/RCTDeprecation.h');
  const fabric = path.join(reactNative, 'React/Fabric/RCTSurfaceTouchHandler.h');
  await writeFile(bridge, '#import <RCTDeprecation/RCTDeprecation.h>\n');
  await writeFile(deprecation, '#pragma once\n');
  await writeFile(fabric, '#pragma once\n');
  await writeFile(path.join(reactNative, 'React-Core.podspec'), '# source root marker\n');
  await writeFile(path.join(reactNative, 'React-RCTFabric.podspec'), '# source root marker\n');
  await writeFile(path.join(reactNative, 'ReactApple/Libraries/RCTFoundation/RCTDeprecation/RCTDeprecation.podspec'), '# source root marker\n');

  await writeFile(path.join(localSpecs, 'React-Core.podspec.json'), JSON.stringify({
    name: 'React-Core',
    header_dir: 'React',
    source_files: ['React/Base/*.{h,m}'],
  }));
  await writeFile(path.join(localSpecs, 'RCTDeprecation.podspec.json'), JSON.stringify({
    name: 'RCTDeprecation',
    source_files: ['Exported/*.h', 'RCTDeprecation.m'],
  }));
  await writeFile(path.join(localSpecs, 'React-RCTFabric.podspec.json'), JSON.stringify({
    name: 'React-RCTFabric',
    header_dir: 'React',
    header_mappings_dir: 'React/Fabric',
    source_files: ['React/Fabric/*.h'],
  }));

  const helper = path.join(ADAPTER_DIR, 'apple/pod-header-shim.rb');
  const result = spawnSync('ruby', [helper, reactNative, pods, out], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);

  assert.equal(await realpath(path.join(out, 'React-Core/React/RCTBridgeModule.h')), await realpath(bridge));
  assert.equal(await realpath(path.join(out, 'RCTDeprecation/RCTDeprecation/RCTDeprecation.h')), await realpath(deprecation));
  assert.equal(await realpath(path.join(out, 'React-RCTFabric/React/RCTSurfaceTouchHandler.h')), await realpath(fabric));
  const roots = result.stdout.trim().split('\n');
  assert.match(roots[0], /React-Core$/, 'bridge-owning include root must be searched first');
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
