import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildSmartAccountAuthFixture,
  createRelayerCaptureFetch,
  SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA,
} from '../src/conformance-recorder.mjs';
import {
  SMART_ACCOUNT_KIT_PROVIDER_ID,
  SMART_ACCOUNT_KIT_VERSION,
  STELLAR_TESTNET_SMART_ACCOUNT,
} from '../src/config.mjs';

const ACCOUNT = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';
const RELAYER = STELLAR_TESTNET_SMART_ACCOUNT.relayerUrl;

function jsonResponse(value, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

test('captures only public relayer func/auth submissions', async () => {
  const calls = [];
  const recorder = createRelayerCaptureFetch({
    relayerUrl: RELAYER,
    async fetchImpl(input, init) {
      calls.push({ input, init });
      return jsonResponse({ success: true, hash: 'abc' });
    },
  });

  await recorder.fetch('https://soroban-testnet.stellar.org', { method: 'POST', body: '{}' });
  assert.equal(recorder.last(), null);

  await recorder.fetch(RELAYER, {
    method: 'POST',
    body: JSON.stringify({ func: 'AAAA', auth: ['BBBB'] }),
  });
  assert.equal(calls.length, 2);
  assert.deepEqual(recorder.last().request, { func: 'AAAA', auth: ['BBBB'] });
  assert.equal(recorder.last().response.body.hash, 'abc');

  recorder.clear();
  assert.equal(recorder.last(), null);
});

test('does not record signed-envelope relayer requests', async () => {
  const recorder = createRelayerCaptureFetch({
    relayerUrl: RELAYER,
    fetchImpl: async () => jsonResponse({ success: true, hash: 'abc' }),
  });
  await recorder.fetch(RELAYER, {
    method: 'POST',
    body: JSON.stringify({ xdr: 'AAAA' }),
  });
  assert.equal(recorder.last(), null);
});

test('builds a pinned Testnet fixture from confirmed capture', async () => {
  const recorder = createRelayerCaptureFetch({
    relayerUrl: RELAYER,
    fetchImpl: async () => jsonResponse({ success: true, hash: 'abc' }),
  });
  await recorder.fetch(RELAYER, {
    method: 'POST',
    body: JSON.stringify({ func: 'AAAA', auth: ['BBBB', 'CCCC'] }),
  });

  const fixture = buildSmartAccountAuthFixture({
    capture: recorder.last(),
    account: {
      accountAddress: ACCOUNT,
      credentialId: 'credential-id',
      provider: SMART_ACCOUNT_KIT_PROVIDER_ID,
    },
    result: { status: 'confirmed', hash: 'abc', ledger: 123 },
  });

  assert.equal(fixture.schema, SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA);
  assert.equal(fixture.provider.id, SMART_ACCOUNT_KIT_PROVIDER_ID);
  assert.equal(fixture.provider.smartAccountKitVersion, SMART_ACCOUNT_KIT_VERSION);
  assert.equal(fixture.network.passphrase, STELLAR_TESTNET_SMART_ACCOUNT.networkPassphrase);
  assert.deepEqual(fixture.invocation.authXdr, ['BBBB', 'CCCC']);
  assert.deepEqual(fixture.transaction, { hash: 'abc', ledger: 123 });
});

test('rejects a capture whose relayer hash differs from confirmed result', async () => {
  const recorder = createRelayerCaptureFetch({
    relayerUrl: RELAYER,
    fetchImpl: async () => jsonResponse({ success: true, hash: 'relayer-hash' }),
  });
  await recorder.fetch(RELAYER, {
    method: 'POST',
    body: JSON.stringify({ func: 'AAAA', auth: ['BBBB'] }),
  });

  assert.throws(() => buildSmartAccountAuthFixture({
    capture: recorder.last(),
    account: { accountAddress: ACCOUNT, credentialId: 'credential-id' },
    result: { status: 'confirmed', hash: 'other-hash' },
  }), /does not match confirmed hash/);
});
