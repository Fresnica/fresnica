#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');

async function text(relativePath) {
  return readFile(path.join(ROOT, relativePath), 'utf8');
}

async function json(relativePath) {
  return JSON.parse(await text(relativePath));
}

function cargoPackageVersion(source, label) {
  const packageSection = source.match(/\[package\]([\s\S]*?)(?:\n\[|$)/)?.[1];
  const version = packageSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  assert.ok(version, `unable to read ${label} package version`);
  return version;
}

function rustApiVersion(source, constant, label) {
  const escaped = constant.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const value = source.match(new RegExp(`pub const ${escaped}: u64 = (\\d+);`))?.[1];
  assert.ok(value, `unable to read ${label} ${constant}`);
  return Number(value);
}

const manifest = await json('sdk/compatibility/manifest.json');
assert.equal(manifest.schemaVersion, 1, 'unsupported compatibility manifest schema');

const [
  coreCargo,
  coreSource,
  sdkCargo,
  sdkSource,
  nativeCargo,
  nativeSource,
  mobileCargo,
  mobileSource,
  wasmCargo,
  wasmSource,
  reactNativePackage,
  reactNativeContract,
  smartAccountProviderPackage,
] = await Promise.all([
  text('core/rust/Cargo.toml'),
  text('core/rust/src/client_api.rs'),
  text('sdk/rust/Cargo.toml'),
  text('sdk/rust/src/lib.rs'),
  text('bindings/native/Cargo.toml'),
  text('bindings/native/src/lib.rs'),
  text('bindings/mobile/Cargo.toml'),
  text('bindings/mobile/src/lib.rs'),
  text('bindings/wasm/Cargo.toml'),
  text('bindings/wasm/src/lib.rs'),
  json('adapters/react-native/package.json'),
  json('adapters/react-native/adapter-contract.json'),
  json('providers/smart-account-kit/package.json'),
]);

const smartAccountConfig = await import(
  pathToFileURL(path.join(ROOT, 'providers/smart-account-kit/src/config.mjs')).href
);
const smartAccountRecorder = await import(
  pathToFileURL(path.join(ROOT, 'providers/smart-account-kit/src/conformance-recorder.mjs')).href
);

const actual = {
  core: {
    packageVersion: cargoPackageVersion(coreCargo, 'core'),
    clientApiVersion: rustApiVersion(coreSource, 'CLIENT_API_VERSION', 'core'),
  },
  sdk: {
    packageVersion: cargoPackageVersion(sdkCargo, 'sdk'),
    apiVersion: rustApiVersion(sdkSource, 'SDK_API_VERSION', 'sdk'),
  },
  native: {
    packageVersion: cargoPackageVersion(nativeCargo, 'native'),
    bindingApiVersion: rustApiVersion(nativeSource, 'NATIVE_BINDING_API_VERSION', 'native'),
  },
  mobileCompatibility: {
    packageVersion: cargoPackageVersion(mobileCargo, 'mobile compatibility'),
    bindingApiVersion: rustApiVersion(mobileSource, 'MOBILE_BINDING_API_VERSION', 'mobile compatibility'),
  },
  wasm: {
    packageVersion: cargoPackageVersion(wasmCargo, 'wasm'),
    bindingApiVersion: rustApiVersion(wasmSource, 'WASM_BINDING_API_VERSION', 'wasm'),
  },
  adapters: {
    reactNative: {
      sourceVersion: reactNativePackage.version,
      requiresNativeBindingApiVersion: reactNativeContract.nativeBindingApiVersion,
    },
  },
  providers: {
    smartAccountKit: {
      packageVersion: smartAccountProviderPackage.version,
      upstreamVersion: smartAccountConfig.SMART_ACCOUNT_KIT_VERSION,
      webAuthnBrowserVersion: smartAccountProviderPackage.dependencies['@simplewebauthn/browser'],
      fixtureSchema: smartAccountRecorder.SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA,
      network: smartAccountConfig.STELLAR_TESTNET_SMART_ACCOUNT.network,
      deploymentDate: smartAccountConfig.STELLAR_TESTNET_SMART_ACCOUNT.deploymentDate,
    },
  },
};

for (const layer of ['core', 'sdk', 'native', 'mobileCompatibility', 'wasm']) {
  for (const [field, value] of Object.entries(actual[layer])) {
    assert.equal(
      manifest[layer][field],
      value,
      `${layer}.${field} drifted: manifest=${manifest[layer][field]} source=${value}`,
    );
  }
}

assert.deepEqual(manifest.adapters.reactNative, actual.adapters.reactNative, 'React Native adapter version contract drifted');
assert.deepEqual(
  manifest.providers.smartAccountKit,
  actual.providers.smartAccountKit,
  'smart-account provider compatibility contract drifted',
);

assert.equal(
  smartAccountProviderPackage.dependencies['smart-account-kit'],
  manifest.providers.smartAccountKit.upstreamVersion,
  'smart-account provider package dependency and pinned upstream version drifted',
);

assert.equal(
  manifest.sdk.requiresCoreClientApiVersion,
  manifest.core.clientApiVersion,
  'SDK/Core API compatibility drifted',
);
assert.equal(
  manifest.native.requiresSdkApiVersion,
  manifest.sdk.apiVersion,
  'Native SDK/SDK API compatibility drifted',
);
assert.equal(
  manifest.mobileCompatibility.requiresSdkApiVersion,
  manifest.sdk.apiVersion,
  'Mobile compatibility/SDK API compatibility drifted',
);
assert.equal(
  manifest.wasm.requiresSdkApiVersion,
  manifest.sdk.apiVersion,
  'WASM/SDK API compatibility drifted',
);
assert.equal(
  manifest.adapters.reactNative.requiresNativeBindingApiVersion,
  manifest.native.bindingApiVersion,
  'React Native/Native binding compatibility drifted',
);
assert.equal(
  reactNativeContract.adapterSourceVersion,
  manifest.adapters.reactNative.sourceVersion,
  'React Native package and adapter contract source versions drifted',
);
assert.equal(
  reactNativeContract.fresnicaNativeSdkVersion,
  manifest.native.packageVersion,
  'React Native adapter contract and Native SDK package versions drifted',
);

console.log('Fresnica SDK compatibility manifest: OK');
console.log(`  Core client API: ${manifest.core.clientApiVersion}`);
console.log(`  SDK API: ${manifest.sdk.apiVersion}`);
console.log(`  Native binding API: ${manifest.native.bindingApiVersion}`);
console.log(`  Mobile compatibility API: ${manifest.mobileCompatibility.bindingApiVersion}`);
console.log(`  WASM binding API: ${manifest.wasm.bindingApiVersion}`);
console.log(`  React Native adapter source: ${manifest.adapters.reactNative.sourceVersion}`);
console.log(
  `  Smart Account Kit provider: ${manifest.providers.smartAccountKit.packageVersion} ` +
  `(upstream ${manifest.providers.smartAccountKit.upstreamVersion}, ${manifest.providers.smartAccountKit.network})`,
);
