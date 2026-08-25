import assert from 'node:assert/strict';
import test from 'node:test';

import {
  SmartAccountKitProvider,
  normalizeTransactionResult,
} from '../src/provider.mjs';
import {
  SMART_ACCOUNT_KIT_VERSION,
  STELLAR_TESTNET_SMART_ACCOUNT,
  smartAccountKitConfig,
} from '../src/config.mjs';

const CONTRACT = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';
const OTHER_CONTRACT = 'CBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADX4';

function fakeKit(overrides = {}) {
  return {
    contractId: null,
    credentialId: null,
    async connectWallet() { return null; },
    async disconnect() {},
    ...overrides,
  };
}

test('pins the reviewed upstream provider and Protocol 27 testnet deployment', () => {
  assert.equal(SMART_ACCOUNT_KIT_VERSION, '0.6.2');
  assert.match(STELLAR_TESTNET_SMART_ACCOUNT.accountWasmHash, /^[0-9a-f]{64}$/);
  assert.match(STELLAR_TESTNET_SMART_ACCOUNT.webauthnVerifierAddress, /^C/);
  assert.equal(STELLAR_TESTNET_SMART_ACCOUNT.deploymentDate, '2026-07-09');
  const config = smartAccountKitConfig();
  assert.equal(config.rpcUrl, 'https://soroban-testnet.stellar.org');
  assert.equal(config.networkPassphrase, 'Test SDF Network ; September 2015');
});

test('rejects deployment identity overrides instead of mixing network identities', () => {
  assert.throws(
    () => smartAccountKitConfig({
      networkPassphrase: 'Public Global Stellar Network ; September 2015',
    }),
    /pinned to the reviewed Protocol 27 Testnet deployment/,
  );
  assert.throws(
    () => smartAccountKitConfig({ accountWasmHash: '00'.repeat(32) }),
    /pinned to the reviewed Protocol 27 Testnet deployment/,
  );
  assert.throws(
    () => smartAccountKitConfig({
      webauthnVerifierAddress: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
    }),
    /pinned to the reviewed Protocol 27 Testnet deployment/,
  );

  const customRpc = smartAccountKitConfig({ rpcUrl: 'https://example.testnet.invalid' });
  assert.equal(customRpc.rpcUrl, 'https://example.testnet.invalid');
  assert.equal(
    customRpc.networkPassphrase,
    STELLAR_TESTNET_SMART_ACCOUNT.networkPassphrase,
  );
});

test('restores and connects contract accounts without inventing an Ed25519 signer', async () => {
  const calls = [];
  const kit = fakeKit({
    async connectWallet(options) {
      calls.push(options);
      if (options === undefined) return { contractId: CONTRACT, credentialId: 'cred-1' };
      return { contractId: options.contractId, credentialId: options.credentialId };
    },
  });
  const provider = new SmartAccountKitProvider(kit);
  assert.deepEqual(await provider.restore(), {
    accountAddress: CONTRACT,
    credentialId: 'cred-1',
    provider: 'stellar-smart-account-kit',
  });
  assert.deepEqual(
    await provider.connect({ accountAddress: OTHER_CONTRACT, credentialId: 'cred-2' }),
    {
      accountAddress: OTHER_CONTRACT,
      credentialId: 'cred-2',
      provider: 'stellar-smart-account-kit',
    },
  );
  assert.deepEqual(calls, [undefined, { contractId: OTHER_CONTRACT, credentialId: 'cred-2' }]);
});

test('authenticates first, then discovers smart accounts by credential id', async () => {
  const kit = fakeKit({
    async authenticatePasskey() { return { credentialId: 'cred-1', rawResponse: { ignored: true } }; },
    async discoverContractsByCredential(id) {
      assert.equal(id, 'cred-1');
      return { contracts: [{ contract_id: CONTRACT }, { contract_id: OTHER_CONTRACT }] };
    },
  });
  const provider = new SmartAccountKitProvider(kit);
  assert.deepEqual(await provider.authenticateAndDiscover(), {
    credentialId: 'cred-1',
    accounts: [
      { accountAddress: CONTRACT, provider: 'stellar-smart-account-kit' },
      { accountAddress: OTHER_CONTRACT, provider: 'stellar-smart-account-kit' },
    ],
  });
});

test('wallet creation is non-submitting by default and never returns raw WebAuthn data', async () => {
  let received;
  const kit = fakeKit({
    async createWallet(appName, userName, options) {
      received = { appName, userName, options };
      return {
        contractId: CONTRACT,
        credentialId: 'cred-new',
        publicKey: new Uint8Array(65),
        rawResponse: { clientDataJSON: 'private-to-provider' },
        relayerPayload: { func: 'AAAA', auth: ['BBBB'] },
      };
    },
  });
  const provider = new SmartAccountKitProvider(kit);
  const result = await provider.createAccount({ appName: 'Fresnica', userName: 'alice' });
  assert.equal(received.options.autoSubmit, false);
  assert.equal(received.options.autoFund, false);
  assert.equal('rawResponse' in result, false);
  assert.equal('publicKey' in result, false);
  assert.deepEqual(result.deploymentPayload, { func: 'AAAA', auth: ['BBBB'] });
});

test('uses upstream signAndSubmit because WebAuthn signatures require re-simulation', async () => {
  const transaction = { assembled: true };
  const kit = fakeKit({
    async signAndSubmit(received, options) {
      assert.equal(received, transaction);
      assert.deepEqual(options, { forceMethod: 'relayer' });
      return { success: true, hash: 'abc', ledger: 123 };
    },
  });
  const provider = new SmartAccountKitProvider(kit);
  assert.deepEqual(
    await provider.signAndSubmit(transaction, { forceMethod: 'relayer' }),
    { status: 'confirmed', hash: 'abc', ledger: 123 },
  );
  assert.equal('sign' in provider, false);
});

test('testnet transfer delegates native-token signing/submission without exposing auth material', async () => {
  const calls = [];
  const kit = fakeKit({
    async transfer(token, recipient, amount, options) {
      calls.push({ token, recipient, amount, options });
      return { success: true, hash: 'transfer-hash', ledger: 456 };
    },
  });
  const provider = new SmartAccountKitProvider(kit);
  assert.deepEqual(
    await provider.transfer({ recipient: CONTRACT, amount: 1 }),
    { status: 'confirmed', hash: 'transfer-hash', ledger: 456 },
  );
  assert.deepEqual(calls, [{
    token: STELLAR_TESTNET_SMART_ACCOUNT.nativeTokenContract,
    recipient: CONTRACT,
    amount: 1,
    options: { forceMethod: 'relayer' },
  }]);
});

test('normalizes expected smart-account submission failures', () => {
  assert.deepEqual(
    normalizeTransactionResult({
      success: false,
      error: { code: 'CONTRACT_ERROR', message: 'denied' },
      hash: 'deadbeef',
    }),
    {
      status: 'failed',
      code: 'CONTRACT_ERROR',
      message: 'denied',
      hash: 'deadbeef',
    },
  );
});
