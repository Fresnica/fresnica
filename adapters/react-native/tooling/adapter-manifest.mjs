#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { lstat, readFile, readdir, readlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const TOOL_DIR = path.dirname(fileURLToPath(import.meta.url));
const ADAPTER_DIR = path.resolve(TOOL_DIR, '..');
const CONTRACT_PATH = path.join(ADAPTER_DIR, 'adapter-contract.json');

export class AdapterCompatibilityError extends Error {}

export async function loadContract(contractPath = CONTRACT_PATH) {
  return JSON.parse(await readFile(contractPath, 'utf8'));
}

export async function readReactNativeVersion(projectDir) {
  const packagePath = path.join(projectDir, 'package.json');
  const pkg = JSON.parse(await readFile(packagePath, 'utf8'));
  const declared = pkg.dependencies?.['react-native'] ?? pkg.devDependencies?.['react-native'];
  if (typeof declared !== 'string' || declared.trim() === '') {
    throw new AdapterCompatibilityError('react-native is not declared in dependencies or devDependencies');
  }
  const version = declared.trim();
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new AdapterCompatibilityError(
      `react-native must be pinned to an exact version before building the adapter; found ${JSON.stringify(version)}`,
    );
  }
  return version;
}

async function updateArtifactHash(hash, artifactPath, relativePath = '') {
  const info = await lstat(artifactPath);
  const normalized = relativePath.split(path.sep).join('/');

  if (info.isFile()) {
    hash.update(`file\0${normalized}\0`);
    hash.update(await readFile(artifactPath));
    return;
  }

  if (info.isSymbolicLink()) {
    hash.update(`symlink\0${normalized}\0${await readlink(artifactPath)}\0`);
    return;
  }

  if (info.isDirectory()) {
    hash.update(`directory\0${normalized}\0`);
    const entries = (await readdir(artifactPath)).sort();
    for (const entry of entries) {
      const childRelative = relativePath ? path.join(relativePath, entry) : entry;
      await updateArtifactHash(hash, path.join(artifactPath, entry), childRelative);
    }
    return;
  }

  throw new AdapterCompatibilityError(`unsupported artifact entry: ${artifactPath}`);
}

async function sha256(artifactPath) {
  const info = await lstat(artifactPath);
  if (info.isFile()) {
    return createHash('sha256').update(await readFile(artifactPath)).digest('hex');
  }
  if (!info.isDirectory()) {
    throw new AdapterCompatibilityError(`artifact must be a file or directory: ${artifactPath}`);
  }
  const hash = createHash('sha256');
  await updateArtifactHash(hash, artifactPath);
  return hash.digest('hex');
}

export async function createManifest({ projectDir, artifacts = {}, contractPath = CONTRACT_PATH }) {
  const contract = await loadContract(contractPath);
  const frameworkVersion = await readReactNativeVersion(projectDir);
  const recordedArtifacts = {};

  for (const [platform, filePath] of Object.entries(artifacts)) {
    if (!filePath) continue;
    const absolute = path.resolve(filePath);
    recordedArtifacts[platform] = {
      fileName: path.basename(absolute),
      sha256: await sha256(absolute),
    };
  }

  return {
    schemaVersion: contract.schemaVersion,
    framework: contract.framework,
    frameworkVersion,
    fresnicaNativeSdkVersion: contract.fresnicaNativeSdkVersion,
    nativeBindingApiVersion: contract.nativeBindingApiVersion,
    adapterSourceVersion: contract.adapterSourceVersion,
    jsModuleName: contract.jsModuleName,
    androidHostDependencies: (contract.androidHostDependencies ?? []).map((dependency) =>
      dependency.replace('<frameworkVersion>', frameworkVersion),
    ),
    appleHostLinkerFlags: contract.appleHostLinkerFlags ?? [],
    artifacts: recordedArtifacts,
  };
}


const COMPATIBILITY_FIELDS = [
  'schemaVersion',
  'framework',
  'frameworkVersion',
  'fresnicaNativeSdkVersion',
  'nativeBindingApiVersion',
  'adapterSourceVersion',
  'jsModuleName',
  'androidHostDependencies',
  'appleHostLinkerFlags',
];

