import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';

import { SMART_ACCOUNT_KIT_VERSION } from '../src/config.mjs';

const require = createRequire(import.meta.url);
let cursor = path.dirname(require.resolve('smart-account-kit'));
let installed;
for (;;) {
  try {
    installed = JSON.parse(await readFile(path.join(cursor, 'package.json'), 'utf8'));
    if (installed.name === 'smart-account-kit') break;
  } catch {
    // Keep walking toward the package root.
  }
  const parent = path.dirname(cursor);
  if (parent === cursor) throw new Error('unable to locate smart-account-kit package.json');
  cursor = parent;
}

assert.equal(
  installed.version,
  SMART_ACCOUNT_KIT_VERSION,
  `smart-account-kit must be exactly ${SMART_ACCOUNT_KIT_VERSION}`,
);
const module = await import('smart-account-kit');
assert.equal(typeof module.SmartAccountKit, 'function');
console.log(`Fresnica smart-account provider dependency: smart-account-kit ${installed.version} OK`);
