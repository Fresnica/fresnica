import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = await readFile(new URL('../src/provider.mjs', import.meta.url), 'utf8');

test('passkey provider stays outside the Fresnica software-signer unlock-key path', () => {
  for (const forbidden of [
    'deriveUnlockKey',
    'validateUnlockKey',
    'WalletUnlockKey(',
    'protectSecret(',
    'protectMnemonic(',
    'applyEd25519Signature(',
  ]) {
    assert.equal(source.includes(forbidden), false, `${forbidden} must not enter the passkey provider`);
  }
});

test('provider does not expose the unsafe auth-entry-only kit.sign shortcut', () => {
  assert.doesNotMatch(source, /\bthis\.kit\.sign\s*\(/);
  assert.match(source, /\bthis\.kit\.signAndSubmit\s*\(/);
});