function sameCompatibility(left, right) {
  return COMPATIBILITY_FIELDS.every((field) => JSON.stringify(left[field]) === JSON.stringify(right[field]));
}

export async function writeManifest({ projectDir, manifestPath, artifacts = {}, contractPath = CONTRACT_PATH }) {
  const fresh = await createManifest({ projectDir, artifacts, contractPath });
  let previous = null;
  try {
    previous = JSON.parse(await readFile(manifestPath, 'utf8'));
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }

  if (previous && sameCompatibility(previous, fresh)) {
    fresh.artifacts = { ...(previous.artifacts ?? {}), ...fresh.artifacts };
  }

  await writeFile(manifestPath, `${JSON.stringify(fresh, null, 2)}\n`);
  return fresh;
}

export async function verifyManifest({ projectDir, manifestPath, contractPath = CONTRACT_PATH }) {
  const [contract, manifest, frameworkVersion] = await Promise.all([
    loadContract(contractPath),
    readFile(manifestPath, 'utf8').then(JSON.parse),
    readReactNativeVersion(projectDir),
  ]);

  const mismatches = [];
  const expected = {
    schemaVersion: contract.schemaVersion,
    framework: contract.framework,
    frameworkVersion,
    fresnicaNativeSdkVersion: contract.fresnicaNativeSdkVersion,
    nativeBindingApiVersion: contract.nativeBindingApiVersion,
    adapterSourceVersion: contract.adapterSourceVersion,
    jsModuleName: contract.jsModuleName,
    androidHostDependencies: (contract.androidHostDependencies ?? []).map((dependency) =>
      dependency.replace('<frameworkVersion>', frameworkVersion),
    ),
    appleHostLinkerFlags: contract.appleHostLinkerFlags ?? [],
  };

  for (const [field, value] of Object.entries(expected)) {
    const actual = manifest[field];
    const matches = Array.isArray(value)
      ? Array.isArray(actual) && JSON.stringify(actual) === JSON.stringify(value)
      : actual === value;
    if (!matches) {
      mismatches.push(`${field}: expected ${JSON.stringify(value)}, found ${JSON.stringify(actual)}`);
    }
  }

  for (const [platform, artifact] of Object.entries(manifest.artifacts ?? {})) {
    if (!artifact || typeof artifact.fileName !== 'string' || typeof artifact.sha256 !== 'string') {
      mismatches.push(`artifact ${platform}: invalid manifest record`);
      continue;
    }
    const artifactPath = path.join(path.dirname(manifestPath), artifact.fileName);
    try {
      const digest = await sha256(artifactPath);
      if (digest !== artifact.sha256) {
        mismatches.push(`artifact ${platform}: SHA-256 mismatch for ${artifact.fileName}`);
      }
    } catch {
      mismatches.push(`artifact ${platform}: missing ${artifact.fileName}`);
    }
  }

  if (mismatches.length > 0) {
    throw new AdapterCompatibilityError(`adapter rebuild required:\n- ${mismatches.join('\n- ')}`);
  }

  return manifest;
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const args = { command };
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
  if (args.command === 'manifest') {
    if (!args.project || !args.out) {
      throw new AdapterCompatibilityError('usage: adapter-manifest.mjs manifest --project PATH --out FILE [--android-aar FILE] [--apple-binary FILE]');
    }
    const manifest = await createManifest({
      projectDir: path.resolve(args.project),
      artifacts: {
        android: args['android-aar'],
        apple: args['apple-binary'],
      },
    });
    await writeFile(path.resolve(args.out), `${JSON.stringify(manifest, null, 2)}\n`);
    return;
  }

  if (args.command === 'check') {
    if (!args.project || !args.manifest) {
      throw new AdapterCompatibilityError('usage: adapter-manifest.mjs check --project PATH --manifest FILE');
    }
    await verifyManifest({
      projectDir: path.resolve(args.project),
      manifestPath: path.resolve(args.manifest),
    });
    process.stdout.write('Fresnica React Native adapter compatibility: OK\n');
    return;
  }

  throw new AdapterCompatibilityError('expected command: manifest or check');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error) => {
    const prefix = error instanceof AdapterCompatibilityError ? '' : `${error.name}: `;
    process.stderr.write(`${prefix}${error.message}\n`);
    process.exitCode = 2;
  });
}
