import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';

import { androidGradleInvocation, buildAndroidAdapter, buildAppleAdapter } from '../fresnica-adapter.mjs';

test('Android build joins the consumer Gradle build instead of owning a toolchain', () => {
  const projectDir = path.resolve('/tmp/fresnica-consumer');
  const nativeSdkAar = path.resolve('/tmp/fresnica-native-sdk.aar');
  const invocation = androidGradleInvocation({
    projectDir,
    nativeSdkAar,
    reactNativeVersion: '0.74.2',
    compileSdk: '35',
  });

  assert.equal(invocation.command, path.join(projectDir, 'android', process.platform === 'win32' ? 'gradlew.bat' : 'gradlew'));
  assert.deepEqual(invocation.args.slice(-2), [
    ':fresnicaReactNativeAdapter:clean',
    ':fresnicaReactNativeAdapter:assembleRelease',
  ]);
  assert.deepEqual(invocation.args.slice(0, 2), ['-p', path.join(projectDir, 'android')]);
  assert.ok(invocation.args.includes('--init-script'));
  assert.ok(invocation.args.some((arg) => arg.startsWith('-PfresnicaAdapterProjectDir=')));
  assert.ok(invocation.args.includes(`-PfresnicaNativeSdkAar=${nativeSdkAar}`));
  assert.ok(invocation.args.includes('-PfresnicaReactNativeVersion=0.74.2'));
  assert.ok(invocation.args.includes('-PfresnicaCompileSdk=35'));
});


test('Android one-time build emits adapter AAR plus compatibility manifest', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'fresnica-rn-build-'));
  const projectDir = path.join(root, 'mobile');
  const outDir = path.join(root, 'vendor');
  const nativeSdkAar = path.join(root, 'fresnica-native-sdk.aar');
  const fakeGradle = path.join(root, 'gradlew');
  await mkdir(path.join(projectDir, 'android'), { recursive: true });
  await writeFile(path.join(projectDir, 'package.json'), JSON.stringify({ dependencies: { 'react-native': '0.74.2' } }));
  await writeFile(nativeSdkAar, 'native-sdk');
  await writeFile(
    fakeGradle,
    '#!/bin/sh\nset -eu\nadapter=\"\"\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    -PfresnicaAdapterProjectDir=*) adapter=${arg#*=} ;;\n  esac\ndone\n[ -n \"$adapter\" ]\nmkdir -p \"$adapter/build/outputs/aar\"\nprintf adapter-binary > \"$adapter/build/outputs/aar/fresnica-react-native-adapter-release.aar\"\n',
  );
  await chmod(fakeGradle, 0o755);

  const previous = process.env.FRESNICA_ADAPTER_GRADLE;
  process.env.FRESNICA_ADAPTER_GRADLE = fakeGradle;
  try {
    const result = await buildAndroidAdapter({ projectDir, nativeSdkAar, outDir });
    assert.equal(await readFile(result.outputAar, 'utf8'), 'adapter-binary');
    const manifest = JSON.parse(await readFile(result.manifestPath, 'utf8'));
    assert.equal(manifest.frameworkVersion, '0.74.2');
    assert.equal(manifest.artifacts.android.fileName, 'fresnica-rn-adapter.aar');
  } finally {
    if (previous === undefined) delete process.env.FRESNICA_ADAPTER_GRADLE;
    else process.env.FRESNICA_ADAPTER_GRADLE = previous;
    await rm(path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../android/build'), { recursive: true, force: true });
    await rm(root, { recursive: true, force: true });
  }
});


test('Apple one-time build emits a static adapter XCFramework plus compatibility manifest', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'fresnica-rn-apple-build-'));
  const projectDir = path.join(root, 'mobile');
  const outDir = path.join(root, 'vendor');
  const nativeSdk = path.join(root, 'FresnicaSDK.xcframework');
  const nativeFfi = path.join(root, 'FresnicaSDKFFI.xcframework');
  const fakeBuilder = path.join(root, 'build-apple.sh');
  await mkdir(projectDir, { recursive: true });
  await mkdir(nativeSdk, { recursive: true });
  await mkdir(nativeFfi, { recursive: true });
  await writeFile(path.join(projectDir, 'package.json'), JSON.stringify({ dependencies: { 'react-native': '0.74.2' } }));
  await writeFile(
    fakeBuilder,
    '#!/bin/sh\nset -eu\nout="$4"\nmkdir -p "$out/ios-arm64"\nprintf plist > "$out/Info.plist"\nprintf adapter-binary > "$out/ios-arm64/libFresnicaRNAdapter.a"\n',
  );
  await chmod(fakeBuilder, 0o755);

  const previous = process.env.FRESNICA_ADAPTER_APPLE_BUILD_SCRIPT;
  process.env.FRESNICA_ADAPTER_APPLE_BUILD_SCRIPT = fakeBuilder;
  try {
    const result = await buildAppleAdapter({
      projectDir,
      nativeSdkXcframework: nativeSdk,
      nativeFfiXcframework: nativeFfi,
      outDir,
    });
    assert.equal(await readFile(path.join(result.outputXcframework, 'ios-arm64/libFresnicaRNAdapter.a'), 'utf8'), 'adapter-binary');
    const manifest = JSON.parse(await readFile(result.manifestPath, 'utf8'));
    assert.equal(manifest.frameworkVersion, '0.74.2');
    assert.equal(manifest.artifacts.apple.fileName, 'FresnicaRNAdapter.xcframework');
  } finally {
    if (previous === undefined) delete process.env.FRESNICA_ADAPTER_APPLE_BUILD_SCRIPT;
    else process.env.FRESNICA_ADAPTER_APPLE_BUILD_SCRIPT = previous;
    await rm(root, { recursive: true, force: true });
  }
});
