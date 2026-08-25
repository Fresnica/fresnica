import {
  SMART_ACCOUNT_KIT_PROVIDER_ID,
  STELLAR_TESTNET_SMART_ACCOUNT,
} from './config.mjs';

function requireText(value, field) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${field} is required`);
  }
  return value;
}

function contractAddress(value, field = 'contractId') {
  requireText(value, field);
  if (!value.startsWith('C')) {
    throw new TypeError(`${field} must be a Stellar contract address`);
  }
  return value;
}

function connectedAccount(value) {
  if (value == null) return null;
  return Object.freeze({
    accountAddress: contractAddress(value.contractId),
    credentialId: requireText(value.credentialId, 'credentialId'),
    provider: SMART_ACCOUNT_KIT_PROVIDER_ID,
  });
}

export function normalizeTransactionResult(result) {
  if (!result || typeof result.success !== 'boolean') {
    throw new TypeError('smart-account-kit returned an invalid transaction result');
  }
  if (result.success) {
    return Object.freeze({
      status: 'confirmed',
      hash: requireText(result.hash, 'transaction hash'),
      ...(Number.isInteger(result.ledger) ? { ledger: result.ledger } : {}),
    });
  }
  const error = result.error ?? {};
  return Object.freeze({
    status: 'failed',
    code: typeof error.code === 'string' && error.code ? error.code : 'smart-account-error',
    message: typeof error.message === 'string' && error.message
      ? error.message
      : 'Smart-account transaction failed',
    ...(typeof result.hash === 'string' && result.hash ? { hash: result.hash } : {}),
  });
}

/**
 * Experimental provider boundary for Stellar contract accounts authorized by passkeys.
 *
 * The provider owns smart-account/WebAuthn integration. It is intentionally not a
 * Fresnica protected-software signer and never exposes WalletUnlockKey material.
 */
export class SmartAccountKitProvider {
  constructor(kit, { nativeTokenContract = STELLAR_TESTNET_SMART_ACCOUNT.nativeTokenContract } = {}) {
    if (!kit || typeof kit.connectWallet !== 'function') {
      throw new TypeError('a SmartAccountKit-compatible instance is required');
    }
    this.kit = kit;
    this.nativeTokenContract = nativeTokenContract;
  }

  get providerId() {
    return SMART_ACCOUNT_KIT_PROVIDER_ID;
  }

  activeAccount() {
    if (!this.kit.contractId || !this.kit.credentialId) return null;
    return connectedAccount({
      contractId: this.kit.contractId,
      credentialId: this.kit.credentialId,
    });
  }

  async restore() {
    return connectedAccount(await this.kit.connectWallet());
  }

  async connect({ accountAddress, credentialId, prompt = false, fresh = false } = {}) {
    if (accountAddress !== undefined) contractAddress(accountAddress, 'accountAddress');
    if (credentialId !== undefined) requireText(credentialId, 'credentialId');
    const result = await this.kit.connectWallet({
      ...(accountAddress ? { contractId: accountAddress } : {}),
      ...(credentialId ? { credentialId } : {}),
      ...(prompt ? { prompt: true } : {}),
      ...(fresh ? { fresh: true } : {}),
    });
    return connectedAccount(result);
  }

  async authenticateAndDiscover() {
    if (typeof this.kit.authenticatePasskey !== 'function' ||
        typeof this.kit.discoverContractsByCredential !== 'function') {
      throw new TypeError('smart-account-kit instance does not support passkey discovery');
    }
    const authenticated = await this.kit.authenticatePasskey();
    const credentialId = requireText(authenticated.credentialId, 'credentialId');
    const discovered = await this.kit.discoverContractsByCredential(credentialId);
    const contracts = Array.isArray(discovered)
      ? discovered
      : Array.isArray(discovered?.contracts)
        ? discovered.contracts
        : [];
    return Object.freeze({
      credentialId,
      accounts: Object.freeze(
        contracts.map((entry) => Object.freeze({
          accountAddress: contractAddress(entry.contract_id, 'contract_id'),
          provider: SMART_ACCOUNT_KIT_PROVIDER_ID,
        })),
      ),
    });
  }

  async createAccount({
    appName,
    userName,
    submit = false,
    autoFund = false,
    nickname,
    authenticatorSelection,
  }) {
    requireText(appName, 'appName');
    requireText(userName, 'userName');
    if (autoFund && !submit) {
      throw new TypeError('autoFund requires submit=true');
    }
    if (autoFund && !this.nativeTokenContract) {
      throw new TypeError('nativeTokenContract is required for autoFund');
    }

    const result = await this.kit.createWallet(appName, userName, {
      autoSubmit: submit,
      autoFund,
      ...(autoFund ? { nativeTokenContract: this.nativeTokenContract } : {}),
      ...(nickname ? { nickname } : {}),
      ...(authenticatorSelection ? { authenticatorSelection } : {}),
    });

    return Object.freeze({
      account: connectedAccount({
        contractId: result.contractId,
        credentialId: result.credentialId,
      }),
      submitted: Boolean(result.submitResult),
      ...(result.submitResult
        ? { submission: normalizeTransactionResult(result.submitResult) }
        : {}),
      // Manual deployment is provider-specific and intentionally remains opaque
      // to Fresnica Core/SDK.
      ...(result.relayerPayload ? { deploymentPayload: result.relayerPayload } : {}),
      ...(result.signedTransaction ? { signedDeploymentTransaction: result.signedTransaction } : {}),
    });
  }

  async signAndSubmit(assembledTransaction, options) {
    if (typeof this.kit.signAndSubmit !== 'function') {
      throw new TypeError('smart-account-kit instance does not support signAndSubmit');
    }
    // Deliberately no wrapper for kit.sign(): upstream requires re-simulation after
    // WebAuthn signatures. This provider keeps the safe sign -> re-simulate -> submit path.
    return normalizeTransactionResult(await this.kit.signAndSubmit(assembledTransaction, options));
  }

  async transfer({ recipient, amount, forceMethod = 'relayer' }) {
    requireText(recipient, 'recipient');
    if (!Number.isFinite(amount) || amount <= 0) {
      throw new TypeError('amount must be a positive finite number');
    }
    if (typeof this.kit.transfer !== 'function') {
      throw new TypeError('smart-account-kit instance does not support transfer');
    }
    return normalizeTransactionResult(
      await this.kit.transfer(this.nativeTokenContract, recipient, amount, { forceMethod }),
    );
  }

  async disconnect() {
    await this.kit.disconnect();
  }
}
