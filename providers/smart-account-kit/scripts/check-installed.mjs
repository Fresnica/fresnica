import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

import { SMART_ACCOUNT_KIT_VERSION } from '../src/config.mjs';

const installed = JSON.parse(
  await readFile(new URL('../node_modules/smart-account-kit/package.json', import.meta.url), 'utf8'),
);

assert.equal(
  installed.version,
  SMART_ACCOUNT_KIT_VERSION,
  `smart-account-kit must be exactly ${SMART_ACCOUNT_KIT_VERSION}`,
);
const module = await import('smart-account-kit');
assert.equal(typeof module.SmartAccountKit, 'function');
console.log(`Fresnica smart-account provider dependency: smart-account-kit ${installed.version} OK`);
