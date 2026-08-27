#!/usr/bin/env node

import { copyFile, mkdir, readdir, stat } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
  AdapterCompatibilityError,
  readReactNativeVersion,
  writeManifest,
} from './adapter-manifest.mjs';

const TOOL_DIR = path.dirname(fileURLToPath(import.meta.url));
const ADAPTER_DIR = path.resolve(TOOL_DIR, '..');
const ANDROID_DIR = path.join(ADAPTER_DIR, 'android');
const ANDROID_INIT_SCRIPT = path.join(ANDROID_DIR, 'include.init.gradle');
const APPLE_BUILD_SCRIPT = path.join(ADAPTER_DIR, 'apple', 'build.sh');

export function androidGradleInvocation({ projectDir, nativeSdkAar, reactNativeVersion, compileSdk }) {
  const wrapper = process.platform === 'win32'
    ? path.join(projectDir, 'android', 'gradlew.bat')
    : path.join(projectDir, 'android', 'gradlew');
  const command = process.env.FRESNICA_ADAPTER_GRADLE || wrapper;
  const args = [
    '-p',
    path.join(projectDir, 'android'),
    '--init-script',
    ANDROID_INIT_SCRIPT,
    `-PfresnicaAdapterProjectDir=${ANDROID_DIR}`,
    `-PfresnicaNativeSdkAar=${path.resolve(nativeSdkAar)}`,
    `-PfresnicaReactNativeVersion=${reactNativeVersion}`,
  ];
  if (compileSdk) {
    args.push(`-PfresnicaCompileSdk=${compileSdk}`);
  }
  args.push(':fresnicaReactNativeAdapter:clean', ':fresnicaReactNativeAdapter:assembleRelease');
  return { command, args };
}

async function ensurePath(filePath, label, kind) {
  let info;
  try {
    info = await stat(filePath);
  } catch {
    throw new AdapterCompatibilityError(`${label} not found: ${filePath}`);
  }
  const valid = kind === 'directory' ? info.isDirectory() : info.isFile();
  if (!valid) {
    throw new AdapterCompatibilityError(`${label} is not a ${kind}: ${filePath}`);
  }
}

async function ensureFile(filePath, label) {
  await ensurePath(filePath, label, 'file');
}

async function ensureDirectory(filePath, label) {
  await ensurePath(filePath, label, 'directory');
}

async function run(command, args, cwd) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' });
    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (signal) {
        reject(new AdapterCompatibilityError(`adapter build terminated by signal ${signal}`));
      } else if (code !== 0) {
        reject(new AdapterCompatibilityError(`adapter build failed with exit code ${code}`));
      } else {
        resolve();
      }
    });
  });
}

async function findReleaseAar() {
  const dir = path.join(ANDROID_DIR, 'build', 'outputs', 'aar');
  const names = await readdir(dir);
  const release = names.filter((name) => name.endsWith('-release.aar')).sort();
  if (release.length !== 1) {
    throw new AdapterCompatibilityError(`expected one Android release AAR in ${dir}, found ${release.length}`);
  }
  return path.join(dir, release[0]);
}

export async function buildAndroidAdapter({ projectDir, nativeSdkAar, outDir, compileSdk }) {
  const absoluteProject = path.resolve(projectDir);
  const absoluteNativeAar = path.resolve(nativeSdkAar);
  const absoluteOut = path.resolve(outDir);
  await ensureFile(path.join(absoluteProject, 'package.json'), 'consumer package.json');
  await ensureFile(absoluteNativeAar, 'Fresnica Native SDK AAR');

  const reactNativeVersion = await readReactNativeVersion(absoluteProject);
  const invocation = androidGradleInvocation({
    projectDir: absoluteProject,
    nativeSdkAar: absoluteNativeAar,
    reactNativeVersion,
    compileSdk,
  });
  await ensureFile(invocation.command, 'consumer Gradle wrapper');

  await run(invocation.command, invocation.args, absoluteProject);

  const builtAar = await findReleaseAar();
  await mkdir(absoluteOut, { recursive: true });
  const outputAar = path.join(absoluteOut, 'fresnica-rn-adapter.aar');
  const manifestPath = path.join(absoluteOut, 'adapter-manifest.json');
  await copyFile(builtAar, outputAar);

  await writeManifest({
    projectDir: absoluteProject,
    manifestPath,
    artifacts: { android: outputAar },
  });

  return { outputAar, manifestPath };
}


