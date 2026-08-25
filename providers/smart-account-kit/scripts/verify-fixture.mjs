import { readFile } from 'node:fs/promises';

import { verifySmartAccountAuthFixture } from '../src/conformance.mjs';

const path = process.argv[2];
if (!path) {
  console.error('usage: node scripts/verify-fixture.mjs <fixture.json>');
  process.exit(2);
}

const fixture = JSON.parse(await readFile(path, 'utf8'));
const result = await verifySmartAccountAuthFixture(fixture);
console.log(JSON.stringify(result, null, 2));
