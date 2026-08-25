import {
  SMART_ACCOUNT_KIT_PROVIDER_ID,
  SMART_ACCOUNT_KIT_VERSION,
  STELLAR_TESTNET_SMART_ACCOUNT,
} from './config.mjs';

export const SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA = 'fresnica-smart-account-auth-v1';

function requireText(value, field) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${field} is required`);
  }
  return value;
}

function normalizedUrl(value) {
  return requireText(value, 'relayerUrl').replace(/\/+$/, '');
}

function requestUrl(input) {
  if (typeof input === 'string') return input;
  if (input instanceof URL) return input.toString();
  if (input && typeof input.url === 'string') return input.url;
  return '';
}

function parseRelayerBody(body) {
  if (typeof body !== 'string') return null;
  let parsed;
  try {
    parsed = JSON.parse(body);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed.func !== 'string' || !Array.isArray(parsed.auth)) {
    return null;
  }
  if (!parsed.func || parsed.auth.length === 0 || parsed.auth.some((value) => typeof value !== 'string' || !value)) {
    return null;
  }
  return Object.freeze({
    func: parsed.func,
    auth: Object.freeze([...parsed.auth]),
  });
}

/**
 * Wrap fetch only for the Testnet smoke harness. It records the public
 * func/auth payload posted to the configured relayer and leaves every other
 * request untouched.
 */
export function createRelayerCaptureFetch({ relayerUrl, fetchImpl = globalThis.fetch } = {}) {
  const target = normalizedUrl(relayerUrl);
  if (typeof fetchImpl !== 'function') {
    throw new TypeError('fetchImpl is required');
  }

  const captures = [];
  const recordedFetch = async (input, init) => {
    const url = requestUrl(input).replace(/\/+$/, '');
    const relayerBody = url === target ? parseRelayerBody(init?.body) : null;
    const response = await fetchImpl(input, init);

    if (relayerBody) {
      let responseBody = null;
      try {
        responseBody = await response.clone().json();
      } catch {
        // A malformed relayer response remains useful as a request capture.
      }
      captures.push(Object.freeze({
        url: target,
        request: relayerBody,
        response: Object.freeze({
          ok: response.ok,
          status: response.status,
          body: responseBody,
        }),
      }));
    }

    return response;
  };

  return Object.freeze({
    fetch: recordedFetch,
    clear() {
      captures.length = 0;
    },
    last() {
      return captures.length ? captures[captures.length - 1] : null;
    },
  });
}

function responseHash(capture) {
  const body = capture?.response?.body;
  if (!body || typeof body !== 'object') return null;
  if (typeof body.hash === 'string' && body.hash) return body.hash;
  if (body.data && typeof body.data === 'object' && typeof body.data.hash === 'string' && body.data.hash) {
    return body.data.hash;
  }
  return null;
}

export function buildSmartAccountAuthFixture({ capture, account, result } = {}) {
  if (!capture?.request) {
    throw new TypeError('a captured relayer func/auth request is required');
  }
  if (!account) {
    throw new TypeError('active smart account is required');
  }
  if (!result || result.status !== 'confirmed') {
    throw new TypeError('a confirmed smart-account transaction result is required');
  }

  const accountAddress = requireText(account.accountAddress, 'accountAddress');
  const credentialId = requireText(account.credentialId, 'credentialId');
  const hash = requireText(result.hash, 'transaction hash');
  const relayerHash = responseHash(capture);
  if (relayerHash && relayerHash !== hash) {
    throw new Error(`relayer hash ${relayerHash} does not match confirmed hash ${hash}`);
  }

  return Object.freeze({
    schema: SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA,
    capturedAt: new Date().toISOString(),
    provider: Object.freeze({
      id: SMART_ACCOUNT_KIT_PROVIDER_ID,
      smartAccountKitVersion: SMART_ACCOUNT_KIT_VERSION,
      deploymentDate: STELLAR_TESTNET_SMART_ACCOUNT.deploymentDate,
      accountWasmHash: STELLAR_TESTNET_SMART_ACCOUNT.accountWasmHash,
      webauthnVerifierAddress: STELLAR_TESTNET_SMART_ACCOUNT.webauthnVerifierAddress,
    }),
    network: Object.freeze({
      name: STELLAR_TESTNET_SMART_ACCOUNT.network,
      passphrase: STELLAR_TESTNET_SMART_ACCOUNT.networkPassphrase,
    }),
    account: Object.freeze({
      address: accountAddress,
      credentialId,
    }),
    transaction: Object.freeze({
      hash,
      ...(Number.isInteger(result.ledger) ? { ledger: result.ledger } : {}),
    }),
    invocation: Object.freeze({
      funcXdr: capture.request.func,
      authXdr: Object.freeze([...capture.request.auth]),
    }),
  });
}