export async function buildAppleAdapter({ projectDir, nativeSdkXcframework, nativeFfiXcframework, outDir }) {
  const absoluteProject = path.resolve(projectDir);
  const absoluteNativeSdk = path.resolve(nativeSdkXcframework);
  const absoluteNativeFfi = path.resolve(nativeFfiXcframework);
  const absoluteOut = path.resolve(outDir);
  await ensureFile(path.join(absoluteProject, 'package.json'), 'consumer package.json');
  await ensureDirectory(absoluteNativeSdk, 'Fresnica Native SDK XCFramework');
  await ensureDirectory(absoluteNativeFfi, 'Fresnica Native SDK FFI XCFramework');
  await readReactNativeVersion(absoluteProject);

  const buildScript = process.env.FRESNICA_ADAPTER_APPLE_BUILD_SCRIPT || APPLE_BUILD_SCRIPT;
  await ensureFile(buildScript, 'Apple adapter build script');
  await mkdir(absoluteOut, { recursive: true });

  const outputXcframework = path.join(absoluteOut, 'FresnicaRNAdapter.xcframework');
  const manifestPath = path.join(absoluteOut, 'adapter-manifest.json');
  await run(buildScript, [absoluteProject, absoluteNativeSdk, absoluteNativeFfi, outputXcframework], absoluteProject);
  await ensureDirectory(outputXcframework, 'Apple React Native adapter XCFramework');

  await writeManifest({
    projectDir: absoluteProject,
    manifestPath,
    artifacts: { apple: outputXcframework },
  });

  return { outputXcframework, manifestPath };
}

function parseArgs(argv) {
  const [command, framework, ...rest] = argv;
  const args = { command, framework };
  for (let index = 0; index < rest.length; index += 1) {
    const token = rest[index];
    if (!token.startsWith('--')) {
      throw new AdapterCompatibilityError(`unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const value = rest[index + 1];
    if (value == null || value.startsWith('--')) {
      throw new AdapterCompatibilityError(`missing value for --${key}`);
    }
    args[key] = value;
    index += 1;
  }
  return args;
}

async function main(argv) {
  const args = parseArgs(argv);
  if (args.command !== 'build' || args.framework !== 'react-native') {
    throw new AdapterCompatibilityError('usage: fresnica-adapter.mjs build react-native --platform android|apple --project PATH --out DIR [platform Native SDK arguments]');
  }

  if (args.platform === 'android') {
    if (!args.project || !args['native-android-aar'] || !args.out) {
      throw new AdapterCompatibilityError('build react-native/android requires --project, --native-android-aar, and --out');
    }
    const result = await buildAndroidAdapter({
      projectDir: args.project,
      nativeSdkAar: args['native-android-aar'],
      outDir: args.out,
      compileSdk: args['android-compile-sdk'],
    });
    process.stdout.write(`Android React Native adapter: ${result.outputAar}\n`);
    process.stdout.write(`Compatibility manifest: ${result.manifestPath}\n`);
    return;
  }

  if (args.platform === 'apple') {
    if (!args.project || !args['native-apple-sdk-xcframework'] || !args['native-apple-ffi-xcframework'] || !args.out) {
      throw new AdapterCompatibilityError('build react-native/apple requires --project, --native-apple-sdk-xcframework, --native-apple-ffi-xcframework, and --out');
    }
    const result = await buildAppleAdapter({
      projectDir: args.project,
      nativeSdkXcframework: args['native-apple-sdk-xcframework'],
      nativeFfiXcframework: args['native-apple-ffi-xcframework'],
      outDir: args.out,
    });
    process.stdout.write(`Apple React Native adapter: ${result.outputXcframework}\n`);
    process.stdout.write(`Compatibility manifest: ${result.manifestPath}\n`);
    return;
  }

  throw new AdapterCompatibilityError('build react-native requires --platform android or --platform apple');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    const prefix = error instanceof AdapterCompatibilityError ? '' : `${error.name}: `;
    process.stderr.write(`${prefix}${error.message}\n`);
    process.exitCode = 2;
  });
}
