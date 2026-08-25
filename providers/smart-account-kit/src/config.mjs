export const SMART_ACCOUNT_KIT_PROVIDER_ID = 'stellar-smart-account-kit';
export const SMART_ACCOUNT_KIT_VERSION = '0.6.2';

export const STELLAR_TESTNET_SMART_ACCOUNT = Object.freeze({
  network: 'testnet',
  rpcUrl: 'https://soroban-testnet.stellar.org',
  networkPassphrase: 'Test SDF Network ; September 2015',
  accountWasmHash: '1b5f4534a76322da2ad7c745f6900857a6802b0ca79850c35a03561df997785a',
  webauthnVerifierAddress: 'CC7EKIHQP3TN4CARQDND6CEOY2UXLWWC2X5GHTD5NLAT7BG5GPZIOM3F',
  ed25519VerifierAddress: 'CAAVTMCBXEIBPR64EAASKFXERVPYFZA2JYP5A3BG6PESWEFUJX5IHKN4',
  nativeTokenContract: 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC',
  relayerUrl: 'https://smart-account-relayer-proxy.sdf-ecosystem.workers.dev',
  deploymentSource: 'stellar/smart-account-kit demo/.env.example',
  deploymentDate: '2026-07-09',
});

export function smartAccountKitConfig(overrides = {}) {
  const source = { ...STELLAR_TESTNET_SMART_ACCOUNT, ...overrides };
  const required = [
    'rpcUrl',
    'networkPassphrase',
    'accountWasmHash',
    'webauthnVerifierAddress',
  ];
  for (const field of required) {
    if (typeof source[field] !== 'string' || source[field].trim() === '') {
      throw new TypeError(`${field} is required`);
    }
  }
  if (!/^[0-9a-f]{64}$/i.test(source.accountWasmHash)) {
    throw new TypeError('accountWasmHash must be 32-byte hex');
  }
  if (!source.webauthnVerifierAddress.startsWith('C')) {
    throw new TypeError('webauthnVerifierAddress must be a Stellar contract address');
  }

  return {
    rpcUrl: source.rpcUrl,
    networkPassphrase: source.networkPassphrase,
    accountWasmHash: source.accountWasmHash,
    webauthnVerifierAddress: source.webauthnVerifierAddress,
    ...(source.ed25519VerifierAddress
      ? { ed25519VerifierAddress: source.ed25519VerifierAddress }
      : {}),
    ...(source.relayerUrl ? { relayerUrl: source.relayerUrl } : {}),
    ...(source.indexerUrl !== undefined ? { indexerUrl: source.indexerUrl } : {}),
    ...(source.rpId ? { rpId: source.rpId } : {}),
    ...(source.rpName ? { rpName: source.rpName } : {}),
    ...(source.storage ? { storage: source.storage } : {}),
    ...(source.webAuthn ? { webAuthn: source.webAuthn } : {}),
  };
}
