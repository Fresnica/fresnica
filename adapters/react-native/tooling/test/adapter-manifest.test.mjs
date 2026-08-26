import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  AdapterCompatibilityError,
  createManifest,
  readReactNativeVersion,
  verifyManifest,
  writeManifest,
} from '../adapter-manifest.mjs';

async function project(version) {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'fresnica-rn-'));
  await writeFile(path.join(dir, 'package.json'), JSON.stringify({ dependencies: { 'react-native': version } }));
  return dir;
}

test('reads an exactly pinned React Native version', async () => {
  const dir = await project('0.74.2');
  assert.equal(await readReactNativeVersion(dir), '0.74.2');
});

test('rejects semver ranges because adapter binaries must be reproducible', async () => {
  const dir = await project('^0.74.2');
  await assert.rejects(
    readReactNativeVersion(dir),
    (error) => error instanceof AdapterCompatibilityError && /pinned to an exact version/.test(error.message),
  );
});

test('creates and verifies the compatibility manifest', async () => {
  const dir = await project('0.74.2');
  const artifact = path.join(dir, 'adapter.aar');
  await writeFile(artifact, 'adapter-bytes');
  const manifest = await createManifest({ projectDir: dir, artifacts: { android: artifact } });
  const manifestPath = path.join(dir, 'adapter-manifest.json');
  await writeFile(manifestPath, JSON.stringify(manifest));

  assert.equal(manifest.framework, 'react-native');
  assert.equal(manifest.frameworkVersion, '0.74.2');
  assert.equal(manifest.nativeBindingApiVersion, 2);
  assert.ok(manifest.androidHostDependencies.includes('com.facebook.react:react-android:0.74.2'));
  assert.deepEqual(manifest.appleHostLinkerFlags, ['-ObjC']);
  assert.equal(manifest.artifacts.android.fileName, 'adapter.aar');
  assert.match(manifest.artifacts.android.sha256, /^[0-9a-f]{64}$/);
  await verifyManifest({ projectDir: dir, manifestPath });
});



test('platform builds merge into one compatible manifest', async () => {
  const dir = await project('0.74.2');
  const android = path.join(dir, 'adapter.aar');
  const apple = path.join(dir, 'FresnicaRNAdapter.xcframework');
  const manifestPath = path.join(dir, 'adapter-manifest.json');
  await writeFile(android, 'android-adapter');
  await mkdir(apple, { recursive: true });
  await writeFile(path.join(apple, 'Info.plist'), 'apple-adapter');

  await writeManifest({ projectDir: dir, manifestPath, artifacts: { android } });
  const manifest = await writeManifest({ projectDir: dir, manifestPath, artifacts: { apple } });

  assert.equal(manifest.artifacts.android.fileName, 'adapter.aar');
  assert.equal(manifest.artifacts.apple.fileName, 'FresnicaRNAdapter.xcframework');
  await verifyManifest({ projectDir: dir, manifestPath });
});

test('creates and verifies a directory artifact such as an XCFramework', async () => {
  const dir = await project('0.74.2');
  const artifact = path.join(dir, 'FresnicaRNAdapter.xcframework');
  await mkdir(path.join(artifact, 'ios-arm64'), { recursive: true });
  await writeFile(path.join(artifact, 'Info.plist'), 'plist');
  await writeFile(path.join(artifact, 'ios-arm64', 'libFresnicaRNAdapter.a'), 'adapter-binary');
  const manifest = await createManifest({ projectDir: dir, artifacts: { apple: artifact } });
  const manifestPath = path.join(dir, 'adapter-manifest.json');
  await writeFile(manifestPath, JSON.stringify(manifest));

  assert.equal(manifest.artifacts.apple.fileName, 'FresnicaRNAdapter.xcframework');
  assert.match(manifest.artifacts.apple.sha256, /^[0-9a-f]{64}$/);
  await verifyManifest({ projectDir: dir, manifestPath });

  await writeFile(path.join(artifact, 'ios-arm64', 'libFresnicaRNAdapter.a'), 'changed-adapter');
  await assert.rejects(
    verifyManifest({ projectDir: dir, manifestPath }),
    (error) => error instanceof AdapterCompatibilityError && /SHA-256 mismatch/.test(error.message),
  );
});

test('reports an adapter rebuild when an adapter artifact changes', async () => {
  const dir = await project('0.74.2');
  const artifact = path.join(dir, 'adapter.aar');
  await writeFile(artifact, 'original-adapter');
  const manifest = await createManifest({ projectDir: dir, artifacts: { android: artifact } });
  const manifestPath = path.join(dir, 'adapter-manifest.json');
  await writeFile(manifestPath, JSON.stringify(manifest));
  await writeFile(artifact, 'changed-adapter');

  await assert.rejects(
    verifyManifest({ projectDir: dir, manifestPath }),
    (error) => error instanceof AdapterCompatibilityError && /SHA-256 mismatch/.test(error.message),
  );
});

test('reports an adapter rebuild when React Native changes', async () => {
  const dir = await project('0.74.2');
  const manifest = await createManifest({ projectDir: dir });
  const manifestPath = path.join(dir, 'adapter-manifest.json');
  await writeFile(manifestPath, JSON.stringify(manifest));
  await writeFile(path.join(dir, 'package.json'), JSON.stringify({ dependencies: { 'react-native': '0.75.0' } }));

  await assert.rejects(
    verifyManifest({ projectDir: dir, manifestPath }),
    (error) => error instanceof AdapterCompatibilityError && /adapter rebuild required/.test(error.message),
  );
});
